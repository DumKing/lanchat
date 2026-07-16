export const MINESWEEPER_DEFAULT_WIDTH = 16;
export const MINESWEEPER_DEFAULT_HEIGHT = 16;
export const MINESWEEPER_DEFAULT_MINES = 40;

export type MinesweeperPhase = "lobby" | "playing" | "ended";
export type MinesweeperPoint = { x: number; y: number };
export type MinesweeperCell = {
  mine: boolean;
  adjacent: number;
  revealed: boolean;
  flagged: boolean;
  exploded?: boolean;
};
export type MinesweeperBoard = MinesweeperCell[][];
export type MinesweeperCreateOptions = {
  width?: number;
  height?: number;
  mines?: number;
  seed?: number;
};
export type MinesweeperActionResult = {
  ok: boolean;
  board: MinesweeperBoard;
  changed: boolean;
  lost: boolean;
  won: boolean;
};

export function createMinesweeperBoard(options: MinesweeperCreateOptions = {}): MinesweeperBoard {
  const width = normalizeDimension(options.width, MINESWEEPER_DEFAULT_WIDTH);
  const height = normalizeDimension(options.height, MINESWEEPER_DEFAULT_HEIGHT);
  const maxMines = Math.max(1, width * height - 1);
  const mines = Math.min(Math.max(1, options.mines ?? MINESWEEPER_DEFAULT_MINES), maxMines);
  const seed = options.seed ?? Date.now();
  const points: MinesweeperPoint[] = [];
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) points.push({ x, y });
  }
  const random = seededRandom(seed);
  for (let index = points.length - 1; index > 0; index -= 1) {
    const swapIndex = Math.floor(random() * (index + 1));
    [points[index], points[swapIndex]] = [points[swapIndex]!, points[index]!];
  }
  return createMinesweeperBoardFromMines(width, height, points.slice(0, mines));
}

export function createMinesweeperBoardFromMines(width: number, height: number, mines: MinesweeperPoint[]): MinesweeperBoard {
  const board = Array.from({ length: height }, () =>
    Array.from<MinesweeperCell>({ length: width }).fill(null as unknown as MinesweeperCell).map(() => ({
      mine: false,
      adjacent: 0,
      revealed: false,
      flagged: false,
    })),
  );
  for (const mine of mines) {
    if (isInsideMinesweeperBoard(board, mine)) board[mine.y]![mine.x]!.mine = true;
  }
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      board[y]![x]!.adjacent = neighborsOf(board, { x, y }).filter((point) => board[point.y]?.[point.x]?.mine).length;
    }
  }
  return board;
}

export function cloneMinesweeperBoard(board: MinesweeperBoard): MinesweeperBoard {
  return board.map((row) => row.map((cell) => ({ ...cell })));
}

export function revealMinesweeperCell(board: MinesweeperBoard, point: MinesweeperPoint): MinesweeperActionResult {
  const nextBoard = cloneMinesweeperBoard(board);
  if (!isInsideMinesweeperBoard(nextBoard, point)) return makeMinesweeperResult(nextBoard, false, false);
  const cell = nextBoard[point.y]![point.x]!;
  if (cell.flagged || cell.revealed) return makeMinesweeperResult(nextBoard, false, false);
  if (cell.mine) {
    cell.revealed = true;
    cell.exploded = true;
    revealAllMines(nextBoard);
    return makeMinesweeperResult(nextBoard, true, true);
  }
  revealSafeArea(nextBoard, point);
  return makeMinesweeperResult(nextBoard, true, false);
}

export function toggleMinesweeperFlag(board: MinesweeperBoard, point: MinesweeperPoint): MinesweeperActionResult {
  const nextBoard = cloneMinesweeperBoard(board);
  if (!isInsideMinesweeperBoard(nextBoard, point)) return makeMinesweeperResult(nextBoard, false, false);
  const cell = nextBoard[point.y]![point.x]!;
  if (cell.revealed) return makeMinesweeperResult(nextBoard, false, false);
  cell.flagged = !cell.flagged;
  return makeMinesweeperResult(nextBoard, true, false);
}

