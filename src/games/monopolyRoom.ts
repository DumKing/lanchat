import {
  createMonopolyState,
  endMonopolyTurn,
  moveMonopolyPlayer,
  purchaseMonopolyProperty,
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
  chatMessages: Array<{ id: string; senderDeviceId: string; sender: string; content: string; createdAt: number }>;
  logs: string[];
  updatedAt: number;
};

export type MonopolyRoomAction =
  | { action: "join"; player: MonopolyRoomSeat }
  | { action: "ready"; playerId: string; ready: boolean }
  | { action: "start"; playerId: string }
  | { action: "roll"; playerId: string }
  | { action: "end_turn"; playerId: string }
  | { action: "buy"; playerId: string; propertyIndex: number }
  | { action: "upgrade"; playerId: string; propertyIndex: number }
  | { action: "card"; playerId: string; card: MonopolyCard; target?: MonopolyCardTarget }
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
    const dice = player?.forcedDice ?? Math.floor(random() * 6) + 1;
    if (player) player.forcedDice = undefined;
    const result = moveMonopolyPlayer(next.game, action.playerId, dice);
    if (!result.ok) return state;
    next.game = result.state;
    next.logs.push(`${player?.nickname ?? "玩家"} 掷出了 ${dice}`);
    return touch(next);
  }
  if (action.action === "buy") {
    const result = purchaseMonopolyProperty(next.game, action.playerId, action.propertyIndex);
    if (!result.ok) return state;
    next.game = result.state;
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
  if (action.action === "end_turn") {
    next.game = endMonopolyTurn(next.game);
    if (next.game.completedRounds >= next.game.maxRounds) next.phase = "ended";
    return touch(next);
  }
  return state;
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
