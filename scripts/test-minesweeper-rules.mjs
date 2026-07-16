import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "minesweeper-"));
const outfile = path.join(tempDir, "minesweeper.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/minesweeper.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });

  const minesweeper = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    cloneMinesweeperBoard,
    createMinesweeperBoard,
    createMinesweeperBoardFromMines,
    getMinesweeperProgress,
    isMinesweeperWin,
    revealMinesweeperCell,
    chordRevealMinesweeperCell,
    toggleMinesweeperFlag,
  } = minesweeper;

  const generated = createMinesweeperBoard({ width: 9, height: 9, mines: 10, seed: 20260708 });
  assert.equal(generated.length, 9);
  assert.equal(generated.every((row) => row.length === 9), true);
  assert.equal(generated.flat().filter((cell) => cell.mine).length, 10);

  const board = createMinesweeperBoardFromMines(3, 3, [{ x: 0, y: 0 }]);
  assert.equal(board[1][1].adjacent, 1);
  assert.equal(board[2][2].adjacent, 0);
  const zeroReveal = revealMinesweeperCell(board, { x: 2, y: 2 });
  assert.equal(zeroReveal.ok, true);
  assert.equal(zeroReveal.lost, false);
  assert.equal(zeroReveal.board[2][2].revealed, true);
  assert.equal(board[2][2].revealed, false, "reveal should not mutate source board");

  const flagBoard = createMinesweeperBoardFromMines(3, 3, [{ x: 0, y: 0 }]);
  const flagged = toggleMinesweeperFlag(flagBoard, { x: 0, y: 0 });
  assert.equal(flagged.board[0][0].flagged, true);
  const revealFlagged = revealMinesweeperCell(flagged.board, { x: 0, y: 0 });
  assert.equal(revealFlagged.lost, false, "flagged cell should not be opened by normal click");
  assert.equal(revealFlagged.board[0][0].revealed, false);

  const chordBoard = createMinesweeperBoardFromMines(3, 3, [{ x: 0, y: 0 }]);
  let chordState = revealMinesweeperCell(chordBoard, { x: 1, y: 1 }).board;
  chordState = toggleMinesweeperFlag(chordState, { x: 0, y: 0 }).board;
  const chorded = chordRevealMinesweeperCell(chordState, { x: 1, y: 1 });
  assert.equal(chorded.ok, true);
  assert.equal(chorded.lost, false);
  for (const point of [
    { x: 0, y: 1 },
    { x: 1, y: 0 },
    { x: 2, y: 0 },
    { x: 2, y: 1 },
    { x: 0, y: 2 },
    { x: 1, y: 2 },
    { x: 2, y: 2 },
  ]) {
    assert.equal(chorded.board[point.y][point.x].revealed, true, `double click should reveal ${point.x},${point.y}`);
  }

  const wrongFlagBoard = createMinesweeperBoardFromMines(3, 3, [{ x: 0, y: 0 }]);
  let wrongState = revealMinesweeperCell(wrongFlagBoard, { x: 1, y: 1 }).board;
  wrongState = toggleMinesweeperFlag(wrongState, { x: 0, y: 1 }).board;
  const wrongChord = chordRevealMinesweeperCell(wrongState, { x: 1, y: 1 });
  assert.equal(wrongChord.lost, true, "wrong flag count matching the number should open the real mine and lose");
  assert.equal(wrongChord.board[0][0].exploded, true);

  const winBoard = createMinesweeperBoardFromMines(2, 2, [{ x: 0, y: 0 }]);
  let winState = revealMinesweeperCell(winBoard, { x: 1, y: 0 }).board;
  winState = revealMinesweeperCell(winState, { x: 0, y: 1 }).board;
  winState = revealMinesweeperCell(winState, { x: 1, y: 1 }).board;
  assert.equal(isMinesweeperWin(winState), true);
  assert.deepEqual(getMinesweeperProgress(winState), { revealedSafe: 3, totalSafe: 3, flagged: 0 });
  assert.deepEqual(cloneMinesweeperBoard(winState), winState);

  console.log("minesweeper rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
