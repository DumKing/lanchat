export type DdzPhase = "lobby" | "bidding" | "playing" | "ended";
export const DDZ_TURN_TIMEOUT_MS = 60_000;

export function turnRemainingSeconds(turnStartedAt: number | undefined, now = Date.now(), timeoutMs = DDZ_TURN_TIMEOUT_MS) {
  if (!turnStartedAt) return Math.ceil(timeoutMs / 1000);
  return Math.max(0, Math.ceil((timeoutMs - (now - turnStartedAt)) / 1000));
}

export function isTurnTimedOut(turnStartedAt: number | undefined, now = Date.now(), timeoutMs = DDZ_TURN_TIMEOUT_MS) {
  return turnRemainingSeconds(turnStartedAt, now, timeoutMs) <= 0;
}

export type DdzSuit = "spade" | "heart" | "club" | "diamond" | "joker";
export type DdzPlayCategory =
  | "single"
  | "pair"
  | "triple"
  | "triple_single"
  | "triple_pair"
  | "straight"
  | "double_straight"
  | "airplane"
  | "airplane_single"
  | "airplane_pair"
  | "four_two_single"
  | "four_two_pair"
  | "bomb"
  | "rocket";

export type DdzCard = {
  id: string;
  rank: string;
  suit: DdzSuit;
  label: string;
  value: number;
  red: boolean;
};

export type DdzPlayer = {
  deviceId: string;
  nickname: string;
  avatar?: string | null;
  ready: boolean;
  role?: "landlord" | "farmer";
  online: boolean;
  handCount: number;
};

export type DdzPlayInfo = {
  category: DdzPlayCategory;
  value: number;
  length: number;
  mainLength?: number;
};

export type DdzPlay = DdzPlayInfo & {
  playerId: string;
  playerName: string;
  cards: DdzCard[];
};

export type DdzChatMessage = {
  id: string;
  senderDeviceId: string;
  senderName: string;
  content: string;
  createdAt: number;
};

export type DdzRoom = {
  roomId: string;
  name: string;
  hostDeviceId: string;
  hostName: string;
  players: DdzPlayer[];
  phase: DdzPhase;
  landlordCards: DdzCard[];
  myHand: DdzCard[];
  handCounts: Record<string, number>;
  turnDeviceId?: string;
  landlordDeviceId?: string;
  bidOrder: string[];
  bidIndex: number;
  bids: Record<string, boolean>;
  lastPlay?: DdzPlay | null;
  passCount: number;
  winnerRole?: "landlord" | "farmer";
  logs: string[];
  chatMessages: DdzChatMessage[];
  updatedAt: number;
};

export type DdzAction =
  | { type: "join_request"; roomId: string }
  | { type: "ready"; roomId: string; ready: boolean }
  | { type: "bid"; roomId: string; call: boolean }
  | { type: "play"; roomId: string; cardIds: string[] }
  | { type: "pass"; roomId: string }
  | { type: "chat"; roomId: string; content: string };

const rankDefs = [
  ["3", 3], ["4", 4], ["5", 5], ["6", 6], ["7", 7], ["8", 8], ["9", 9], ["10", 10],
  ["J", 11], ["Q", 12], ["K", 13], ["A", 14], ["2", 16],
] as const;
const suits: Array<{ suit: DdzSuit; red: boolean }> = [
  { suit: "spade", red: false },
  { suit: "heart", red: true },
  { suit: "club", red: false },
  { suit: "diamond", red: true },
];

export function createDeck() {
  const deck: DdzCard[] = [];
  for (const [rank, value] of rankDefs) {
    for (const suit of suits) {
      deck.push({ id: `${rank}-${suit.suit}`, rank, suit: suit.suit, label: rank, value, red: suit.red });
    }
  }
  deck.push({ id: "joker-small", rank: "SJ", suit: "joker", label: "小", value: 17, red: true });
  deck.push({ id: "joker-big", rank: "BJ", suit: "joker", label: "大", value: 18, red: false });
  return deck;
}

export function shuffleDeck(deck: DdzCard[]) {
  const next = [...deck];
  for (let i = next.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [next[i], next[j]] = [next[j], next[i]];
  }
  return next;
}

export function sortCards(cards: DdzCard[]) {
  return [...cards].sort((a, b) => a.value - b.value || a.id.localeCompare(b.id));
}

function countByValue(cards: DdzCard[]) {
  const map = new Map<number, DdzCard[]>();
  for (const card of cards) {
    map.set(card.value, [...(map.get(card.value) ?? []), card]);
  }
  return [...map.entries()].sort(([a], [b]) => a - b);
}

function isConsecutive(values: number[]) {
  if (values.length === 0 || values.some((value) => value >= 16)) return false;
  return values.every((value, index) => index === 0 || value === values[index - 1] + 1);
}

function makeInfo(category: DdzPlayCategory, value: number, length: number, mainLength?: number): DdzPlayInfo {
  return { category, value, length, mainLength };
}

