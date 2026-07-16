import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "gomoku-"));
const outfile = path.join(tempDir, "gomoku.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/gomoku.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });

  const gomoku = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    GOMOKU_BOARD_SIZE,
    chooseAutoGomokuPoint,
    createGomokuBoard,
    gomokuTurnRemainingSeconds,
    isGomokuBoardFull,
    isGomokuTurnTimedOut,
    nextGomokuStone,
    placeGomokuStone,
  } = gomoku;

  const empty = createGomokuBoard();
  assert.equal(empty.length, GOMOKU_BOARD_SIZE);
  assert.equal(empty.every((row) => row.length === GOMOKU_BOARD_SIZE), true);
  assert.equal(empty.flat().every((cell) => cell === null), true);
  assert.deepEqual(chooseAutoGomokuPoint(empty), { x: 7, y: 7 });
  assert.equal(gomokuTurnRemainingSeconds(undefined, 1000), 20);
  assert.equal(gomokuTurnRemainingSeconds(1000, 20_100), 1);
  assert.equal(isGomokuTurnTimedOut(1000, 21_001), true);

  const firstMove = placeGomokuStone(empty, { x: 7, y: 7 }, "black");
  assert.equal(firstMove.ok, true);
  assert.equal(firstMove.board[7][7], "black");
  assert.equal(empty[7][7], null, "placeGomokuStone should not mutate the source board");
  assert.deepEqual(chooseAutoGomokuPoint(firstMove.board), { x: 7, y: 6 });

  const duplicate = placeGomokuStone(firstMove.board, { x: 7, y: 7 }, "white");
  assert.equal(duplicate.ok, false);
  assert.equal(duplicate.error, "当前位置已有棋子");

  const outside = placeGomokuStone(firstMove.board, { x: -1, y: 0 }, "white");
  assert.equal(outside.ok, false);
  assert.equal(outside.error, "落子位置超出棋盘");

  let horizontal = createGomokuBoard();
  for (let x = 3; x <= 7; x += 1) {
    const result = placeGomokuStone(horizontal, { x, y: 8 }, "black");
    assert.equal(result.ok, true);
    horizontal = result.board;
  }
  const horizontalWin = placeGomokuStone(horizontal, { x: 8, y: 8 }, "black");
  assert.equal(horizontalWin.ok, true);
  assert.equal(horizontalWin.winner, "black");
  assert.equal(horizontalWin.winLine.length, 6);

  let diagonal = createGomokuBoard();
  for (let index = 0; index < 4; index += 1) {
    const result = placeGomokuStone(diagonal, { x: index, y: index }, "white");
    assert.equal(result.ok, true);
    diagonal = result.board;
  }
  const diagonalWin = placeGomokuStone(diagonal, { x: 4, y: 4 }, "white");
  assert.equal(diagonalWin.ok, true);
  assert.equal(diagonalWin.winner, "white");
  assert.deepEqual(diagonalWin.winLine, [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
    { x: 2, y: 2 },
    { x: 3, y: 3 },
    { x: 4, y: 4 },
  ]);

  const full = Array.from({ length: GOMOKU_BOARD_SIZE }, (_, y) =>
    Array.from({ length: GOMOKU_BOARD_SIZE }, (_, x) => ((x + y) % 2 === 0 ? "black" : "white")),
  );
  assert.equal(isGomokuBoardFull(full), true);
  assert.equal(chooseAutoGomokuPoint(full), null);
  assert.equal(nextGomokuStone("black"), "white");
  assert.equal(nextGomokuStone("white"), "black");

  console.log("gomoku rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

