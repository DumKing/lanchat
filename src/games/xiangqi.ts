export const XIANGQI_FILES = 9;
export const XIANGQI_RANKS = 10;

export type XiangqiSide = "red" | "black";
export type XiangqiPieceKind = "general" | "advisor" | "elephant" | "horse" | "rook" | "cannon" | "soldier";
export type XiangqiPoint = { x: number; y: number };
export type XiangqiPiece = {
  id: string;
  side: XiangqiSide;
  kind: XiangqiPieceKind;
};
export type XiangqiCell = XiangqiPiece | null;
export type XiangqiBoard = XiangqiCell[][];
export type XiangqiPhase = "lobby" | "playing" | "ended";
export type XiangqiMoveResult =
  | { ok: true; board: XiangqiBoard; captured?: XiangqiPiece; winner?: XiangqiSide; check?: boolean }
  | { ok: false; board: XiangqiBoard; error: string };
export type XiangqiUndoSnapshot = {
  from: XiangqiPoint;
  to: XiangqiPoint;
  piece: XiangqiPiece;
  captured?: XiangqiPiece | null;
};

export function createXiangqiBoard(): XiangqiBoard {
  const board = createEmptyXiangqiBoard();
  const place = (x: number, y: number, side: XiangqiSide, kind: XiangqiPieceKind, id: string) => {
    board[y]![x] = { id, side, kind };
  };

  place(0, 0, "black", "rook", "black-rook-left");
  place(1, 0, "black", "horse", "black-horse-left");
  place(2, 0, "black", "elephant", "black-elephant-left");
  place(3, 0, "black", "advisor", "black-advisor-left");
  place(4, 0, "black", "general", "black-general");
  place(5, 0, "black", "advisor", "black-advisor-right");
  place(6, 0, "black", "elephant", "black-elephant-right");
  place(7, 0, "black", "horse", "black-horse-right");
  place(8, 0, "black", "rook", "black-rook-right");
  place(1, 2, "black", "cannon", "black-cannon-left");
  place(7, 2, "black", "cannon", "black-cannon-right");
  for (const x of [0, 2, 4, 6, 8]) {
    place(x, 3, "black", "soldier", `black-soldier-${x}`);
  }

  place(0, 9, "red", "rook", "red-rook-left");
  place(1, 9, "red", "horse", "red-horse-left");
  place(2, 9, "red", "elephant", "red-elephant-left");
  place(3, 9, "red", "advisor", "red-advisor-left");
  place(4, 9, "red", "general", "red-general");
  place(5, 9, "red", "advisor", "red-advisor-right");
  place(6, 9, "red", "elephant", "red-elephant-right");
  place(7, 9, "red", "horse", "red-horse-right");
  place(8, 9, "red", "rook", "red-rook-right");
  place(1, 7, "red", "cannon", "red-cannon-left");
  place(7, 7, "red", "cannon", "red-cannon-right");
  for (const x of [0, 2, 4, 6, 8]) {
    place(x, 6, "red", "soldier", `red-soldier-${x}`);
  }

  return board;
}

export function createEmptyXiangqiBoard(): XiangqiBoard {
  return Array.from({ length: XIANGQI_RANKS }, () => Array.from<XiangqiCell>({ length: XIANGQI_FILES }).fill(null));
}

export function toXiangqiBoardPoint(displayPoint: XiangqiPoint, perspective?: XiangqiSide | null): XiangqiPoint {
  if (perspective !== "black") return { ...displayPoint };
  return {
    x: XIANGQI_FILES - 1 - displayPoint.x,
    y: XIANGQI_RANKS - 1 - displayPoint.y,
  };
}

export function createXiangqiDisplayGrid(perspective?: XiangqiSide | null): XiangqiPoint[][] {
  return Array.from({ length: XIANGQI_RANKS }, (_, displayY) =>
    Array.from({ length: XIANGQI_FILES }, (_, displayX) => toXiangqiBoardPoint({ x: displayX, y: displayY }, perspective)),
  );
}

export function cloneXiangqiBoard(board: XiangqiBoard): XiangqiBoard {
  return board.map((row) => row.map((cell) => cell ? { ...cell } : null));
}

