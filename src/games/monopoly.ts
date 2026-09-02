export const MONOPOLY_BOARD_SIZE = 40;
export const MONOPOLY_TURN_TIMEOUT_MS = 30_000;
export const MONOPOLY_BUILDING_VARIANT_COUNT = 3;

export const MONOPOLY_ECONOMY = {
  emptyPurchase: 350,
  houseDirectPurchase: 500,
  level2DirectPurchase: 1500,
  level3DirectPurchase: 3500,
  houseRent: 500,
  level2Rent: 1000,
  level3Rent: 2000,
} as const;

export type MonopolyCorner = "start" | "airport" | "price_double" | "jail";
export type MonopolyPropertyLevel = "empty" | "house" | "level2" | "level3";

export type MonopolyCornerTile = {
  index: number;
  kind: "corner";
  corner: MonopolyCorner;
};

export type MonopolyEventTile = {
  index: number;
  kind: "event";
};

export type MonopolyPropertyTile = {
  index: number;
  kind: "property";
  district: number;
};

export type MonopolyTile = MonopolyCornerTile | MonopolyEventTile | MonopolyPropertyTile;

export type MonopolyDirection = "clockwise" | "counterclockwise";
export type MonopolyCard = "acquittal" | "seize" | "frame" | "double" | "fixed_dice" | "roadblock" | "turtle" | "reverse" | "loot" | "seal";
export type MonopolyGod = "wealth" | "poverty" | "angel" | "devil";
export type MonopolyRandomEvent = "demolish" | "downgrade" | "takeover" | "jail" | "subsidy" | "rich_to_poor" | "upgrade" | "maintenance" | "dispute" | "rent_holiday";

export type MonopolyPlayerSeed = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  isBot?: boolean;
};

export type MonopolyPlayer = MonopolyPlayerSeed & {
  coins: number;
  position: number;
  direction: MonopolyDirection;
  eliminated: boolean;
  jailTurns: number;
  cards: MonopolyCard[];
  turtleTurns: number;
  forcedDice?: number;
  god?: MonopolyGod;
  godTurns: number;
};

export type MonopolyPropertyState = {
  index: number;
  level: MonopolyPropertyLevel;
  /** 由房主在购买或升级时抽取，确保所有客户端展示同一栋建筑。 */
  buildingVariant: number | null;
  ownerDeviceId: string | null;
  sealedTurns: number;
  tollMultiplier: number;
};

export type MonopolyRoadblock = {
  index: number;
  placedByDeviceId: string;
};

export type MonopolyGodToken = {
  index: number;
  god: MonopolyGod;
  /** 神明仅在非角落格刷新，两个完整回合无人拾取即消失。 */
  expiresAtRound: number;
};

export type MonopolyState = {
  board: MonopolyTile[];
  players: MonopolyPlayer[];
  properties: Record<number, MonopolyPropertyState>;
  roadblocks: MonopolyRoadblock[];
  godTokens: MonopolyGodToken[];
  rentHolidayDistrict?: number;
  rentHolidayRounds: number;
  startingCoins: number;
  maxRounds: number;
  completedRounds: number;
  currentPlayerId: string;
  turnStartedAt: number;
  logs: string[];
};

export type MonopolyActionResult =
  | { ok: true; state: MonopolyState }
  | { ok: false; state: MonopolyState; error: string };

export type MonopolyCardTarget = {
  playerId?: string;
  propertyIndex?: number;
  index?: number;
  dice?: number;
};

const baseCardWeights: Record<MonopolyCard, number> = {
  acquittal: 10,
  seize: 3,
  frame: 8,
  double: 9,
  fixed_dice: 14,
  roadblock: 17,
  turtle: 13,
  reverse: 10,
  loot: 6,
  seal: 10,
};

const trailingCardWeights: Record<MonopolyCard, number> = {
  acquittal: 12,
  seize: 8,
  frame: 8,
  double: 14,
  fixed_dice: 12,
  roadblock: 13,
  turtle: 10,
  reverse: 8,
  loot: 10,
  seal: 5,
};

const strongCards = new Set<MonopolyCard>(["seize", "frame", "loot", "seal"]);

const corners: Record<number, MonopolyCorner> = {
  0: "start",
  10: "airport",
  20: "price_double",
  30: "jail",
};

const eventIndices = new Set([5, 15, 25, 35]);

