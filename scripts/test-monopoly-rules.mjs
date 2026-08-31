import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "monopoly-"));
const outfile = path.join(tempDir, "monopoly.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/monopoly.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });

  const monopoly = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const { MONOPOLY_BOARD_SIZE, createMonopolyBoard, propertyDistrictOf } = monopoly;

  const board = createMonopolyBoard();
  assert.equal(MONOPOLY_BOARD_SIZE, 40);
  assert.equal(board.length, 40);
  assert.deepEqual(
    board.filter((tile) => tile.kind === "corner").map((tile) => tile.corner),
    ["start", "airport", "price_double", "jail"],
  );
  assert.equal(board.filter((tile) => tile.kind === "property").length, 32);
  assert.equal(board.filter((tile) => tile.kind === "event").length, 4);

  for (const eventIndex of [5, 15, 25, 35]) {
    assert.equal(board[eventIndex].kind, "event", `tile ${eventIndex} should be a random event`);
  }
  for (const cornerIndex of [0, 10, 20, 30]) {
    assert.equal(board[cornerIndex].kind, "corner", `tile ${cornerIndex} should be a corner`);
  }

  assert.equal(propertyDistrictOf(board, 1), 0);
  assert.equal(propertyDistrictOf(board, 4), 0);
  assert.equal(propertyDistrictOf(board, 6), 1);
  assert.equal(propertyDistrictOf(board, 9), 1);
  assert.equal(propertyDistrictOf(board, 5), null, "event tiles do not belong to a property district");
  assert.equal(propertyDistrictOf(board, 10), null, "corner tiles do not belong to a property district");

  console.log("monopoly board rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
