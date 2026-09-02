import {
  applyMonopolyPriceDouble,
  applyMonopolyRandomEvent,
  collectMonopolyGodToken,
  cloneMonopolyState,
  createMonopolyState,
  declareMonopolyBankruptcy,
  discardMonopolyCard,
  endMonopolyTurn,
  monopolyDirectPurchasePrice,
  monopolyLandingRent,
  moveMonopolyPlayer,
  purchaseMonopolyProperty,
  resolveMonopolyLanding,
  sendMonopolyPlayerToJail,
  teleportMonopolyPlayer,
  type MonopolyCard,
  type MonopolyCardTarget,
  type MonopolyPlayerSeed,
  type MonopolyState,
  upgradeMonopolyProperty,
  useMonopolyCard,
} from "./monopoly";

export type MonopolyRoomSeat = MonopolyPlayerSeed & {
  online: boolean;
  ready: boolean;
};

export type MonopolyDiceRoll = {
  playerId: string;
  nickname: string;
  values: number[];
  total: number;
  fixed: boolean;
  rolledAt: number;
};

export type MonopolyRoomAnnouncement = {
  id: string;
  kind: "event" | "card";
  text: string;
  createdAt: number;
};

export type MonopolyRoomState = {
  roomId: string;
  phase: "lobby" | "playing" | "ended";
  seats: MonopolyRoomSeat[];
  /** 已开局或座位已满后进入房间的成员，只同步局面与聊天，不参与骰序。 */
  spectators: MonopolyRoomSeat[];
  game: MonopolyState;
  winnerDeviceId?: string;
  /** 掷骰落地后需要玩家选择的补充操作。 */
  pendingLanding?: { playerId: string; kind: "buy" | "airport"; index: number };
  turnRolled: boolean;
  /** 最近一次投骰结果，所有房间成员都可见。 */
  lastDice?: MonopolyDiceRoll;
  /** 掷出 12 点并完成落地结算后，当前玩家可再次投骰。 */
  extraRollAvailable: boolean;
  /** 最近一次需要向全房间强调的随机事件或道具操作。 */
  lastAnnouncement?: MonopolyRoomAnnouncement;
  chatMessages: Array<{ id: string; senderDeviceId: string; sender: string; content: string; mine?: boolean; createdAt: number }>;
  logs: string[];
  updatedAt: number;
};

export type MonopolyRoomAction =
  | { action: "join"; player: MonopolyRoomSeat }
  | { action: "add_bot"; hostId: string; bot: MonopolyRoomSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "start"; playerId: string }
  | { action: "roll"; playerId: string }
  | { action: "airport"; playerId: string; targetIndex: number }
  | { action: "skip_landing"; playerId: string }
  | { action: "end_turn"; playerId: string }
  | { action: "buy"; playerId: string; propertyIndex: number }
  | { action: "upgrade"; playerId: string; propertyIndex: number }
  | { action: "card"; playerId: string; card: MonopolyCard; target?: MonopolyCardTarget }
  | { action: "discard"; playerId: string; card: MonopolyCard }
  | { action: "chat"; message: MonopolyRoomState["chatMessages"][number] }
  | { action: "leave"; playerId: string };

export function createMonopolyRoomState(input: {
  roomId: string;
  host: MonopolyRoomSeat;
  startingCoins?: number;
  maxRounds?: number;
  now?: number;
}): MonopolyRoomState {
  return {
    roomId: input.roomId,
    phase: "lobby",
    seats: [input.host],
    spectators: [],
    game: createMonopolyState([input.host], { startingCoins: input.startingCoins, maxRounds: input.maxRounds, now: input.now }),
    turnRolled: false,
    extraRollAvailable: false,
    chatMessages: [],
    logs: [`${input.host.nickname} 创建了大富翁房间`],
    updatedAt: input.now ?? Date.now(),
  };
}