export function createMonopolyBoard(): MonopolyTile[] {
  return Array.from({ length: MONOPOLY_BOARD_SIZE }, (_, index) => {
    const corner = corners[index];
    if (corner) return { index, kind: "corner", corner };
    if (eventIndices.has(index)) return { index, kind: "event" };
    return { index, kind: "property", district: propertyDistrictIndex(index) };
  });
}

export function propertyDistrictOf(board: MonopolyTile[], index: number): number | null {
  const tile = board[index];
  return tile?.kind === "property" ? tile.district : null;
}

export function createMonopolyState(players: MonopolyPlayerSeed[], options: { startingCoins?: number; maxRounds?: number; now?: number; seed?: number } = {}): MonopolyState {
  const startingCoins = normalizeStartingCoins(options.startingCoins ?? 5000);
  const maxRounds = normalizeMaxRounds(options.maxRounds ?? 20);
  const board = createMonopolyBoard();
  const properties: Record<number, MonopolyPropertyState> = {};
  for (const tile of board) {
    if (tile.kind === "property") {
      properties[tile.index] = {
        index: tile.index,
        level: "empty",
        buildingVariant: null,
        ownerDeviceId: null,
        sealedTurns: 0,
        tollMultiplier: 1,
      };
    }
  }
  const seats = players.slice(0, 4).map((player) => ({
    ...player,
    avatar: player.avatar ?? null,
    coins: startingCoins,
    position: 0,
    direction: "clockwise" as const,
    eliminated: false,
    jailTurns: 0,
    cards: [],
    turtleTurns: 0,
    godTurns: 0,
  }));
  const state: MonopolyState = {
    board,
    players: seats,
    properties,
    roadblocks: [],
    godTokens: [],
    rentHolidayRounds: 0,
    startingCoins,
    maxRounds,
    completedRounds: 0,
    currentPlayerId: seats[0]?.deviceId ?? "",
    turnStartedAt: options.now ?? Date.now(),
    logs: [],
  };
  const random = seededRandom(options.seed ?? Date.now());
  for (const player of state.players) {
    while (player.cards.length < 3) {
      const card = selectMonopolyCard(state, player.deviceId, random, true);
      if (!card) break;
      player.cards.push(card);
    }
  }
  return state;
}

export function monopolyTurnRemainingSeconds(turnStartedAt: number | undefined, now = Date.now(), timeoutMs = MONOPOLY_TURN_TIMEOUT_MS): number {
  if (!turnStartedAt) return Math.ceil(timeoutMs / 1000);
  return Math.max(0, Math.ceil((timeoutMs - (now - turnStartedAt)) / 1000));
}

export function purchaseMonopolyProperty(state: MonopolyState, playerId: string, index: number, random: () => number = Math.random): MonopolyActionResult {
  const property = state.properties[index];
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!property || !player || player.eliminated) return failed(state, "无法购买该地产");
  if (property.ownerDeviceId) return failed(state, "该地产已有归属");
  const price = monopolyDirectPurchasePrice(property.level);
  if (player.coins < price) return failed(state, "金币不足，无法购买地产");
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  const nextProperty = next.properties[index]!;
  nextPlayer.coins -= price;
  nextProperty.ownerDeviceId = playerId;
  if (nextProperty.level === "empty") {
    nextProperty.level = "house";
    nextProperty.buildingVariant = monopolyBuildingVariant(random);
  }
  nextProperty.sealedTurns = 0;
  next.logs.push(`${nextPlayer.nickname} 购买了${propertyLabel(nextProperty.level)}地产`);
  return { ok: true, state: next };
}

export function upgradeMonopolyProperty(state: MonopolyState, playerId: string, index: number, random: () => number = Math.random): MonopolyActionResult {
  const property = state.properties[index];
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!property || !player || player.eliminated || property.ownerDeviceId !== playerId) return failed(state, "只能升级自己的地产");
  const upgrade = property.level === "house"
    ? { level: "level2" as const }
    : property.level === "level2"
      ? { level: "level3" as const }
      : null;
  if (!upgrade) return failed(state, "该地产不能继续升级");
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  next.properties[index]!.level = upgrade.level;
  next.properties[index]!.buildingVariant = monopolyBuildingVariant(random);
  next.logs.push(`${nextPlayer.nickname} 再次踩中自己的地产，免费升级了建筑`);
  return { ok: true, state: next };
}

