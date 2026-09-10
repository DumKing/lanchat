import type { GameRoomShell } from "../../games/registry";

export function isGameRoomHost(room: GameRoomShell | null | undefined, deviceId: string): boolean {
  return Boolean(room && deviceId && room.hostDeviceId === deviceId);
}

export function upsertGameRoomShell(
  rooms: readonly GameRoomShell[],
  incoming: GameRoomShell,
): GameRoomShell[] {
  return [...rooms.filter((room) => room.roomId !== incoming.roomId), incoming]
    .sort((left, right) => right.updatedAt - left.updatedAt);
}

export function removeGameRoomShell(
  rooms: readonly GameRoomShell[],
  roomId: string,
  activeRoomId: string,
): { rooms: GameRoomShell[]; activeRoomId: string } {
  const remaining = rooms.filter((room) => room.roomId !== roomId);
  return {
    rooms: remaining,
    activeRoomId: activeRoomId === roomId ? remaining[0]?.roomId ?? "" : activeRoomId,
  };
}
