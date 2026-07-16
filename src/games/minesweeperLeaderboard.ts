export type MinesweeperDifficulty = {
  key: string;
  label: string;
  width: number;
  height: number;
  mines: number;
};

export type MinesweeperLeaderboardRecord = {
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

export const MINESWEEPER_DIFFICULTIES: MinesweeperDifficulty[] = [
  { key: "16x16-40", label: "16 x 16", width: 16, height: 16, mines: 40 },
  { key: "32x32-160", label: "32 x 32", width: 32, height: 32, mines: 160 },
  { key: "64x32-320", label: "64 x 32", width: 64, height: 32, mines: 320 },
  { key: "9x9-10", label: "9 x 9", width: 9, height: 9, mines: 10 },
  { key: "30x16-99", label: "30 x 16", width: 30, height: 16, mines: 99 },
];

export function minesweeperDifficultyKey(width: number, height: number, mines: number) {
  return findMinesweeperDifficulty(width, height, mines)?.key ?? `${width}x${height}-${mines}`;
}

export function minesweeperDifficultyLabel(width: number, height: number, mines: number) {
  return findMinesweeperDifficulty(width, height, mines)?.label ?? `${width} x ${height}`;
}

export function findMinesweeperDifficulty(width: number, height: number, mines: number) {
  return MINESWEEPER_DIFFICULTIES.find((item) => item.width === width && item.height === height && item.mines === mines) ?? null;
}

export function difficultyByKey(key: string) {
  return MINESWEEPER_DIFFICULTIES.find((item) => item.key === key) ?? MINESWEEPER_DIFFICULTIES[0];
}

export function createMinesweeperLeaderboardRecord(input: {
  deviceId: string;
  nickname: string;
  width: number;
  height: number;
  mines: number;
  elapsedMs: number;
  moves: number;
  finishedAt?: number;
}): MinesweeperLeaderboardRecord {
  const finishedAt = input.finishedAt ?? Date.now();
  const difficultyKey = minesweeperDifficultyKey(input.width, input.height, input.mines);
  return {
    id: `${difficultyKey}:${input.deviceId}`,
    deviceId: input.deviceId,
    nickname: input.nickname || "局域网玩家",
    difficultyKey,
    difficultyLabel: minesweeperDifficultyLabel(input.width, input.height, input.mines),
    width: input.width,
    height: input.height,
    mines: input.mines,
    elapsedMs: Math.max(0, Math.round(input.elapsedMs)),
    moves: Math.max(0, Math.round(input.moves)),
    finishedAt,
  };
}

export function upsertMinesweeperLeaderboardRecords(
  existing: MinesweeperLeaderboardRecord[],
  incoming: MinesweeperLeaderboardRecord[],
  limitPerDifficulty = Number.MAX_SAFE_INTEGER,
) {
  const best = new Map<string, MinesweeperLeaderboardRecord>();
  for (const record of [...existing, ...incoming]) {
    if (!isValidRecord(record)) continue;
    const difficultyKey = minesweeperDifficultyKey(record.width, record.height, record.mines);
    const key = `${difficultyKey}:${record.deviceId}`;
    const normalized = {
      ...record,
      id: key,
      difficultyKey,
      difficultyLabel: minesweeperDifficultyLabel(record.width, record.height, record.mines),
    };
    const previous = best.get(key);
    if (!previous || normalized.elapsedMs < previous.elapsedMs || (normalized.elapsedMs === previous.elapsedMs && normalized.finishedAt > previous.finishedAt)) {
      best.set(key, normalized);
    }
  }

  const order = new Map(MINESWEEPER_DIFFICULTIES.map((item, index) => [item.key, index]));
  const grouped = new Map<string, MinesweeperLeaderboardRecord[]>();
  for (const record of best.values()) {
    grouped.set(record.difficultyKey, [...(grouped.get(record.difficultyKey) ?? []), record]);
  }

  return [...grouped.entries()]
    .sort((a, b) => (order.get(a[0]) ?? 999) - (order.get(b[0]) ?? 999) || a[0].localeCompare(b[0]))
    .flatMap(([, records]) => records
      .sort((a, b) => a.elapsedMs - b.elapsedMs || a.finishedAt - b.finishedAt || a.nickname.localeCompare(b.nickname))
      .slice(0, limitPerDifficulty));
}

export function recordsForDifficulty(records: MinesweeperLeaderboardRecord[], difficultyKey: string, limit = 5) {
  return records
    .filter((record) => record.difficultyKey === difficultyKey)
    .sort((a, b) => a.elapsedMs - b.elapsedMs || a.finishedAt - b.finishedAt)
    .slice(0, limit);
}

export function formatMinesweeperElapsed(elapsedMs: number) {
  if (!Number.isFinite(elapsedMs) || elapsedMs <= 0) return "--";
  const totalSeconds = Math.floor(elapsedMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const decimal = Math.floor((elapsedMs % 1000) / 100);
  return minutes > 0 ? `${minutes}:${seconds.toString().padStart(2, "0")}.${decimal}` : `${seconds}.${decimal}s`;
}

function isValidRecord(record: MinesweeperLeaderboardRecord) {
  return Boolean(record.deviceId)
    && Number.isFinite(record.width)
    && Number.isFinite(record.height)
    && Number.isFinite(record.mines)
    && Number.isFinite(record.elapsedMs)
    && record.width > 0
    && record.height > 0
    && record.mines > 0
    && record.elapsedMs > 0;
}