export function monopolyLandingRent(state: MonopolyState, payerId: string, index: number): number {
  const property = state.properties[index];
  const payer = state.players.find((player) => player.deviceId === payerId);
  if (!property || !property.ownerDeviceId || property.ownerDeviceId === payerId || property.sealedTurns > 0 || payer?.god === "wealth") return 0;
  const tile = state.board[index];
  if (tile?.kind !== "property") return 0;
  if (state.rentHolidayRounds > 0 && state.rentHolidayDistrict === tile.district) return 0;
  const district = state.board
    .filter((item): item is MonopolyPropertyTile => item.kind === "property" && item.district === tile.district)
    .map((item) => item.index)
    .sort((a, b) => a - b);
  const position = district.indexOf(index);
  if (position < 0) return 0;
  const segment = [index];
  for (let cursor = position - 1; cursor >= 0; cursor -= 1) {
    const candidate = state.properties[district[cursor]!];
    if (!candidate || candidate.ownerDeviceId !== property.ownerDeviceId || candidate.sealedTurns > 0) break;
    segment.unshift(candidate.index);
  }
  for (let cursor = position + 1; cursor < district.length; cursor += 1) {
    const candidate = state.properties[district[cursor]!];
    if (!candidate || candidate.ownerDeviceId !== property.ownerDeviceId || candidate.sealedTurns > 0) break;
    segment.push(candidate.index);
  }
  const rent = segment.reduce((total, propertyIndex) => total + propertyToll(state.properties[propertyIndex]!), 0);
  return payer?.god === "poverty" ? rent * 2 : rent;
}

export function applyMonopolyPropertySeal(state: MonopolyState, index: number, turns = 3): MonopolyState {
  const property = state.properties[index];
  if (!property || property.level === "empty") return state;
  const next = cloneMonopolyState(state);
  next.properties[index]!.sealedTurns = Math.max(1, Math.floor(turns));
  next.logs.push(`一块${propertyLabel(property.level)}地产被查封`);
  return next;
}

export function endMonopolyTurn(state: MonopolyState, now = Date.now(), random: () => number = Math.random): MonopolyState {
  const currentIndex = state.players.findIndex((player) => player.deviceId === state.currentPlayerId);
  if (currentIndex < 0) return state;
  const next = cloneMonopolyState(state);
  const currentId = next.players[currentIndex]!.deviceId;
  for (const property of Object.values(next.properties)) {
    if (property.ownerDeviceId === currentId && property.sealedTurns > 0) property.sealedTurns -= 1;
  }
  const currentPlayer = playerOf(next, currentId);
  if (currentPlayer?.godTurns && currentPlayer.godTurns > 0) {
    currentPlayer.godTurns -= 1;
    if (currentPlayer.godTurns === 0) currentPlayer.god = undefined;
  }
  const activePlayers = next.players.filter((player) => !player.eliminated);
  if (activePlayers.length <= 1) return { ...next, turnStartedAt: now };
  let nextIndex = currentIndex;
  do {
    nextIndex = (nextIndex + 1) % next.players.length;
  } while (next.players[nextIndex]?.eliminated);
  next.currentPlayerId = next.players[nextIndex]!.deviceId;
  if (nextIndex <= currentIndex) {
    next.completedRounds += 1;
    if (next.rentHolidayRounds > 0) {
      next.rentHolidayRounds -= 1;
      if (next.rentHolidayRounds === 0) next.rentHolidayDistrict = undefined;
    }
    next.godTokens = next.godTokens.filter((token) => token.expiresAtRound > next.completedRounds);
    if (next.completedRounds > 0 && next.completedRounds % 3 === 0) {
      for (const player of next.players) {
        if (player.eliminated || player.cards.length >= 3) continue;
        const card = selectMonopolyCard(next, player.deviceId, random, false);
        if (card) {
          player.cards.push(card);
          next.logs.push(`${player.nickname} 获得了一张道具卡`);
        }
      }
      refreshMonopolyGodTokens(next, random);
    }
  }
  next.turnStartedAt = now;
  return next;
}

export function declareMonopolyBankruptcy(state: MonopolyState, playerId: string): MonopolyState {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) return state;
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  nextPlayer.eliminated = true;
  nextPlayer.coins = 0;
  nextPlayer.jailTurns = 0;
  for (const property of Object.values(next.properties)) {
    if (property.ownerDeviceId !== playerId) continue;
    property.ownerDeviceId = null;
    property.sealedTurns = 0;
    property.tollMultiplier = 1;
  }
  next.logs.push(`${nextPlayer.nickname} 破产，地产转为银行暂持`);
  return next;
}

