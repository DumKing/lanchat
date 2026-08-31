export const MONOPOLY_BOARD_SIZE = 40;

export type MonopolyCorner = "start" | "airport" | "price_double" | "jail";
export type MonopolyPropertyLevel = "empty" | "house" | "level2" | "level3";

export type MonopolyCornerTile = {
  index: number;
  kind: "corner";
  corner: MonopolyCorner;
};

export type MonopolyEventTile = {
  index: number;
  kind: "event";
};

export type MonopolyPropertyTile = {
  index: number;
  kind: "property";
  district: number;
};

export type MonopolyTile = MonopolyCornerTile | MonopolyEventTile | MonopolyPropertyTile;

const corners: Record<number, MonopolyCorner> = {
  0: "start",
  10: "airport",
  20: "price_double",
  30: "jail",
};

const eventIndices = new Set([5, 15, 25, 35]);

export function createMonopolyBoard(): MonopolyTile[] {
  return Array.from({ length: MONOPOLY_BOARD_SIZE }, (_, index) => {
    const corner = corners[index];
    if (corner) return { index, kind: "corner", corner };
    if (eventIndices.has(index)) return { index, kind: "event" };
    return { index, kind: "property", district: propertyDistrictIndex(index) };
  });
}

export function propertyDistrictOf(board: MonopolyTile[], index: number): number | null {
  const tile = board[index];
  return tile?.kind === "property" ? tile.district : null;
}

function propertyDistrictIndex(index: number): number {
  const side = Math.floor(index / 10);
  const offset = index % 10;
  return side * 2 + (offset < 5 ? 0 : 1);
}
