export type RankedGameType = "doudizhu" | "gomoku" | "xiangqi";

export type GameStatsRecord = {
  id: string;
  game: RankedGameType;
  deviceId: string;
  nickname: string;
  totalGames: number;
  wins: number;
  updatedAt: number;
};

export function createGameStatsRecord(input: {
  game: RankedGameType;
  deviceId: string;
  nickname: string;
  totalGames?: number;
  wins?: number;
  updatedAt?: number;
}): GameStatsRecord {
  const totalGames = Math.max(0, Math.round(input.totalGames ?? 0));
  const wins = Math.min(totalGames, Math.max(0, Math.round(input.wins ?? 0)));
  return {
    id: `${input.game}:${input.deviceId}`,
    game: input.game,
    deviceId: input.deviceId,
    nickname: input.nickname || "局域网玩家",
    totalGames,
    wins,
    updatedAt: input.updatedAt ?? Date.now(),
  };
}

export function incrementGameStats(
  records: GameStatsRecord[],
  input: {
    game: RankedGameType;
    deviceId: string;
    nickname: string;
    won: boolean;
    updatedAt?: number;
  },
) {
  const key = `${input.game}:${input.deviceId}`;
  const current = records.find((record) => record.id === key);
  const next = createGameStatsRecord({
    game: input.game,
    deviceId: input.deviceId,
    nickname: input.nickname,
    totalGames: (current?.totalGames ?? 0) + 1,
    wins: (current?.wins ?? 0) + (input.won ? 1 : 0),
    updatedAt: input.updatedAt,
  });
  return upsertGameStatsRecords(records.filter((record) => record.id !== key), [next]);
}

export function upsertGameStatsRecords(existing: GameStatsRecord[], incoming: GameStatsRecord[]) {
  const best = new Map<string, GameStatsRecord>();
  for (const record of [...existing, ...incoming]) {
    if (!isValidRecord(record)) continue;
    const normalized = createGameStatsRecord(record);
    const previous = best.get(normalized.id);
    if (
      !previous
      || normalized.totalGames > previous.totalGames
      || (normalized.totalGames === previous.totalGames && normalized.wins > previous.wins)
      || (normalized.totalGames === previous.totalGames && normalized.wins === previous.wins && normalized.updatedAt > previous.updatedAt)
    ) {
      best.set(normalized.id, normalized);
    }
  }
  return [...best.values()].sort((a, b) => a.game.localeCompare(b.game) || compareGameStats(a, b));
}

export function recordsForGame(records: GameStatsRecord[], game: RankedGameType, limit = 20) {
  return records
    .filter((record) => record.game === game)
    .sort(compareGameStats)
    .slice(0, limit);
}

export function formatWinRate(record: GameStatsRecord) {
  if (record.totalGames <= 0) return "0%";
  return `${Math.round((record.wins / record.totalGames) * 100)}%`;
}

function compareGameStats(a: GameStatsRecord, b: GameStatsRecord) {
  const aRate = a.totalGames > 0 ? a.wins / a.totalGames : 0;
  const bRate = b.totalGames > 0 ? b.wins / b.totalGames : 0;
  return bRate - aRate
    || b.wins - a.wins
    || b.totalGames - a.totalGames
    || b.updatedAt - a.updatedAt
    || a.nickname.localeCompare(b.nickname);
}

function isValidRecord(record: GameStatsRecord) {
  return Boolean(record.deviceId)
    && Boolean(record.game)
    && Number.isFinite(record.totalGames)
    && Number.isFinite(record.wins)
    && record.totalGames >= 0
    && record.wins >= 0
    && record.wins <= record.totalGames;
}