export function moveMonopolyPlayer(state: MonopolyState, playerId: string, steps: number, random: () => number = Math.random): MonopolyActionResult {
  const player = state.players.find((item) => item.deviceId === playerId);
  const requestedDistance = Math.floor(steps);
  if (!player || player.eliminated) return failed(state, "玩家无法移动");
  if (player.jailTurns > 0) return failed(state, "玩家正在监狱中");
  if (requestedDistance < 1 || requestedDistance > 12) return failed(state, "骰子点数必须在 1 到 12 之间");
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  const distance = nextPlayer.turtleTurns > 0 ? 1 : requestedDistance;
  const direction = nextPlayer.direction === "clockwise" ? 1 : -1;
  for (let moved = 0; moved < distance; moved += 1) {
    nextPlayer.position = (nextPlayer.position + direction + MONOPOLY_BOARD_SIZE) % MONOPOLY_BOARD_SIZE;
    if (nextPlayer.position === 0) grantStartReward(next, nextPlayer, random);
    const roadblockIndex = next.roadblocks.findIndex((roadblock) => roadblock.index === nextPlayer.position);
    if (roadblockIndex >= 0) {
      next.roadblocks.splice(roadblockIndex, 1);
      next.logs.push(`${nextPlayer.nickname} 被路障拦下`);
      break;
    }
  }
  if (nextPlayer.turtleTurns > 0) nextPlayer.turtleTurns -= 1;
  next.logs.push(`${nextPlayer.nickname} 前进了 ${distance} 格`);
  return { ok: true, state: next };
}

export function teleportMonopolyPlayer(state: MonopolyState, playerId: string, targetIndex: number, random: () => number = Math.random): MonopolyActionResult {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) return failed(state, "玩家无法传送");
  if (!Number.isInteger(targetIndex) || targetIndex < 0 || targetIndex >= MONOPOLY_BOARD_SIZE || targetIndex === player.position) {
    return failed(state, "传送目标无效");
  }
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  nextPlayer.position = targetIndex;
  if (targetIndex === 0) grantStartReward(next, nextPlayer, random);
  next.logs.push(`${nextPlayer.nickname} 使用飞机场传送`);
  return { ok: true, state: next };
}

export function applyMonopolyPriceDouble(state: MonopolyState, playerId: string, random: () => number = Math.random): MonopolyState {
  const candidates = Object.values(state.properties).filter((property) => property.ownerDeviceId === playerId);
  if (candidates.length === 0) return state;
  const next = cloneMonopolyState(state);
  const selected = candidates[Math.min(candidates.length - 1, Math.max(0, Math.floor(random() * candidates.length)))];
  if (!selected) return state;
  const property = next.properties[selected.index]!;
  property.tollMultiplier += 1;
  next.logs.push(`${playerOf(next, playerId)?.nickname ?? "玩家"} 的一块地产过路费提升至 ×${property.tollMultiplier}`);
  return next;
}

export function sendMonopolyPlayerToJail(state: MonopolyState, playerId: string): MonopolyState {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) return state;
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  const acquittalIndex = nextPlayer.cards.indexOf("acquittal");
  if (acquittalIndex >= 0) {
    nextPlayer.cards.splice(acquittalIndex, 1);
    nextPlayer.position = 0;
    nextPlayer.jailTurns = 0;
    grantStartReward(next, nextPlayer);
    next.logs.push(`${nextPlayer.nickname} 使用免罪卡回到起点`);
    return next;
  }
  nextPlayer.position = 30;
  nextPlayer.jailTurns = 3;
  next.logs.push(`${nextPlayer.nickname} 被送入监狱`);
  return next;
}

export function monopolyCardWeights(state: MonopolyState, playerId: string): Record<MonopolyCard, number> {
  const score = monopolyCatchupScore(state, playerId);
  if (score >= 3) return { ...trailingCardWeights };
  if (score <= 0) return { ...baseCardWeights };
  return Object.fromEntries((Object.keys(baseCardWeights) as MonopolyCard[]).map((card) => [
    card,
    baseCardWeights[card] + (trailingCardWeights[card] - baseCardWeights[card]) * (score / 3),
  ])) as Record<MonopolyCard, number>;
}

