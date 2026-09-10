import { describe, expect, it } from "vitest";
import { createLanChatPluginClient } from "../../packages/plugin-sdk/src/index";
import type { PluginBridgeTransport, PluginUnsubscribe } from "../../packages/plugin-sdk/src/types";

function createTransport() {
  const requests: Array<{ method: string; params: unknown }> = [];
  const subscriptions: Array<{ event: string; callback: (payload: unknown) => void }> = [];
  const transport: PluginBridgeTransport = {
    async request<TResult>(method: string, params?: unknown) {
      requests.push({ method, params });
      return undefined as TResult;
    },
    subscribe<TPayload>(event: string, callback: (payload: TPayload) => void): PluginUnsubscribe {
      const subscription = { event, callback: callback as (payload: unknown) => void };
      subscriptions.push(subscription);
      return () => {
        const index = subscriptions.indexOf(subscription);
        if (index >= 0) subscriptions.splice(index, 1);
      };
    },
  };
  return { transport, requests, subscriptions };
}

describe("createLanChatPluginClient", () => {
  it("暴露完整的 v1 标准 API 命名空间", () => {
    const { transport } = createTransport();
    const client = createLanChatPluginClient(transport);

    expect(Object.keys(client).sort()).toEqual([
      "apiVersion",
      "app",
      "chat",
      "devices",
      "events",
      "leaderboard",
      "logger",
      "rooms",
      "storage",
      "theme",
      "ui",
    ]);
    expect(Object.isFrozen(client)).toBe(true);
  });

  it("把业务调用映射到稳定的桥方法名", async () => {
    const { transport, requests } = createTransport();
    const client = createLanChatPluginClient(transport);

    await client.devices.list();
    await client.chat.send({ conversationId: "room-1", text: "你好" });
    await client.rooms.join({ roomId: "game-1" });
    await client.leaderboard.list({ gameId: "gomoku" });
    await client.storage.set("settings", { sound: true });
    await client.ui.notify({ title: "五子棋", body: "轮到你了" });

    expect(requests.map((request) => request.method)).toEqual([
      "devices.list",
      "chat.send",
      "rooms.join",
      "leaderboard.list",
      "storage.set",
      "ui.notify",
    ]);
  });

  it("取消订阅后不再保留事件监听器", () => {
    const { transport, subscriptions } = createTransport();
    const client = createLanChatPluginClient(transport);
    const unsubscribe = client.events.onVisibilityChanged(() => undefined);

    expect(subscriptions.map((item) => item.event)).toEqual(["visibility.changed"]);
    unsubscribe();
    expect(subscriptions).toHaveLength(0);
  });
});