export function evaluatePlay(cards: DdzCard[]): DdzPlayInfo | null {
  const sorted = sortCards(cards);
  const length = sorted.length;
  if (length === 0) return null;
  const groups = countByValue(sorted);
  const counts = groups.map(([value, grouped]) => ({ value, count: grouped.length }));
  const countValues = counts.map((item) => item.count).sort((a, b) => b - a);

  if (length === 1) return makeInfo("single", sorted[0].value, length);
  if (length === 2 && sorted.every((card) => card.suit === "joker")) return makeInfo("rocket", 99, length);
  if (length === 2 && counts.length === 1) return makeInfo("pair", counts[0].value, length);
  if (length === 3 && counts.length === 1) return makeInfo("triple", counts[0].value, length);
  if (length === 4 && counts.length === 1) return makeInfo("bomb", counts[0].value, length);

  if (length === 4 && countValues.join(",") === "3,1") {
    const main = counts.find((item) => item.count === 3)!;
    return makeInfo("triple_single", main.value, length);
  }
  if (length === 5 && countValues.join(",") === "3,2") {
    const main = counts.find((item) => item.count === 3)!;
    return makeInfo("triple_pair", main.value, length);
  }

  if (length >= 5 && counts.every((item) => item.count === 1) && isConsecutive(counts.map((item) => item.value))) {
    return makeInfo("straight", counts[counts.length - 1]!.value, length, length);
  }
  if (length >= 6 && length % 2 === 0 && counts.every((item) => item.count === 2) && isConsecutive(counts.map((item) => item.value))) {
    return makeInfo("double_straight", counts[counts.length - 1]!.value, length, length / 2);
  }

  const triples = counts.filter((item) => item.count === 3).map((item) => item.value);
  if (triples.length >= 2 && isConsecutive(triples)) {
    const wingCount = length - triples.length * 3;
    const maxTriple = triples[triples.length - 1]!;
    if (wingCount === 0) return makeInfo("airplane", maxTriple, length, triples.length);
    if (wingCount === triples.length) return makeInfo("airplane_single", maxTriple, length, triples.length);
    const nonTriples = counts.filter((item) => !triples.includes(item.value));
    if (wingCount === triples.length * 2 && nonTriples.every((item) => item.count === 2)) {
      return makeInfo("airplane_pair", maxTriple, length, triples.length);
    }
  }

  if (length === 6 && countValues.join(",") === "4,1,1") {
    const main = counts.find((item) => item.count === 4)!;
    return makeInfo("four_two_single", main.value, length);
  }
  if (length === 8 && countValues.join(",") === "4,2,2") {
    const main = counts.find((item) => item.count === 4)!;
    const pairs = counts.filter((item) => item.count === 2);
    if (pairs.length === 2) return makeInfo("four_two_pair", main.value, length);
  }

  return null;
}

export function playLabel(play?: DdzPlayInfo | null) {
  if (!play) return "请选择合法牌型";
  const labels: Record<DdzPlayCategory, string> = {
    single: "单牌",
    pair: "对子",
    triple: "三张",
    triple_single: "三带一",
    triple_pair: "三带二",
    straight: "顺子",
    double_straight: "连对",
    airplane: "飞机",
    airplane_single: "飞机带单",
    airplane_pair: "飞机带对",
    four_two_single: "四带二",
    four_two_pair: "四带两对",
    bomb: "炸弹",
    rocket: "王炸",
  };
  return labels[play.category];
}

export function canBeat(candidate: DdzCard[], lastPlay?: DdzPlay | DdzPlayInfo | null) {
  const evaluated = evaluatePlay(candidate);
  if (!evaluated) return false;
  if (!lastPlay) return true;
  if (evaluated.category === "rocket") return lastPlay.category !== "rocket";
  if (lastPlay.category === "rocket") return false;
  if (evaluated.category === "bomb" && lastPlay.category !== "bomb") return true;
  if (lastPlay.category === "bomb" && evaluated.category !== "bomb") return false;
  return evaluated.category === lastPlay.category
    && evaluated.length === lastPlay.length
    && evaluated.mainLength === lastPlay.mainLength
    && evaluated.value > lastPlay.value;
}

export function makeRoom(roomId: string, name: string, hostDeviceId: string, hostName: string, avatar?: string | null): DdzRoom {
  return {
    roomId,
    name,
    hostDeviceId,
    hostName,
    players: [{ deviceId: hostDeviceId, nickname: hostName, avatar, ready: false, online: true, handCount: 0 }],
    phase: "lobby",
    landlordCards: [],
    myHand: [],
    handCounts: {},
    bidOrder: [],
    bidIndex: 0,
    bids: {},
    lastPlay: null,
    passCount: 0,
    logs: [`${hostName} 创建了房间`],
    chatMessages: [],
    updatedAt: Date.now(),
  };
}

export function nextTurn(players: DdzPlayer[], currentId: string) {
  const index = players.findIndex((player) => player.deviceId === currentId);
  if (index < 0 || players.length === 0) return players[0]?.deviceId;
  return players[(index + 1) % players.length]?.deviceId;
}

export function dealHands(players: DdzPlayer[]) {
  const deck = shuffleDeck(createDeck());
  const hands: Record<string, DdzCard[]> = {};
  players.forEach((player) => {
    hands[player.deviceId] = sortCards(deck.splice(0, 17));
  });
  const landlordCards = sortCards(deck.splice(0, 3));
  return { hands, landlordCards };
}