export function drawMonopolyCard(state: MonopolyState, playerId: string, random: () => number = Math.random): { state: MonopolyState; card: MonopolyCard | null } {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated || player.cards.length >= 3) return { state, card: null };
  const card = selectMonopolyCard(state, playerId, random, false);
  if (!card) return { state, card: null };
  const next = cloneMonopolyState(state);
  playerOf(next, playerId)!.cards.push(card);
  next.logs.push(`${playerOf(next, playerId)!.nickname} 获得了一张道具卡`);
  return { state: next, card };
}

export function discardMonopolyCard(state: MonopolyState, playerId: string, card: MonopolyCard | undefined): MonopolyState {
  if (!card) return state;
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || !player.cards.includes(card)) return state;
  const next = cloneMonopolyState(state);
  const nextPlayer = playerOf(next, playerId)!;
  const index = nextPlayer.cards.indexOf(card);
  nextPlayer.cards.splice(index, 1);
  next.logs.push(`${nextPlayer.nickname} 弃置了一张道具卡`);
  return next;
}

export function useMonopolyCard(state: MonopolyState, playerId: string, card: MonopolyCard, target: MonopolyCardTarget = {}, random: () => number = Math.random): MonopolyActionResult {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated || !player.cards.includes(card)) return failed(state, "未持有该道具卡");
  if (card === "acquittal") return failed(state, "免罪卡会在入狱时自动使用");
  const next = cloneMonopolyState(state);
  const actor = playerOf(next, playerId)!;
  const consume = () => actor.cards.splice(actor.cards.indexOf(card), 1);

  if (card === "reverse") {
    const targetPlayer = playerOf(next, target.playerId ?? "");
    if (!targetPlayer || targetPlayer.eliminated) return failed(state, "转向目标无效");
    targetPlayer.direction = targetPlayer.direction === "clockwise" ? "counterclockwise" : "clockwise";
    consume();
    next.logs.push(`${actor.nickname} 改变了 ${targetPlayer.nickname} 的行进方向`);
    return { ok: true, state: next };
  }
  if (card === "roadblock") {
    const index = target.index;
    if (!Number.isInteger(index) || index! < 0 || index! >= MONOPOLY_BOARD_SIZE || next.roadblocks.some((item) => item.index === index)) {
      return failed(state, "路障位置无效或已存在路障");
    }
    consume();
    next.roadblocks.push({ index: index!, placedByDeviceId: playerId });
    next.logs.push(`${actor.nickname} 放置了路障`);
    return { ok: true, state: next };
  }
  if (card === "seize") {
    const property = next.properties[target.propertyIndex ?? -1];
    if (!property || !property.ownerDeviceId || property.ownerDeviceId === playerId) return failed(state, "抢占目标无效");
    property.ownerDeviceId = playerId;
    property.sealedTurns = 0;
    consume();
    next.logs.push(`${actor.nickname} 抢占了一块地产`);
    return { ok: true, state: next };
  }
  if (card === "frame") {
    const targetPlayer = playerOf(next, target.playerId ?? "");
    if (!targetPlayer || targetPlayer.eliminated) return failed(state, "陷害目标无效");
    consume();
    return { ok: true, state: sendMonopolyPlayerToJail(next, targetPlayer.deviceId) };
  }
  if (card === "double") {
    const property = next.properties[target.propertyIndex ?? -1];
    if (!property || property.level === "empty") return failed(state, "翻倍目标无效");
    property.tollMultiplier += 1;
    consume();
    next.logs.push(`${actor.nickname} 将一块地产提升至 ×${property.tollMultiplier}`);
    return { ok: true, state: next };
  }
  if (card === "fixed_dice") {
    const dice = Math.floor(target.dice ?? 0);
    if (dice < 1 || dice > 6) return failed(state, "指定点数必须在 1 到 6 之间");
    actor.forcedDice = dice;
    consume();
    next.logs.push(`${actor.nickname} 指定了下一次骰子点数`);
    return { ok: true, state: next };
  }
  if (card === "turtle") {
    const targetPlayer = playerOf(next, target.playerId ?? "");
    if (!targetPlayer || targetPlayer.eliminated) return failed(state, "乌龟目标无效");
    targetPlayer.turtleTurns = 3;
    consume();
    next.logs.push(`${actor.nickname} 对 ${targetPlayer.nickname} 使用了乌龟卡`);
    return { ok: true, state: next };
  }
  if (card === "loot") {
    const targetPlayer = playerOf(next, target.playerId ?? "");
    if (!targetPlayer || targetPlayer.deviceId === playerId || targetPlayer.cards.length === 0) return failed(state, "掠夺目标无道具可夺取");
    consume();
    const cardIndex = Math.min(targetPlayer.cards.length - 1, Math.max(0, Math.floor(random() * targetPlayer.cards.length)));
    const stolen = targetPlayer.cards.splice(cardIndex, 1)[0];
    if (stolen) actor.cards.push(stolen);
    next.logs.push(`${actor.nickname} 掠夺了 ${targetPlayer.nickname} 的道具`);
    return { ok: true, state: next };
  }
  if (card === "seal") {
    const property = next.properties[target.propertyIndex ?? -1];
    if (!property || property.level === "empty") return failed(state, "查封目标无效");
    consume();
    return { ok: true, state: applyMonopolyPropertySeal(next, property.index, 3) };
  }
  return failed(state, "未知道具卡");
}

