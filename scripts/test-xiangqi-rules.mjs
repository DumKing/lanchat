import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "xiangqi-"));
const outfile = path.join(tempDir, "xiangqi.mjs");

function emptyBoard() {
  return Array.from({ length: 10 }, () => Array.from({ length: 9 }).fill(null));
}

function piece(side, kind, id = `${side}-${kind}`) {
  return { id, side, kind };
}

try {
  await build({
    entryPoints: [path.join(root, "src/games/xiangqi.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });

  const xiangqi = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    XIANGQI_FILES,
    XIANGQI_RANKS,
    createXiangqiBoard,
    createXiangqiDisplayGrid,
    cloneXiangqiBoard,
    isLegalXiangqiMove,
    isXiangqiGeneralInCheck,
    moveXiangqiPiece,
    resignXiangqiSide,
    undoXiangqiMove,
    xiangqiPieceLabel,
    toXiangqiBoardPoint,
  } = xiangqi;

  const board = createXiangqiBoard();
  assert.equal(XIANGQI_FILES, 9);
  assert.equal(XIANGQI_RANKS, 10);
  assert.equal(board.length, 10);
  assert.equal(board.every((row) => row.length === 9), true);
  assert.deepEqual(board[9][4], { id: "red-general", side: "red", kind: "general" });
  assert.deepEqual(board[0][4], { id: "black-general", side: "black", kind: "general" });
  assert.equal(xiangqiPieceLabel(board[9][4]), "帅");
  assert.equal(xiangqiPieceLabel(board[0][4]), "将");
  assert.deepEqual(toXiangqiBoardPoint({ x: 0, y: 0 }, "red"), { x: 0, y: 0 });
  assert.deepEqual(toXiangqiBoardPoint({ x: 0, y: 0 }, "black"), { x: 8, y: 9 });
  const redPerspectiveGrid = createXiangqiDisplayGrid("red");
  const blackPerspectiveGrid = createXiangqiDisplayGrid("black");
  assert.deepEqual(redPerspectiveGrid[0][0], { x: 0, y: 0 });
  assert.deepEqual(redPerspectiveGrid[9][8], { x: 8, y: 9 });
  assert.deepEqual(blackPerspectiveGrid[0][0], { x: 8, y: 9 });
  assert.deepEqual(blackPerspectiveGrid[9][8], { x: 0, y: 0 });
  assert.deepEqual(board[blackPerspectiveGrid[0][0].y][blackPerspectiveGrid[0][0].x], { id: "red-rook-right", side: "red", kind: "rook" });
  assert.deepEqual(board[blackPerspectiveGrid[9][8].y][blackPerspectiveGrid[9][8].x], { id: "black-rook-left", side: "black", kind: "rook" });

  const undoBoard = emptyBoard();
  const redRook = piece("red", "rook", "red-rook");
  const blackHorse = piece("black", "horse", "black-horse");
  undoBoard[5][4] = redRook;
  undoBoard[4][4] = blackHorse;
  const afterCapture = moveXiangqiPiece(undoBoard, { x: 4, y: 5 }, { x: 4, y: 4 }, "red");
  assert.equal(afterCapture.ok, true);
  const restoredBoard = undoXiangqiMove(afterCapture.board, {
    from: { x: 4, y: 5 },
    to: { x: 4, y: 4 },
    piece: redRook,
    captured: blackHorse,
  });
  assert.deepEqual(restoredBoard[5][4], redRook, "悔棋后移动棋子回到起点");
  assert.deepEqual(restoredBoard[4][4], blackHorse, "悔棋后被吃棋子回到终点");
  assert.equal(resignXiangqiSide("red"), "black", "红方投降时黑方获胜");
  assert.equal(resignXiangqiSide("black"), "red", "黑方投降时红方获胜");

  const rookBoard = cloneXiangqiBoard(board);
  assert.equal(isLegalXiangqiMove(rookBoard, { x: 0, y: 9 }, { x: 0, y: 5 }, "red"), false, "车不能越过己方兵");
  rookBoard[6][0] = null;
  assert.equal(isLegalXiangqiMove(rookBoard, { x: 0, y: 9 }, { x: 0, y: 5 }, "red"), true, "车直线无阻挡可走");

  const horseBoard = cloneXiangqiBoard(board);
  assert.equal(isLegalXiangqiMove(horseBoard, { x: 1, y: 9 }, { x: 2, y: 7 }, "red"), true, "马可走日字");
  horseBoard[8][1] = piece("red", "soldier", "red-leg-blocker");
  assert.equal(isLegalXiangqiMove(horseBoard, { x: 1, y: 9 }, { x: 2, y: 7 }, "red"), false, "马腿被挡不能走");

  const elephantBoard = emptyBoard();
  elephantBoard[9][2] = piece("red", "elephant", "red-elephant");
  assert.equal(isLegalXiangqiMove(elephantBoard, { x: 2, y: 9 }, { x: 4, y: 7 }, "red"), true, "相可走田字");
  elephantBoard[8][3] = piece("red", "soldier", "red-eye-blocker");
  assert.equal(isLegalXiangqiMove(elephantBoard, { x: 2, y: 9 }, { x: 4, y: 7 }, "red"), false, "象眼被挡不能走");
  const crossingElephant = emptyBoard();
  crossingElephant[5][6] = piece("red", "elephant", "red-elephant-river");
  assert.equal(isLegalXiangqiMove(crossingElephant, { x: 6, y: 5 }, { x: 8, y: 3 }, "red"), false, "相不能过河");

  const palaceBoard = emptyBoard();
  palaceBoard[9][4] = piece("red", "general", "red-general");
  palaceBoard[8][3] = piece("red", "advisor", "red-advisor");
  assert.equal(isLegalXiangqiMove(palaceBoard, { x: 4, y: 9 }, { x: 4, y: 8 }, "red"), true, "帅在九宫内走一步");
  assert.equal(isLegalXiangqiMove(palaceBoard, { x: 4, y: 9 }, { x: 4, y: 7 }, "red"), false, "帅不能走两步");
  assert.equal(isLegalXiangqiMove(palaceBoard, { x: 3, y: 8 }, { x: 4, y: 7 }, "red"), true, "仕在九宫内斜走");
  assert.equal(isLegalXiangqiMove(palaceBoard, { x: 3, y: 8 }, { x: 2, y: 7 }, "red"), false, "仕不能出九宫");

  const cannonBoard = cloneXiangqiBoard(board);
  assert.equal(isLegalXiangqiMove(cannonBoard, { x: 1, y: 7 }, { x: 1, y: 2 }, "red"), false, "炮吃子必须隔一个炮架");
  cannonBoard[5][1] = piece("red", "soldier", "red-screen");
  assert.equal(isLegalXiangqiMove(cannonBoard, { x: 1, y: 7 }, { x: 1, y: 2 }, "red"), true, "炮隔一个炮架可吃子");

  const soldierBoard = cloneXiangqiBoard(board);
  assert.equal(isLegalXiangqiMove(soldierBoard, { x: 0, y: 6 }, { x: 0, y: 5 }, "red"), true, "兵可向前走");
  assert.equal(isLegalXiangqiMove(soldierBoard, { x: 0, y: 6 }, { x: 1, y: 6 }, "red"), false, "兵未过河不能横走");
  const crossedSoldier = emptyBoard();
  crossedSoldier[4][0] = piece("red", "soldier", "red-crossed-soldier");
  assert.equal(isLegalXiangqiMove(crossedSoldier, { x: 0, y: 4 }, { x: 1, y: 4 }, "red"), true, "兵过河后可横走");
  assert.equal(isLegalXiangqiMove(crossedSoldier, { x: 0, y: 4 }, { x: 0, y: 5 }, "red"), false, "兵不能后退");

  const faceBoard = emptyBoard();
  faceBoard[9][4] = piece("red", "general", "red-general");
  faceBoard[0][4] = piece("black", "general", "black-general");
  assert.equal(isLegalXiangqiMove(faceBoard, { x: 4, y: 9 }, { x: 4, y: 8 }, "red"), false, "双方将帅不能照面");

  const checkBoard = emptyBoard();
  checkBoard[9][4] = piece("red", "general", "red-general");
  checkBoard[0][4] = piece("black", "general", "black-general");
  checkBoard[4][4] = piece("black", "rook", "black-rook");
  assert.equal(isXiangqiGeneralInCheck(checkBoard, "red"), true, "车直线将军");

  const captureBoard = emptyBoard();
  captureBoard[9][4] = piece("red", "general", "red-general");
  captureBoard[0][4] = piece("black", "general", "black-general");
  captureBoard[2][4] = piece("red", "rook", "red-rook");
  const capture = moveXiangqiPiece(captureBoard, { x: 4, y: 2 }, { x: 4, y: 0 }, "red");
  assert.equal(capture.ok, true);
  assert.equal(capture.winner, "red");
  assert.equal(capture.captured.kind, "general");

  console.log("xiangqi rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}



