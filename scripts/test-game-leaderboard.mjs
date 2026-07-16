import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "game-leaderboard-"));
const outfile = path.join(tempDir, "game-leaderboard.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/gameLeaderboard.ts")],
    outfile,
    bundle: true,
    platform: "node",
    format: "esm",
  });
  const mod = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    createGameStatsRecord,
    formatWinRate,
    incrementGameStats,
    recordsForGame,
    upsertGameStatsRecords,
  } = mod;

  const a = createGameStatsRecord({ game: "gomoku", deviceId: "a", nickname: "A", totalGames: 1, wins: 1, updatedAt: 10 });
  const aLater = createGameStatsRecord({ game: "gomoku", deviceId: "a", nickname: "A", totalGames: 2, wins: 1, updatedAt: 20 });
  const b = createGameStatsRecord({ game: "gomoku", deviceId: "b", nickname: "B", totalGames: 3, wins: 3, updatedAt: 30 });
  const c = createGameStatsRecord({ game: "doudizhu", deviceId: "c", nickname: "C", totalGames: 5, wins: 2, updatedAt: 40 });
  const records = upsertGameStatsRecords([a, c], [aLater, b]);
  const gomokuRows = recordsForGame(records, "gomoku");

  assert.equal(gomokuRows.length, 2);
  assert.equal(gomokuRows[0].deviceId, "b");
  assert.equal(gomokuRows[1].deviceId, "a");
  assert.equal(gomokuRows[1].totalGames, 2);
  assert.equal(recordsForGame(records, "doudizhu")[0].deviceId, "c");
  assert.equal(formatWinRate(gomokuRows[0]), "100%");
  assert.equal(formatWinRate(gomokuRows[1]), "50%");

  const incremented = incrementGameStats(records, { game: "gomoku", deviceId: "a", nickname: "A", won: true, updatedAt: 50 });
  const nextA = recordsForGame(incremented, "gomoku").find((record) => record.deviceId === "a");
  assert.equal(nextA.totalGames, 3);
  assert.equal(nextA.wins, 2);

  console.log("game leaderboard ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