export function acquireMonopolyGod(state: MonopolyState, playerId: string, god: MonopolyGod): MonopolyState {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) return state;
  const next = cloneMonopolyState(state);
  const holder = playerOf(next, playerId)!;
  holder.god = god;
  holder.godTurns = 3;
  const amount = godCashAmount(next.startingCoins);
  if (god === "wealth") {
    for (const opponent of next.players) {
      if (opponent.deviceId === playerId || opponent.eliminated) continue;
      const paid = Math.min(amount, opponent.coins);
      opponent.coins -= paid;
      holder.coins += paid;
    }
  }
  if (god === "poverty") {
    const opponents = next.players.filter((opponent) => opponent.deviceId !== playerId && !opponent.eliminated);
    for (const opponent of opponents) {
      const paid = Math.min(amount, holder.coins);
      holder.coins -= paid;
      opponent.coins += paid;
    }
  }
  next.logs.push(`${holder.nickname} 获得了${godLabel(god)}附身`);
  return next;
}

/** 只在房主权威状态机调用；同一玩家最多携带一位神明。 */
export function collectMonopolyGodToken(state: MonopolyState, playerId: string, index: number): MonopolyState {
  const token = state.godTokens.find((item) => item.index === index);
  if (!token) return state;
  const next = acquireMonopolyGod(state, playerId, token.god);
  return {
    ...next,
    godTokens: next.godTokens.filter((item) => item.index !== index),
  };
}

export function resolveMonopolyLanding(state: MonopolyState, playerId: string, index: number, _random: () => number = Math.random): MonopolyState {
  const player = state.players.find((item) => item.deviceId === playerId);
  const property = state.properties[index];
  if (!player || player.eliminated || !property || property.level === "empty") return state;
  if (player.god !== "angel" && player.god !== "devil") return state;
  const next = cloneMonopolyState(state);
  const target = next.properties[index]!;
  if (player.god === "angel") {
    if (target.level === "house") target.level = "level2";
    else if (target.level === "level2") target.level = "level3";
    next.logs.push(`${player.nickname} 的天使升级了一块地产`);
  } else if (target.level === "level3") {
    target.level = "level2";
    next.logs.push(`${player.nickname} 的恶魔降低了一块地产`);
  } else if (target.level === "level2") {
    target.level = "house";
    next.logs.push(`${player.nickname} 的恶魔降低了一块地产`);
  } else {
    target.level = "empty";
    target.ownerDeviceId = null;
    target.sealedTurns = 0;
    target.tollMultiplier = 1;
    next.logs.push(`${player.nickname} 的恶魔拆除了一座小屋`);
  }
  return next;
}