export function undoXiangqiMove(board: XiangqiBoard, move: XiangqiUndoSnapshot): XiangqiBoard {
  const nextBoard = cloneXiangqiBoard(board);
  nextBoard[move.from.y]![move.from.x] = { ...move.piece };
  nextBoard[move.to.y]![move.to.x] = move.captured ? { ...move.captured } : null;
  return nextBoard;
}

export function resignXiangqiSide(side: XiangqiSide): XiangqiSide {
  return otherXiangqiSide(side);
}

export function otherXiangqiSide(side: XiangqiSide): XiangqiSide {
  return side === "red" ? "black" : "red";
}

export function xiangqiSideLabel(side?: XiangqiSide | null): string {
  if (side === "red") return "红方";
  if (side === "black") return "黑方";
  return "未分配";
}

export function xiangqiPieceLabel(piece?: XiangqiPiece | XiangqiPieceKind | null): string {
  const kind = typeof piece === "string" ? piece : piece?.kind;
  const side = typeof piece === "string" ? undefined : piece?.side;
  if (kind === "general") return side === "black" ? "将" : "帅";
  if (kind === "advisor") return side === "black" ? "士" : "仕";
  if (kind === "elephant") return side === "black" ? "象" : "相";
  if (kind === "horse") return "马";
  if (kind === "rook") return "车";
  if (kind === "cannon") return "炮";
  if (kind === "soldier") return side === "black" ? "卒" : "兵";
  return "";
}

export function isLegalXiangqiMove(board: XiangqiBoard, from: XiangqiPoint, to: XiangqiPoint, side: XiangqiSide): boolean {
  if (!isInsideXiangqiBoard(from) || !isInsideXiangqiBoard(to)) return false;
  if (from.x === to.x && from.y === to.y) return false;
  const piece = board[from.y]?.[from.x];
  const target = board[to.y]?.[to.x];
  if (!piece || piece.side !== side) return false;
  if (target?.side === side) return false;
  if (!isRawXiangqiMoveLegal(board, from, to, piece)) return false;

  const nextBoard = cloneXiangqiBoard(board);
  nextBoard[to.y]![to.x] = { ...piece };
  nextBoard[from.y]![from.x] = null;

  if (target?.kind === "general") return true;
  if (xiangqiGeneralsFace(nextBoard)) return false;
  return !isXiangqiGeneralInCheck(nextBoard, side);
}

export function moveXiangqiPiece(board: XiangqiBoard, from: XiangqiPoint, to: XiangqiPoint, side: XiangqiSide): XiangqiMoveResult {
  if (!isLegalXiangqiMove(board, from, to, side)) {
    return { ok: false, board, error: "不符合中国象棋走法" };
  }
  const nextBoard = cloneXiangqiBoard(board);
  const piece = nextBoard[from.y]?.[from.x];
  const captured = nextBoard[to.y]?.[to.x] ?? undefined;
  if (!piece) return { ok: false, board, error: "起点没有棋子" };
  nextBoard[to.y]![to.x] = piece;
  nextBoard[from.y]![from.x] = null;

  if (captured?.kind === "general") {
    return { ok: true, board: nextBoard, captured, winner: side, check: false };
  }

  const opponent = otherXiangqiSide(side);
  const check = isXiangqiGeneralInCheck(nextBoard, opponent);
  if (check && !hasLegalXiangqiMove(nextBoard, opponent)) {
    return { ok: true, board: nextBoard, captured, winner: side, check };
  }
  return { ok: true, board: nextBoard, captured, check };
}

export function isXiangqiGeneralInCheck(board: XiangqiBoard, side: XiangqiSide): boolean {
  const generalPoint = findXiangqiGeneral(board, side);
  if (!generalPoint) return false;
  const opponent = otherXiangqiSide(side);

  for (let y = 0; y < XIANGQI_RANKS; y += 1) {
    for (let x = 0; x < XIANGQI_FILES; x += 1) {
      const piece = board[y]?.[x];
      if (piece?.side === opponent && isRawXiangqiMoveLegal(board, { x, y }, generalPoint, piece)) {
        return true;
      }
    }
  }
  return false;
}

export function hasLegalXiangqiMove(board: XiangqiBoard, side: XiangqiSide): boolean {
  for (let fromY = 0; fromY < XIANGQI_RANKS; fromY += 1) {
    for (let fromX = 0; fromX < XIANGQI_FILES; fromX += 1) {
      if (board[fromY]?.[fromX]?.side !== side) continue;
      for (let toY = 0; toY < XIANGQI_RANKS; toY += 1) {
        for (let toX = 0; toX < XIANGQI_FILES; toX += 1) {
          if (isLegalXiangqiMove(board, { x: fromX, y: fromY }, { x: toX, y: toY }, side)) {
            return true;
          }
        }
      }
    }
  }
  return false;
}

