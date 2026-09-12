export type PluginGameStatsRecord = {
  id: string;
  game: string;
  deviceId: string;
  nickname: string;
  totalGames: number;
  wins: number;
  updatedAt: number;
};

export type PluginTimedLeaderboardRecord = {
  id: string;
  deviceId: string;
  nickname: string;
  difficultyKey: string;
  difficultyLabel: string;
  width: number;
  height: number;
  mines: number;
  elapsedMs: number;
  moves: number;
  finishedAt: number;
};

export function createPluginGameStatsRecord(input: Omit<PluginGameStatsRecord, "id" | "updatedAt" | "totalGames" | "wins"> & {
  totalGames?: number;
  wins?: number;
  updatedAt?: number;
}): PluginGameStatsRecord {
  const totalGames = Math.max(0, Math.round(input.totalGames ?? 0));
  return {
    id: `${input.game}:${input.deviceId}`,
    game: input.game,
    deviceId: input.deviceId,
    nickname: input.nickname || "局域网玩家",
    totalGames,
    wins: Math.min(totalGames, Math.max(0, Math.round(input.wins ?? 0))),
    updatedAt: input.updatedAt ?? Date.now(),
  };
}

export function upsertPluginGameStatsRecords(existing: PluginGameStatsRecord[], incoming: PluginGameStatsRecord[]) {
  const records = new Map<string, PluginGameStatsRecord>();
  for (const candidate of [...existing, ...incoming]) {
    if (!candidate.deviceId || !candidate.game || !Number.isFinite(candidate.totalGames) || !Number.isFinite(candidate.wins)) continue;
    const record = createPluginGameStatsRecord(candidate);
    const previous = records.get(record.id);
    if (!previous || record.totalGames > previous.totalGames || (record.totalGames === previous.totalGames && record.wins > previous.wins) || (record.totalGames === previous.totalGames && record.wins === previous.wins && record.updatedAt > previous.updatedAt)) {
      records.set(record.id, record);
    }
  }
  return [...records.values()].sort((a, b) => a.game.localeCompare(b.game) || score(b) - score(a) || b.wins - a.wins || b.updatedAt - a.updatedAt);
}

export function createPluginTimedLeaderboardRecord(input: {
  deviceId: string;
  nickname: string;
  width: number;
  height: number;
  mines: number;
  elapsedMs: number;
  moves: number;
  finishedAt?: number;
}): PluginTimedLeaderboardRecord {
  const difficultyKey = `${input.width}x${input.height}-${input.mines}`;
  return {
    id: `${difficultyKey}:${input.deviceId}`,
    deviceId: input.deviceId,
    nickname: input.nickname || "局域网玩家",
    difficultyKey,
    difficultyLabel: `${input.width} x ${input.height}`,
    width: input.width,
    height: input.height,
    mines: input.mines,
    elapsedMs: Math.max(0, Math.round(input.elapsedMs)),
    moves: Math.max(0, Math.round(input.moves)),
    finishedAt: input.finishedAt ?? Date.now(),
  };
}

export function upsertPluginTimedLeaderboardRecords(existing: PluginTimedLeaderboardRecord[], incoming: PluginTimedLeaderboardRecord[]) {
  const records = new Map<string, PluginTimedLeaderboardRecord>();
  for (const candidate of [...existing, ...incoming]) {
    if (!candidate.deviceId || candidate.width <= 0 || candidate.height <= 0 || candidate.mines <= 0 || candidate.elapsedMs <= 0) continue;
    const record = createPluginTimedLeaderboardRecord(candidate);
    const previous = records.get(record.id);
    if (!previous || record.elapsedMs < previous.elapsedMs || (record.elapsedMs === previous.elapsedMs && record.finishedAt > previous.finishedAt)) records.set(record.id, record);
  }
  return [...records.values()].sort((a, b) => a.difficultyKey.localeCompare(b.difficultyKey) || a.elapsedMs - b.elapsedMs || a.finishedAt - b.finishedAt);
}

function score(record: PluginGameStatsRecord) {
  return record.totalGames > 0 ? record.wins / record.totalGames : 0;
}
