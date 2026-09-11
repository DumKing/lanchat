import { describe, expect, it, vi } from "vitest";
import { PluginRoomService } from "../../src/plugin-host/services/PluginRoomService";

describe("PluginRoomService", () => {
  it("创建房间并把本地事件送回插件", async () => {
    const emit = vi.fn(async () => undefined);
    const send = vi.fn(async () => undefined);
    const service = new PluginRoomService({
      profile: () => ({ device_id: "peer-a", nickname: "A", listen_port: 18145, nickname_locked: false }),
      emit,
      send,
    });
    const room = await service.create({ gameId: "gomoku", name: "午休局", maxPlayers: 2, schemaVersion: 1 });
    await service.send({ roomId: room.roomId, type: "gomoku.ready", payload: { ready: true }, idempotencyKey: "ready-1" });

    expect(service.list("gomoku")).toHaveLength(1);
    expect(emit).toHaveBeenCalledWith("gomoku", expect.objectContaining({ senderPeerId: "peer-a", type: "gomoku.ready" }));
    expect(send).toHaveBeenCalledTimes(2);
  });

  it("拒绝插件创建其他游戏的房间", async () => {
    const service = new PluginRoomService({
      profile: () => ({ device_id: "peer-a", nickname: "A", listen_port: 18145, nickname_locked: false }),
      emit: async () => undefined,
      send: async () => undefined,
    });
    const handler = service.handlers()["rooms.create"];
    expect(() => handler({ gameId: "xiangqi", name: "x", maxPlayers: 2, schemaVersion: 1 }, {
      pluginId: "com.lanchat.gomoku", instanceId: "i", capabilities: new Set(),
    })).toThrow("不能创建");
  });
});
