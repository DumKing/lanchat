import {
  applyMonopolyPriceDouble,
  applyMonopolyRandomEvent,
  collectMonopolyGodToken,
  createMonopolyState,
  declareMonopolyBankruptcy,
  discardMonopolyCard,
  endMonopolyTurn,
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

export type MonopolyRoomState = {
  roomId: string;
  phase: "lobby" | "playing" | "ended";
  seats: MonopolyRoomSeat[];
  game: MonopolyState;
  winnerDeviceId?: string;
  /** 掷骰落地后需要玩家选择的补充操作。 */
  pendingLanding?: { playerId: string; kind: "buy" | "airport"; index: number };
  turnRolled: boolean;
  chatMessages: Array<{ id: string; senderDeviceId: string; sender: string; content: string; mine?: boolean; createdAt: number }>;
  logs: string[];
  updatedAt: number;
};

export type MonopolyRoomAction =
  | { action: "join"; player: MonopolyRoomSeat }
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
    game: createMonopolyState([input.host], { startingCoins: input.startingCoins, maxRounds: input.maxRounds, now: input.now }),
    turnRolled: false,
    chatMessages: [],
    logs: [`${input.host.nickname} 创建了大富翁房间`],
    updatedAt: input.now ?? Date.now(),
  };
}

export function applyMonopolyRoomAction(state: MonopolyRoomState, action: MonopolyRoomAction, random: () => number = Math.random): MonopolyRoomState {
  const next = cloneRoomState(state);
  if (action.action === "join") {
    if (next.phase !== "lobby" || next.seats.length >= 4 || next.seats.some((seat) => seat.deviceId === action.player.deviceId)) return state;
    next.seats.push(action.player);
    next.game = createMonopolyState(next.seats, { startingCoins: next.game.startingCoins, maxRounds: next.game.maxRounds });
    next.logs.push(`${action.player.nickname} 加入房间`);
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
    next.logs.push("所有玩家已准备，游戏开始");
    return touch(next);
  }
  if (action.action === "chat") {
    next.chatMessages.push(action.message);
    next.chatMessages = next.chatMessages.slice(-80);
    return touch(next);
  }
  if (action.action === "leave") {
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
      if (player.jailTurns === 0) {
        const release = teleportMonopolyPlayer(next.game, action.playerId, 0, random);
        if (release.ok) next.game = release.state;
        next.logs.push(`${player.nickname} 刑满回到起点`);
      } else {
        next.logs.push(`${player.nickname} 正在监狱中，还需 ${player.jailTurns} 回合`);
      }
      return touch(next);
    }
    const dice = player?.forcedDice ?? Math.floor(random() * 6) + 1;
    if (player) player.forcedDice = undefined;
    const result = moveMonopolyPlayer(next.game, action.playerId, dice, random);
    if (!result.ok) return state;
    next.game = result.state;
    next.logs.push(`${player?.nickname ?? "玩家"} 掷出了 ${dice}`);
    next.turnRolled = true;
    settleLanding(next, action.playerId, random);
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
    return touch(next);
  }
  if (action.action === "skip_landing") {
    if (!next.pendingLanding || next.pendingLanding.playerId !== action.playerId) return state;
    next.logs.push(`${next.game.players.find((item) => item.deviceId === action.playerId)?.nickname ?? "玩家"} 放弃了落地操作`);
    next.pendingLanding = undefined;
    return touch(next);
  }
  if (action.action === "buy") {
    const pending = next.pendingLanding;
    if (!pending || pending.playerId !== action.playerId || pending.kind !== "buy" || pending.index !== action.propertyIndex) return state;
    const result = purchaseMonopolyProperty(next.game, action.playerId, action.propertyIndex);
    if (!result.ok) return state;
    next.game = result.state;
    next.pendingLanding = undefined;
    return touch(next);
  }
  if (action.action === "upgrade") {
    const result = upgradeMonopolyProperty(next.game, action.playerId, action.propertyIndex);
    if (!result.ok) return state;
    next.game = result.state;
    return touch(next);
  }
  if (action.action === "card") {
    const result = useMonopolyCard(next.game, action.playerId, action.card, action.target, random);
    if (!result.ok) return state;
    next.game = result.state;
    return touch(next);
  }
  if (action.action === "discard") {
    const game = discardMonopolyCard(next.game, action.playerId, action.card);
    if (game === next.game) return state;
    next.game = game;
    return touch(next);
  }
  if (action.action === "end_turn") {
    if (!next.turnRolled || next.pendingLanding) return state;
    next.game = endMonopolyTurn(next.game, Date.now(), random);
    next.turnRolled = false;
    if (next.game.completedRounds >= next.game.maxRounds || next.game.players.filter((player) => !player.eliminated).length <= 1) finishRoom(next);
    return touch(next);
  }
  return state;
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
  const winner = [...state.game.players].sort((a, b) => b.coins - a.coins || a.nickname.localeCompare(b.nickname))[0];
  state.winnerDeviceId = winner?.deviceId;
  state.logs.push(`${winner?.nickname ?? "玩家"} 以现金排名第一，游戏结束`);
}

function cloneRoomState(state: MonopolyRoomState): MonopolyRoomState {
  return {
    ...state,
    seats: state.seats.map((seat) => ({ ...seat })),
    game: structuredClone(state.game),
    chatMessages: state.chatMessages.map((message) => ({ ...message })),
    logs: [...state.logs],
  };
}

function touch(state: MonopolyRoomState): MonopolyRoomState {
  state.updatedAt = Date.now();
  return state;
}
