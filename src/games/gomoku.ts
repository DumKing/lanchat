export const GOMOKU_BOARD_SIZE = 15;
export const GOMOKU_TURN_TIMEOUT_MS = 20_000;

export type GomokuStone = "black" | "white";
export type GomokuCell = GomokuStone | null;
export type GomokuBoard = GomokuCell[][];
export type GomokuPhase = "lobby" | "playing" | "ended";
export type GomokuPoint = { x: number; y: number };
export type GomokuMoveResult =
  | { ok: true; board: GomokuBoard; draw: boolean; winner?: GomokuStone; winLine?: GomokuPoint[] }
  | { ok: false; board: GomokuBoard; error: string };

const directions = [
  { x: 1, y: 0 },
  { x: 0, y: 1 },
  { x: 1, y: 1 },
  { x: 1, y: -1 },
] as const;

export function createGomokuBoard(size = GOMOKU_BOARD_SIZE): GomokuBoard {
  return Array.from({ length: size }, () => Array.from<GomokuCell>({ length: size }).fill(null));
}

export function cloneGomokuBoard(board: GomokuBoard): GomokuBoard {
  return board.map((row) => [...row]);
}

export function nextGomokuStone(stone: GomokuStone): GomokuStone {
  return stone === "black" ? "white" : "black";
}

export function isGomokuBoardFull(board: GomokuBoard): boolean {
  return board.every((row) => row.every((cell) => cell !== null));
}

export function gomokuStoneLabel(stone?: GomokuStone | null): string {
  if (stone === "black") return "黑棋";
  if (stone === "white") return "白棋";
  return "未分配";
}

export function gomokuTurnRemainingSeconds(turnStartedAt: number | undefined, now = Date.now(), timeoutMs = GOMOKU_TURN_TIMEOUT_MS) {
  if (!turnStartedAt) return Math.ceil(timeoutMs / 1000);
  return Math.max(0, Math.ceil((timeoutMs - (now - turnStartedAt)) / 1000));
}

export function isGomokuTurnTimedOut(turnStartedAt: number | undefined, now = Date.now(), timeoutMs = GOMOKU_TURN_TIMEOUT_MS) {
  return gomokuTurnRemainingSeconds(turnStartedAt, now, timeoutMs) <= 0;
}

export function chooseAutoGomokuPoint(board: GomokuBoard): GomokuPoint | null {
  const center = Math.floor(board.length / 2);
  const points: GomokuPoint[] = [];

  for (let y = 0; y < board.length; y += 1) {
    for (let x = 0; x < (board[y]?.length ?? 0); x += 1) {
      if (!board[y]?.[x]) {
        points.push({ x, y });
      }
    }
  }

  points.sort((a, b) => {
    const distanceA = Math.abs(a.x - center) + Math.abs(a.y - center);
    const distanceB = Math.abs(b.x - center) + Math.abs(b.y - center);
    return distanceA - distanceB || a.y - b.y || a.x - b.x;
  });

  return points[0] ?? null;
}

export function placeGomokuStone(board: GomokuBoard, point: GomokuPoint, stone: GomokuStone): GomokuMoveResult {
  if (!isInsideBoard(board, point)) {
    return { ok: false, board, error: "落子位置超出棋盘" };
  }
  if (board[point.y]?.[point.x]) {
    return { ok: false, board, error: "当前位置已有棋子" };
  }

  const nextBoard = cloneGomokuBoard(board);
  nextBoard[point.y]![point.x] = stone;
  const winLine = getGomokuWinLine(nextBoard, point);
  if (winLine) {
    return { ok: true, board: nextBoard, winner: stone, winLine, draw: false };
  }
  return { ok: true, board: nextBoard, draw: isGomokuBoardFull(nextBoard) };
}

export function getGomokuWinLine(board: GomokuBoard, point: GomokuPoint): GomokuPoint[] | null {
  if (!isInsideBoard(board, point)) return null;
  const stone = board[point.y]?.[point.x];
  if (!stone) return null;

  for (const direction of directions) {
    const forward = collectDirection(board, point, direction.x, direction.y, stone);
    const backward = collectDirection(board, point, -direction.x, -direction.y, stone);
    const line = [...backward.reverse(), point, ...forward];
    if (line.length >= 5) return line;
  }
  return null;
}

function collectDirection(board: GomokuBoard, start: GomokuPoint, dx: number, dy: number, stone: GomokuStone): GomokuPoint[] {
  const points: GomokuPoint[] = [];
  let x = start.x + dx;
  let y = start.y + dy;
  while (isInsideBoard(board, { x, y }) && board[y]?.[x] === stone) {
    points.push({ x, y });
    x += dx;
    y += dy;
  }
  return points;
}

function isInsideBoard(board: GomokuBoard, point: GomokuPoint): boolean {
  return point.y >= 0 && point.y < board.length && point.x >= 0 && point.x < (board[point.y]?.length ?? 0);
}


