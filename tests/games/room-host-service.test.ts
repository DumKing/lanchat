import { describe, expect, it } from "vitest";
import type { GameRoomShell } from "../../src/games/registry";
import { isGameRoomHost, removeGameRoomShell, upsertGameRoomShell } from "../../src/features/games/roomHostService";

const room = (roomId: string, updatedAt: number, hostDeviceId = "host"): GameRoomShell => ({
  roomId,
  gameType: "gomoku",
  roomName: roomId,
  hostDeviceId,
  hostName: hostDeviceId,
  players: [],
  createdAt: 1,
  updatedAt,
});

describe("roomHostService", () => {
  it("新增和更新房间后按更新时间倒序排列", () => {
    const initial = [room("a", 10), room("b", 20)];
    const inserted = upsertGameRoomShell(initial, room("c", 15));
    const updated = upsertGameRoomShell(inserted, room("a", 30));

    expect(inserted.map((item) => item.roomId)).toEqual(["b", "c", "a"]);
    expect(updated.map((item) => `${item.roomId}:${item.updatedAt}`)).toEqual(["a:30", "b:20", "c:15"]);
    expect(initial.map((item) => item.roomId)).toEqual(["a", "b"]);
  });

  it("删除当前房间后选择剩余列表第一项", () => {
    const result = removeGameRoomShell([room("a", 10), room("b", 20)], "b", "b");

    expect(result.rooms.map((item) => item.roomId)).toEqual(["a"]);
    expect(result.activeRoomId).toBe("a");
  });

  it("删除非当前房间时保留当前选择", () => {
    const result = removeGameRoomShell([room("a", 10), room("b", 20)], "a", "b");

    expect(result.activeRoomId).toBe("b");
  });

  it("只把 hostDeviceId 对应设备视为房主", () => {
    expect(isGameRoomHost(room("a", 1, "peer-a"), "peer-a")).toBe(true);
    expect(isGameRoomHost(room("a", 1, "peer-a"), "peer-b")).toBe(false);
    expect(isGameRoomHost(null, "peer-a")).toBe(false);
  });
});