export function applyMonopolyRandomEvent(state: MonopolyState, event: MonopolyRandomEvent, random: () => number = Math.random): MonopolyState {
  const next = cloneMonopolyState(state);
  const properties = Object.values(next.properties);
  const activePlayers = next.players.filter((player) => !player.eliminated);
  const pick = <T>(items: T[]): T | undefined => items[Math.min(items.length - 1, Math.max(0, Math.floor(random() * items.length)))];
  if (event === "subsidy") {
    for (const player of activePlayers) player.coins += 100;
    next.logs.push("随机事件：财政补贴");
  } else if (event === "demolish" || event === "dispute") {
    const property = pick(properties.filter((item) => item.level === "house"));
    if (property) {
      property.level = "empty";
      property.ownerDeviceId = null;
      property.sealedTurns = 0;
      property.tollMultiplier = 1;
    }
    next.logs.push(`随机事件：${event === "demolish" ? "拆迁令" : "产权纠纷"}`);
  } else if (event === "downgrade") {
    const property = pick(properties.filter((item) => item.level === "level2" || item.level === "level3"));
    if (property) property.level = property.level === "level3" ? "level2" : "house";
    next.logs.push("随机事件：楼市下调");
  } else if (event === "upgrade") {
    const property = pick(properties.filter((item) => item.level === "house" || item.level === "level2"));
    if (property) property.level = property.level === "house" ? "level2" : "level3";
    next.logs.push("随机事件：城市改造");
  } else if (event === "jail") {
    const target = pick(activePlayers.filter((player) => player.jailTurns === 0));
    if (target) return sendMonopolyPlayerToJail(next, target.deviceId);
    next.logs.push("随机事件：临时拘捕未找到目标");
  } else if (event === "rich_to_poor") {
    const richest = [...activePlayers].sort((a, b) => b.coins - a.coins)[0];
    const poorest = [...activePlayers].sort((a, b) => a.coins - b.coins)[0];
    if (richest && poorest && richest.deviceId !== poorest.deviceId) {
      const amount = Math.min(200, richest.coins);
      richest.coins -= amount;
      poorest.coins += amount;
    }
    next.logs.push("随机事件：富者济贫");
  } else if (event === "maintenance") {
    for (const player of activePlayers) {
      const count = properties.filter((property) => property.ownerDeviceId === player.deviceId && property.level !== "empty").length;
      player.coins = Math.max(0, player.coins - Math.min(250, count * 50));
    }
    next.logs.push("随机事件：维护支出");
  } else if (event === "takeover") {
    const property = pick(properties.filter((item) => item.ownerDeviceId));
    const target = pick(activePlayers.filter((player) => player.deviceId !== property?.ownerDeviceId));
    if (property && target) property.ownerDeviceId = target.deviceId;
    next.logs.push("随机事件：强制征收");
  } else if (event === "rent_holiday") {
    const property = pick(properties.filter((item) => item.level !== "empty"));
    const tile = property ? next.board[property.index] : undefined;
    if (tile?.kind === "property") {
      next.rentHolidayDistrict = tile.district;
      next.rentHolidayRounds = 1;
    }
    next.logs.push("随机事件：租金假日");
  }
  return next;
}

export function monopolyDirectPurchasePrice(level: MonopolyPropertyLevel): number {
  if (level === "house") return MONOPOLY_ECONOMY.houseDirectPurchase;
  if (level === "level2") return MONOPOLY_ECONOMY.level2DirectPurchase;
  if (level === "level3") return MONOPOLY_ECONOMY.level3DirectPurchase;
  return MONOPOLY_ECONOMY.emptyPurchase;
}

export function monopolyBuildingVariant(random: () => number = Math.random): number {
  return Math.max(0, Math.min(MONOPOLY_BUILDING_VARIANT_COUNT - 1, Math.floor(random() * MONOPOLY_BUILDING_VARIANT_COUNT)));
}

function grantStartReward(state: MonopolyState, player: MonopolyPlayer, random: () => number = Math.random): void {
  player.coins += 200;
  const candidates = Object.values(state.properties).filter((property) =>
    property.ownerDeviceId === player.deviceId && (property.level === "house" || property.level === "level2"),
  );
  if (candidates.length === 0) {
    state.logs.push(`${player.nickname} 经过起点，获得 200 金币`);
    return;
  }
  const selected = candidates[Math.floor(random() * candidates.length)]!;
  selected.level = selected.level === "house" ? "level2" : "level3";
  state.logs.push(`${player.nickname} 经过起点，获得 200 金币并升级一座建筑`);
}

function propertyToll(property: MonopolyPropertyState): number {
  const base = property.level === "house"
    ? MONOPOLY_ECONOMY.houseRent
    : property.level === "level2"
      ? MONOPOLY_ECONOMY.level2Rent
      : property.level === "level3"
        ? MONOPOLY_ECONOMY.level3Rent
        : 0;
  return base * property.tollMultiplier;
}

export function cloneMonopolyState(state: MonopolyState): MonopolyState {
  return {
    ...state,
    board: state.board.map((tile) => ({ ...tile })),
    players: state.players.map((player) => ({ ...player })),
    properties: Object.fromEntries(Object.entries(state.properties).map(([index, property]) => [index, { ...property }])),
    roadblocks: state.roadblocks.map((roadblock) => ({ ...roadblock })),
    godTokens: state.godTokens.map((token) => ({ ...token })),
    logs: [...state.logs],
  };
}

