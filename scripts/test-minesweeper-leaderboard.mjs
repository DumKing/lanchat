import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "minesweeper-leaderboard-"));
const outfile = path.join(tempDir, "leaderboard.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/minesweeperLeaderboard.ts")],
    outfile,
    bundle: true,
    platform: "node",
    format: "esm",
  });
  const mod = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    MINESWEEPER_DIFFICULTIES,
    createMinesweeperLeaderboardRecord,
    formatMinesweeperElapsed,
    minesweeperDifficultyKey,
    recordsForDifficulty,
    upsertMinesweeperLeaderboardRecords,
  } = mod;

  assert.equal(MINESWEEPER_DIFFICULTIES[0].key, "16x16-40");
  assert.equal(MINESWEEPER_DIFFICULTIES.some((item) => item.key === "64x32-320" && item.width === 64 && item.height === 32), true);
  assert.equal(minesweeperDifficultyKey(64, 32, 320), "64x32-320");

  const slow = createMinesweeperLeaderboardRecord({ deviceId: "a", nickname: "A", width: 16, height: 16, mines: 40, elapsedMs: 80_000, moves: 88, finishedAt: 10 });
  const fast = createMinesweeperLeaderboardRecord({ deviceId: "a", nickname: "A2", width: 16, height: 16, mines: 40, elapsedMs: 70_000, moves: 80, finishedAt: 20 });
  const other = createMinesweeperLeaderboardRecord({ deviceId: "b", nickname: "B", width: 16, height: 16, mines: 40, elapsedMs: 75_000, moves: 90, finishedAt: 30 });
  const large = createMinesweeperLeaderboardRecord({ deviceId: "a", nickname: "A", width: 64, height: 32, mines: 320, elapsedMs: 300_000, moves: 220, finishedAt: 40 });
  const records = upsertMinesweeperLeaderboardRecords([slow], [fast, other, large]);
  const smallRows = recordsForDifficulty(records, "16x16-40", 5);

  assert.equal(smallRows.length, 2);
  assert.equal(smallRows[0].deviceId, "a");
  assert.equal(smallRows[0].elapsedMs, 70_000);
  assert.equal(smallRows[1].deviceId, "b");
  assert.equal(recordsForDifficulty(records, "64x32-320", 5)[0].elapsedMs, 300_000);

  const manyPlayers = Array.from({ length: 25 }, (_, index) => createMinesweeperLeaderboardRecord({
    deviceId: `device-${index}`,
    nickname: `玩家${index}`,
    width: 16,
    height: 16,
    mines: 40,
    elapsedMs: 30_000 + index * 1000,
    moves: 40 + index,
    finishedAt: 100 + index,
  }));
  const manyRows = recordsForDifficulty(upsertMinesweeperLeaderboardRecords([], manyPlayers), "16x16-40", 100);
  assert.equal(manyRows.length, 25);
  assert.equal(formatMinesweeperElapsed(65_430), "1:05.4");
  assert.equal(formatMinesweeperElapsed(9_900), "9.9s");

  console.log("minesweeper leaderboard ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}