export function findXiangqiGeneral(board: XiangqiBoard, side: XiangqiSide): XiangqiPoint | null {
  for (let y = 0; y < XIANGQI_RANKS; y += 1) {
    for (let x = 0; x < XIANGQI_FILES; x += 1) {
      const piece = board[y]?.[x];
      if (piece?.side === side && piece.kind === "general") return { x, y };
    }
  }
  return null;
}

function isRawXiangqiMoveLegal(board: XiangqiBoard, from: XiangqiPoint, to: XiangqiPoint, piece: XiangqiPiece): boolean {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const absX = Math.abs(dx);
  const absY = Math.abs(dy);
  const target = board[to.y]?.[to.x];

  if (piece.kind === "general") {
    if (target?.kind === "general" && from.x === to.x && countPiecesBetween(board, from, to) === 0) return true;
    return isInsidePalace(to, piece.side) && absX + absY === 1;
  }

  if (piece.kind === "advisor") {
    return isInsidePalace(to, piece.side) && absX === 1 && absY === 1;
  }

  if (piece.kind === "elephant") {
    if (absX !== 2 || absY !== 2 || crossesRiver(to, piece.side)) return false;
    const eye = { x: from.x + dx / 2, y: from.y + dy / 2 };
    return !board[eye.y]?.[eye.x];
  }

  if (piece.kind === "horse") {
    if (!((absX === 1 && absY === 2) || (absX === 2 && absY === 1))) return false;
    const leg = absX === 2 ? { x: from.x + dx / 2, y: from.y } : { x: from.x, y: from.y + dy / 2 };
    return !board[leg.y]?.[leg.x];
  }

  if (piece.kind === "rook") {
    return isStraight(from, to) && countPiecesBetween(board, from, to) === 0;
  }

  if (piece.kind === "cannon") {
    if (!isStraight(from, to)) return false;
    const between = countPiecesBetween(board, from, to);
    return target ? between === 1 : between === 0;
  }

  if (piece.kind === "soldier") {
    const forward = piece.side === "red" ? -1 : 1;
    if (dx === 0 && dy === forward) return true;
    if (hasCrossedRiver(from, piece.side) && absX === 1 && dy === 0) return true;
    return false;
  }

  return false;
}

function isInsideXiangqiBoard(point: XiangqiPoint): boolean {
  return point.x >= 0 && point.x < XIANGQI_FILES && point.y >= 0 && point.y < XIANGQI_RANKS;
}

function isStraight(from: XiangqiPoint, to: XiangqiPoint): boolean {
  return from.x === to.x || from.y === to.y;
}

function countPiecesBetween(board: XiangqiBoard, from: XiangqiPoint, to: XiangqiPoint): number {
  if (!isStraight(from, to)) return Number.POSITIVE_INFINITY;
  const stepX = Math.sign(to.x - from.x);
  const stepY = Math.sign(to.y - from.y);
  let x = from.x + stepX;
  let y = from.y + stepY;
  let count = 0;
  while (x !== to.x || y !== to.y) {
    if (board[y]?.[x]) count += 1;
    x += stepX;
    y += stepY;
  }
  return count;
}

function isInsidePalace(point: XiangqiPoint, side: XiangqiSide): boolean {
  const yMin = side === "red" ? 7 : 0;
  const yMax = side === "red" ? 9 : 2;
  return point.x >= 3 && point.x <= 5 && point.y >= yMin && point.y <= yMax;
}

function crossesRiver(point: XiangqiPoint, side: XiangqiSide): boolean {
  return side === "red" ? point.y < 5 : point.y > 4;
}

function hasCrossedRiver(point: XiangqiPoint, side: XiangqiSide): boolean {
  return side === "red" ? point.y <= 4 : point.y >= 5;
}

function xiangqiGeneralsFace(board: XiangqiBoard): boolean {
  const red = findXiangqiGeneral(board, "red");
  const black = findXiangqiGeneral(board, "black");
  if (!red || !black || red.x !== black.x) return false;
  return countPiecesBetween(board, red, black) === 0;
}


