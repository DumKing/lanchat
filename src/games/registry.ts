export type GameType = "doudizhu" | "gomoku" | "xiangqi" | "minesweeper";

export type GameDefinition = {
  type: GameType;
  name: string;
  description: string;
  minPlayers: number;
  maxPlayers: number;
  icon: string;
  status: "available" | "planned";
};

export type GameRoomPlayer = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  online: boolean;
  ready: boolean;
};

export type GameRoomShell = {
  roomId: string;
  gameType: GameType;
  roomName: string;
  hostDeviceId: string;
  hostName: string;
  players: GameRoomPlayer[];
  createdAt: number;
  updatedAt: number;
};

export const gameRegistry: GameDefinition[] = [
  {
    type: "doudizhu",
    name: "斗地主",
    description: "三人局域网娱乐房间，支持房间聊天和实时同步。",
    minPlayers: 3,
    maxPlayers: 3,
    icon: "斗",
    status: "available",
  },
  {
    type: "gomoku",
    name: "五子棋",
    description: "双人局域网棋盘对战，黑白轮流落子，五连即胜。",
    minPlayers: 2,
    maxPlayers: 2,
    icon: "五",
    status: "available",
  },
  {
    type: "minesweeper",
    name: "扫雷竞速",
    description: "单人或多人同图竞速扫雷，先清完非雷格获胜，支持九宫格双击展开。",
    minPlayers: 1,
    maxPlayers: 6,
    icon: "雷",
    status: "available",
  },
  {
    type: "xiangqi",
    name: "中国象棋",
    description: "双人局域网象棋对局，红黑轮流走子，支持完整基础走法。",
    minPlayers: 2,
    maxPlayers: 2,
    icon: "象",
    status: "available",
  },
];

export function gameDefinitionOf(type: GameType) {
  return gameRegistry.find((game) => game.type === type) ?? gameRegistry[0];
}

export function createGameRoomShell(
  gameType: GameType,
  roomName: string,
  hostDeviceId: string,
  hostName: string,
  avatar?: string | null,
): GameRoomShell {
  const now = Date.now();
  return {
    roomId: `${gameType}-${now}-${Math.random().toString(16).slice(2, 8)}`,
    gameType,
    roomName: roomName.trim() || `${gameDefinitionOf(gameType).name}房间`,
    hostDeviceId,
    hostName,
    players: [{ deviceId: hostDeviceId, nickname: hostName, avatar, online: true, ready: false }],
    createdAt: now,
    updatedAt: now,
  };
}