export function applyMonopolyRoomAction(state: MonopolyRoomState, action: MonopolyRoomAction, random: () => number = Math.random): MonopolyRoomState {
  const next = cloneRoomState(state);
  if (action.action === "add_bot") {
    if (next.phase !== "lobby" || next.seats[0]?.deviceId !== action.hostId || next.seats.length >= 4 || !action.bot.isBot || !action.bot.deviceId.startsWith("bot:")) return state;
    if (next.seats.some((seat) => seat.deviceId === action.bot.deviceId)) return state;
    next.seats.push({ ...action.bot, online: true, ready: true, isBot: true });
    next.game = createMonopolyState(next.seats, { startingCoins: next.game.startingCoins, maxRounds: next.game.maxRounds });
    next.logs.push(`${action.bot.nickname} 加入房间并自动准备`);
    return touch(next);
  }
  if (action.action === "join") {
    if (next.seats.some((seat) => seat.deviceId === action.player.deviceId) || next.spectators.some((spectator) => spectator.deviceId === action.player.deviceId)) return state;
    if (next.phase === "lobby" && next.seats.length < 4) {
      next.seats.push(action.player);
      next.game = createMonopolyState(next.seats, { startingCoins: next.game.startingCoins, maxRounds: next.game.maxRounds });
      next.logs.push(`${action.player.nickname} 加入房间`);
    } else {
      next.spectators.push(action.player);
      next.logs.push(`${action.player.nickname} 进入观战`);
    }
    return touch(next);
  }
  if (action.action === "ready") {
    if (next.phase !== "lobby") return state;
    const seat = next.seats.find((item) => item.deviceId === action.playerId);
    if (!seat) return state;
    seat.ready = action.ready;
    return touch(next);
  }
  if (action.action === "start") {
    if (next.phase !== "lobby" || next.seats[0]?.deviceId !== action.playerId || next.seats.length < 2 || !next.seats.every((seat) => seat.ready)) return state;
    next.phase = "playing";
    next.game.turnStartedAt = Date.now();
    next.turnRolled = false;
    next.extraRollAvailable = false;
    next.lastDice = undefined;
    next.logs.push("所有玩家已准备，游戏开始");
    return touch(next);
  }
  if (action.action === "chat") {
    next.chatMessages.push(action.message);
    next.chatMessages = next.chatMessages.slice(-80);
    return touch(next);
  }
  if (action.action === "leave") {
    const spectatorIndex = next.spectators.findIndex((spectator) => spectator.deviceId === action.playerId);
    if (spectatorIndex >= 0) {
      next.spectators.splice(spectatorIndex, 1);
      next.logs.push("一位观众离开房间");
      return touch(next);
    }
    next.seats = next.seats.filter((seat) => seat.deviceId !== action.playerId);
    if (next.phase === "playing") {
      next.phase = "ended";
      next.logs.push("有玩家退出，游戏结束");
    }
    return touch(next);
  }
  if (next.phase !== "playing" || next.game.currentPlayerId !== action.playerId) return state;
  if (action.action === "roll") {
    const player = next.game.players.find((item) => item.deviceId === action.playerId);
    if (!player || next.turnRolled || next.pendingLanding) return state;
    if (player.jailTurns > 0) {
      player.jailTurns -= 1;
      next.turnRolled = true;
      next.extraRollAvailable = false;
      if (player.jailTurns === 0) {
        const release = teleportMonopolyPlayer(next.game, action.playerId, 0, random);
        if (release.ok) next.game = release.state;
        next.logs.push(`${player.nickname} 刑满回到起点`);
      } else {
        next.logs.push(`${player.nickname} 正在监狱中，还需 ${player.jailTurns} 回合`);
      }
      finishResolvedLanding(next, action.playerId, random);
      return touch(next);
    }
    const fixedDice = player.forcedDice;
    const diceValues = fixedDice === undefined
      ? [Math.floor(random() * 6) + 1, Math.floor(random() * 6) + 1]
      : [fixedDice];
    const dice = diceValues.reduce((total, value) => total + value, 0);
    if (player) player.forcedDice = undefined;
    const result = moveMonopolyPlayer(next.game, action.playerId, dice, random);
    if (!result.ok) return state;
    next.game = result.state;
    next.lastDice = { playerId: action.playerId, nickname: player.nickname, values: diceValues, total: dice, fixed: fixedDice !== undefined, rolledAt: Date.now() };
    next.extraRollAvailable = dice === 12;
    next.logs.push(`${player.nickname} 掷出了 ${diceValues.join(" + ")} = ${dice}`);
    next.turnRolled = true;
    settleLanding(next, action.playerId, random);
    finishResolvedLanding(next, action.playerId, random);
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  if (action.action === "airport") {
    const pending = next.pendingLanding;
    if (!pending || pending.playerId !== action.playerId || pending.kind !== "airport") return state;
    const result = teleportMonopolyPlayer(next.game, action.playerId, action.targetIndex, random);
    if (!result.ok) return state;
    next.game = result.state;
    next.pendingLanding = undefined;
    settleLanding(next, action.playerId, random);
    finishResolvedLanding(next, action.playerId, random);
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  if (action.action === "skip_landing") {
    if (!next.pendingLanding || next.pendingLanding.playerId !== action.playerId) return state;
    next.logs.push(`${next.game.players.find((item) => item.deviceId === action.playerId)?.nickname ?? "玩家"} 放弃了落地操作`);
    next.pendingLanding = undefined;
    finishResolvedLanding(next, action.playerId, random, true);
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  if (action.action === "buy") {
    const pending = next.pendingLanding;
    if (!pending || pending.playerId !== action.playerId || pending.kind !== "buy" || pending.index !== action.propertyIndex) return state;
    const result = purchaseMonopolyProperty(next.game, action.playerId, action.propertyIndex, random);
    if (!result.ok) return state;
    next.game = result.state;
    next.pendingLanding = undefined;
    finishResolvedLanding(next, action.playerId, random, true);
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  // 升级由落地结算自动完成，拒绝旧客户端的手动升级请求，避免一次落地重复升级。
  if (action.action === "upgrade") return state;
  if (action.action === "card") {
    if (next.turnRolled || next.pendingLanding) return state;
    const result = useMonopolyCard(next.game, action.playerId, action.card, action.target, random);
    if (!result.ok) return state;
    next.game = result.state;
    announceCardUse(next, action);
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  if (action.action === "discard") {
    if (next.turnRolled || next.pendingLanding) return state;
    const game = discardMonopolyCard(next.game, action.playerId, action.card);
    if (game === next.game) return state;
    next.game = game;
    resetMonopolyActionDeadline(next);
    return touch(next);
  }
  if (action.action === "end_turn") {
    if (!next.turnRolled || next.pendingLanding) return state;
    if (next.extraRollAvailable) {
      openExtraRoll(next, action.playerId);
    } else {
      advanceMonopolyTurn(next, random);
    }
    return touch(next);
  }
  return state;
}

/** 机器人只由房主调用；决策返回普通房间动作，仍由同一权威状态机校验。 */
export function planMonopolyBotAction(state: MonopolyRoomState, botId: string, random: () => number = Math.random): MonopolyRoomAction | null {
  if (state.phase !== "playing" || state.game.currentPlayerId !== botId) return null;
  const bot = state.game.players.find((player) => player.deviceId === botId && player.isBot && !player.eliminated);
  if (!bot) return null;
  const pending = state.pendingLanding;
  if (pending?.playerId === botId) {
    if (pending.kind === "airport") {
      const candidates = state.game.board
        .filter((tile) => tile.index !== pending.index)
        .map((tile) => tile.index);
      return { action: "airport", playerId: botId, targetIndex: candidates[Math.floor(random() * candidates.length)] ?? 0 };
    }
    const property = state.game.properties[pending.index];
    if (property && !property.ownerDeviceId && bot.coins >= monopolyDirectPurchasePrice(property.level)) {
      return { action: "buy", playerId: botId, propertyIndex: pending.index };
    }
    return { action: "skip_landing", playerId: botId };
  }
  if (!state.turnRolled) return { action: "roll", playerId: botId };
  return { action: "end_turn", playerId: botId };
}

/** 落地强制结算完成后自动交棒；自己的可升级地产会在此处免费升一级。 */
function finishResolvedLanding(state: MonopolyRoomState, playerId: string, random: () => number, landingActionCompleted = false): void {
  if (state.phase !== "playing" || state.pendingLanding) return;
  const player = state.game.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) {
    advanceMonopolyTurn(state, random);
    return;
  }
  if (landingActionCompleted) {
    if (state.extraRollAvailable) {
      openExtraRoll(state, playerId);
    } else {
      advanceMonopolyTurn(state, random);
    }
    return;
  }
  const property = state.game.properties[player.position];
  const canUpgradeCurrentProperty = property?.ownerDeviceId === playerId && (property.level === "house" || property.level === "level2");
  if (canUpgradeCurrentProperty) {
    const result = upgradeMonopolyProperty(state.game, playerId, player.position, random);
    if (result.ok) state.game = result.state;
  }
  if (state.extraRollAvailable) {
    openExtraRoll(state, playerId);
    return;
  }
  advanceMonopolyTurn(state, random);
}

function openExtraRoll(state: MonopolyRoomState, playerId: string): void {
  state.turnRolled = false;
  resetMonopolyActionDeadline(state);
  const player = state.game.players.find((item) => item.deviceId === playerId);
  state.logs.push(`${player?.nickname ?? "玩家"} 掷出 12 点，获得一次额外投骰机会`);
}

function resetMonopolyActionDeadline(state: MonopolyRoomState): void {
  state.game.turnStartedAt = Date.now();
}

function advanceMonopolyTurn(state: MonopolyRoomState, random: () => number): void {
  state.game = endMonopolyTurn(state.game, Date.now(), random);
  state.turnRolled = false;
  state.extraRollAvailable = false;
  if (state.game.completedRounds >= state.game.maxRounds || state.game.players.filter((player) => !player.eliminated).length <= 1) finishRoom(state);
}

function settleLanding(state: MonopolyRoomState, playerId: string, random: () => number): void {
  const player = state.game.players.find((item) => item.deviceId === playerId);
  if (!player || player.eliminated) return;
  const index = player.position;
  state.game = collectMonopolyGodToken(state.game, playerId, index);
  state.game = resolveMonopolyLanding(state.game, playerId, index, random);
  const tile = state.game.board[index];
  if (!tile) return;
  if (tile.kind === "event") {
    const events = ["demolish", "downgrade", "takeover", "jail", "subsidy", "rich_to_poor", "upgrade", "maintenance", "dispute", "rent_holiday"] as const;
    state.game = applyMonopolyRandomEvent(state.game, events[Math.floor(random() * events.length)]!, random);
    announceLatestGameLog(state, "event");
    return;
  }
  if (tile.kind === "corner") {
    if (tile.corner === "airport") {
      state.pendingLanding = { playerId, kind: "airport", index };
    } else if (tile.corner === "price_double") {
      state.game = applyMonopolyPriceDouble(state.game, playerId, random);
    } else if (tile.corner === "jail") {
      state.game = sendMonopolyPlayerToJail(state.game, playerId);
    }
    return;
  }
  const property = state.game.properties[index];
  if (!property) return;
  if (!property.ownerDeviceId) {
    state.pendingLanding = { playerId, kind: "buy", index };
    return;
  }
  if (property.ownerDeviceId === playerId) return;
  const rent = monopolyLandingRent(state.game, playerId, index);
  if (rent <= 0) return;
  const payer = state.game.players.find((item) => item.deviceId === playerId)!;
  const owner = state.game.players.find((item) => item.deviceId === property.ownerDeviceId);
  const paid = Math.min(rent, payer.coins);
  payer.coins -= paid;
  if (owner) owner.coins += paid;
  state.game.logs.push(`${payer.nickname} 支付过路费 ${paid}`);
  if (paid < rent || payer.coins <= 0) state.game = declareMonopolyBankruptcy(state.game, playerId);
}

function finishRoom(state: MonopolyRoomState): void {
  state.phase = "ended";
  state.pendingLanding = undefined;
  state.turnRolled = false;
  state.extraRollAvailable = false;
  const winner = [...state.game.players].sort((a, b) => b.coins - a.coins || a.nickname.localeCompare(b.nickname))[0];
  state.winnerDeviceId = winner?.deviceId;
  state.logs.push(`${winner?.nickname ?? "玩家"} 以现金排名第一，游戏结束`);
}

function cloneRoomState(state: MonopolyRoomState): MonopolyRoomState {
  return {
    ...state,
    seats: state.seats.map((seat) => ({ ...seat })),
    spectators: (state.spectators ?? []).map((spectator) => ({ ...spectator })),
    game: cloneMonopolyState(state.game),
    lastDice: state.lastDice ? { ...state.lastDice, values: [...state.lastDice.values] } : undefined,
    lastAnnouncement: state.lastAnnouncement ? { ...state.lastAnnouncement } : undefined,
    chatMessages: state.chatMessages.map((message) => ({ ...message })),
    logs: [...state.logs],
  };
}

function announceLatestGameLog(state: MonopolyRoomState, kind: MonopolyRoomAnnouncement["kind"]): void {
  const text = state.game.logs[state.game.logs.length - 1];
  if (!text) return;
  state.lastAnnouncement = {
    id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    kind,
    text,
    createdAt: Date.now(),
  };
}

function announceCardUse(state: MonopolyRoomState, action: Extract<MonopolyRoomAction, { action: "card" }>): void {
  const actor = state.game.players.find((player) => player.deviceId === action.playerId);
  if (!actor) return;
  const cardName: Record<MonopolyCard, string> = {
    acquittal: "免罪卡",
    seize: "抢占卡",
    frame: "陷害卡",
    double: "翻倍卡",
    fixed_dice: "指定卡",
    roadblock: "路障卡",
    turtle: "乌龟卡",
    reverse: "转向卡",
    loot: "掠夺卡",
    seal: "查封卡",
  };
  const targetPlayer = action.target?.playerId
    ? state.game.players.find((player) => player.deviceId === action.target?.playerId)
    : undefined;
  const targetTile = action.target?.propertyIndex ?? action.target?.index;
  const target = targetPlayer
    ? `对 ${targetPlayer.nickname}`
    : Number.isInteger(targetTile)
      ? `在 ${Number(targetTile) + 1} 号地块`
      : "";
  state.lastAnnouncement = {
    id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
    kind: "card",
    text: `${actor.nickname}${target} 使用了${cardName[action.card]}`,
    createdAt: Date.now(),
  };
}

function touch(state: MonopolyRoomState): MonopolyRoomState {
  state.updatedAt = Date.now();
  return state;
}