function refreshMonopolyGodTokens(state: MonopolyState, random: () => number): void {
  const candidates = state.board.filter((tile) => tile.kind !== "corner").map((tile) => tile.index);
  const gods: MonopolyGod[] = ["wealth", "poverty", "angel", "devil"];
  while (state.godTokens.length < 2 && candidates.length > 0) {
    const index = Math.max(0, Math.min(candidates.length - 1, Math.floor(random() * candidates.length)));
    const tileIndex = candidates.splice(index, 1)[0];
    if (tileIndex === undefined || state.godTokens.some((token) => token.index === tileIndex)) continue;
    const god = gods[Math.max(0, Math.min(gods.length - 1, Math.floor(random() * gods.length)))]!;
    state.godTokens.push({ index: tileIndex, god, expiresAtRound: state.completedRounds + 2 });
    state.logs.push(`${godLabel(god)} 出现在棋盘上`);
  }
}

function playerOf(state: MonopolyState, playerId: string): MonopolyPlayer | undefined {
  return state.players.find((player) => player.deviceId === playerId);
}

function propertyLabel(level: MonopolyPropertyLevel): string {
  if (level === "level2") return "洋房";
  if (level === "level3") return "地标";
  if (level === "house") return "小屋";
  return "空地";
}

function godCashAmount(startingCoins: number): number {
  return Math.round(Math.min(500, Math.max(100, startingCoins * 0.2)) / 50) * 50;
}

function godLabel(god: MonopolyGod): string {
  return { wealth: "财神", poverty: "穷鬼", angel: "天使", devil: "恶魔" }[god];
}

function failed(state: MonopolyState, error: string): MonopolyActionResult {
  return { ok: false, state, error };
}

function normalizeStartingCoins(value: number): number {
  const normalized = Math.round(value / 1000) * 1000;
  return Math.min(50_000, Math.max(5_000, normalized));
}

function normalizeMaxRounds(value: number): number {
  return Math.min(50, Math.max(5, Math.round(value)));
}

function monopolyCatchupScore(state: MonopolyState, playerId: string): number {
  const player = state.players.find((item) => item.deviceId === playerId);
  const active = state.players.filter((item) => !item.eliminated);
  if (!player || active.length <= 1) return 0;
  const medianCoins = median(active.map((item) => item.coins));
  const ownedBuildings = active.map((item) => ownedBuildingCount(state, item.deviceId));
  const medianBuildings = median(ownedBuildings);
  let score = 0;
  if (player.coins <= medianCoins * 0.4) score += 2;
  else if (player.coins <= medianCoins * 0.7) score += 1;
  const buildings = ownedBuildingCount(state, playerId);
  if (medianBuildings > 0) {
    if (buildings <= medianBuildings * 0.5) score += 2;
    else if (buildings < medianBuildings) score += 1;
  }
  return score;
}

function ownedBuildingCount(state: MonopolyState, playerId: string): number {
  return Object.values(state.properties).filter((property) => property.ownerDeviceId === playerId && property.level !== "empty").length;
}

function selectMonopolyCard(state: MonopolyState, playerId: string, random: () => number, openingDraw: boolean): MonopolyCard | null {
  const player = state.players.find((item) => item.deviceId === playerId);
  if (!player) return null;
  const hasStrong = player.cards.some((card) => strongCards.has(card));
  const weights = monopolyCardWeights(state, playerId);
  const candidates = (Object.keys(weights) as MonopolyCard[]).filter((card) =>
    (!openingDraw || !player.cards.includes(card)) && (!hasStrong || !strongCards.has(card)),
  );
  const total = candidates.reduce((sum, card) => sum + weights[card], 0);
  if (total <= 0) return null;
  let needle = Math.max(0, Math.min(0.999999, random())) * total;
  for (const card of candidates) {
    needle -= weights[card];
    if (needle <= 0) return card;
  }
  return candidates[candidates.length - 1] ?? null;
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) return 0;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[middle - 1]! + sorted[middle]!) / 2 : sorted[middle]!;
}

function seededRandom(seed: number): () => number {
  let value = Math.floor(seed) >>> 0;
  return () => {
    value = (value * 1664525 + 1013904223) >>> 0;
    return value / 0x100000000;
  };
}

function propertyDistrictIndex(index: number): number {
  const side = Math.floor(index / 10);
  const offset = index % 10;
  return side * 2 + (offset < 5 ? 0 : 1);
}