export function chordRevealMinesweeperCell(board: MinesweeperBoard, point: MinesweeperPoint): MinesweeperActionResult {
  const nextBoard = cloneMinesweeperBoard(board);
  if (!isInsideMinesweeperBoard(nextBoard, point)) return makeMinesweeperResult(nextBoard, false, false);
  const cell = nextBoard[point.y]![point.x]!;
  if (!cell.revealed || cell.adjacent <= 0) return makeMinesweeperResult(nextBoard, false, false);
  const neighbors = neighborsOf(nextBoard, point);
  const flagged = neighbors.filter((item) => nextBoard[item.y]?.[item.x]?.flagged).length;
  if (flagged !== cell.adjacent) return makeMinesweeperResult(nextBoard, false, false);
  let changed = false;
  for (const neighbor of neighbors) {
    const neighborCell = nextBoard[neighbor.y]![neighbor.x]!;
    if (neighborCell.revealed || neighborCell.flagged) continue;
    changed = true;
    if (neighborCell.mine) {
      neighborCell.revealed = true;
      neighborCell.exploded = true;
      revealAllMines(nextBoard);
      return makeMinesweeperResult(nextBoard, true, true);
    }
    revealSafeArea(nextBoard, neighbor);
  }
  return makeMinesweeperResult(nextBoard, changed, false);
}

export function isMinesweeperWin(board: MinesweeperBoard): boolean {
  return board.every((row) => row.every((cell) => cell.mine || cell.revealed));
}

export function getMinesweeperProgress(board: MinesweeperBoard) {
  let revealedSafe = 0;
  let totalSafe = 0;
  let flagged = 0;
  for (const row of board) {
    for (const cell of row) {
      if (!cell.mine) totalSafe += 1;
      if (!cell.mine && cell.revealed) revealedSafe += 1;
      if (cell.flagged) flagged += 1;
    }
  }
  return { revealedSafe, totalSafe, flagged };
}

function makeMinesweeperResult(board: MinesweeperBoard, changed: boolean, lost: boolean): MinesweeperActionResult {
  return {
    ok: true,
    board,
    changed,
    lost,
    won: !lost && isMinesweeperWin(board),
  };
}

function revealSafeArea(board: MinesweeperBoard, start: MinesweeperPoint) {
  const queue: MinesweeperPoint[] = [start];
  const visited = new Set<string>();
  while (queue.length > 0) {
    const point = queue.shift()!;
    const key = `${point.x}:${point.y}`;
    if (visited.has(key) || !isInsideMinesweeperBoard(board, point)) continue;
    visited.add(key);
    const cell = board[point.y]![point.x]!;
    if (cell.revealed || cell.flagged || cell.mine) continue;
    cell.revealed = true;
    if (cell.adjacent !== 0) continue;
    for (const neighbor of neighborsOf(board, point)) {
      const neighborCell = board[neighbor.y]![neighbor.x]!;
      if (!neighborCell.revealed && !neighborCell.flagged && !neighborCell.mine) queue.push(neighbor);
    }
  }
}

function revealAllMines(board: MinesweeperBoard) {
  for (const row of board) {
    for (const cell of row) {
      if (cell.mine) cell.revealed = true;
    }
  }
}

function neighborsOf(board: MinesweeperBoard, point: MinesweeperPoint): MinesweeperPoint[] {
  const result: MinesweeperPoint[] = [];
  for (let y = point.y - 1; y <= point.y + 1; y += 1) {
    for (let x = point.x - 1; x <= point.x + 1; x += 1) {
      if (x === point.x && y === point.y) continue;
      const next = { x, y };
      if (isInsideMinesweeperBoard(board, next)) result.push(next);
    }
  }
  return result;
}

function isInsideMinesweeperBoard(board: MinesweeperBoard, point: MinesweeperPoint): boolean {
  return point.y >= 0 && point.y < board.length && point.x >= 0 && point.x < (board[0]?.length ?? 0);
}

function normalizeDimension(value: number | undefined, fallback: number) {
  return Math.max(4, Math.floor(value ?? fallback));
}

function seededRandom(seed: number) {
  let value = Math.floor(seed) >>> 0;
  return () => {
    value = (value * 1664525 + 1013904223) >>> 0;
    return value / 0x100000000;
  };
}
