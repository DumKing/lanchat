import { describe, expect, it } from "vitest";
import { PLUGIN_EVENTS, PLUGIN_HASH_CONTRACT, createLanChatPluginClient } from "../../packages/plugin-sdk/src/index";
import type { PluginEventName } from "../../packages/plugin-sdk/src/index";
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
  it("公开房间在线状态事件名", () => {
    const eventName: PluginEventName = "room.presenceChanged";

    expect(PLUGIN_EVENTS.roomPresenceChanged).toBe(eventName);
  });

  it("公开稳定的哈希与权威房间事件约定", () => {
    expect(PLUGIN_HASH_CONTRACT).toEqual({ algorithm: "sha256", canonicalization: "canonical-json-v1" });
    expect(PLUGIN_EVENTS.authoritativeRoomEvent).toBe("room.authoritativeEvent");
  });

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
    await client.rooms.invite({ roomId: "game-1" });
    await client.leaderboard.list({ gameId: "gomoku" });
    await client.storage.set("settings", { sound: true });
    await client.ui.notify({ title: "五子棋", body: "轮到你了" });

    expect(requests.map((request) => request.method)).toEqual([
      "devices.list",
      "chat.send",
      "rooms.join",
      "rooms.invite",
      "leaderboard.list",
      "storage.set",
      "ui.notify",
    ]);
  });

  it("把权威房间调用的方法名和参数原样传给桥接层", async () => {
    const { transport, requests } = createTransport();
    const client = createLanChatPluginClient(transport);
    const intent = {
      roomId: "room-1",
      commandId: "command-1",
      baseRevision: 3,
      protocolVersion: 2,
      payload: { action: "roll" },
    };
    const commit = {
      roomId: "room-1",
      commandId: "command-1",
      baseRevision: 3,
      protocolVersion: 2,
      events: [{ type: "dice-rolled", value: 6 }],
      stateHash: "state-4",
      terminal: { resultHash: "result-4" },
    };
    const checkpoint = {
      roomId: "room-1",
      revision: 4,
      protocolVersion: 2,
      payload: { turn: "peer-2" },
      stateHash: "state-4",
    };
    const recovery = { roomId: "room-1", afterRevision: 4 };
    const admission = { roomId: "room-1", admission: "closed" as const };
    const invite = { inviteId: "invite-1", token: "opaque-token" };

    await client.rooms.sendIntent(intent);
    await client.rooms.commit(commit);
    await client.rooms.publishCheckpoint(checkpoint);
    await client.rooms.recover(recovery);
    await client.rooms.setAdmission(admission);
    await client.rooms.resolveInvite(invite);

    expect(requests).toEqual([
      { method: "rooms.sendIntent", params: intent },
      { method: "rooms.commit", params: commit },
      { method: "rooms.publishCheckpoint", params: checkpoint },
      { method: "rooms.recover", params: recovery },
      { method: "rooms.setAdmission", params: admission },
      { method: "rooms.resolveInvite", params: invite },
    ]);
  });

  it("保留旧 rooms.send 并使用新的排行榜提交契约", async () => {
    const { transport, requests } = createTransport();
    const client = createLanChatPluginClient(transport);
    const legacyEvent = {
      roomId: "room-1",
      type: "move",
      payload: { steps: 2 },
      idempotencyKey: "legacy-1",
    };
    const result = {
      gameId: "monopoly",
      roomId: "room-1",
      terminalRevision: 18,
      terminalCommitId: "commit-18",
      stateHash: "state-18",
      result: { winnerPeerId: "peer-1" },
    };

    await client.rooms.send(legacyEvent);
    await client.leaderboard.submit(result);

    expect(requests).toEqual([
      { method: "rooms.send", params: legacyEvent },
      { method: "leaderboard.submit", params: result },
    ]);
  });

  it("排行榜旧、新提交参数都原样透传", async () => {
    const { transport, requests } = createTransport();
    const client = createLanChatPluginClient(transport);
    const legacy = {
      gameId: "gomoku",
      roomId: "room-legacy",
      idempotencyKey: "legacy-result-1",
      result: { winnerPeerId: "peer-1" },
    };
    const authoritative = {
      gameId: "monopoly",
      roomId: "room-authoritative",
      terminalRevision: 18,
      terminalCommitId: "commit-18",
      stateHash: "state-18",
      result: { winnerPeerId: "peer-2" },
    };

    await client.leaderboard.submit(legacy);
    await client.leaderboard.submit(authoritative);

    expect(requests).toEqual([
      { method: "leaderboard.submit", params: legacy },
      { method: "leaderboard.submit", params: authoritative },
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

  it("订阅并取消订阅房间在线状态事件", () => {
    const { transport, subscriptions } = createTransport();
    const client = createLanChatPluginClient(transport);
    const unsubscribe = client.events.onRoomPresenceChanged(() => undefined);

    expect(subscriptions.map((item) => item.event)).toEqual(["room.presenceChanged"]);
    unsubscribe();
    expect(subscriptions).toHaveLength(0);
  });

  it("分别订阅并取消订阅旧房间事件与权威房间事件", () => {
    const { transport, subscriptions } = createTransport();
    const client = createLanChatPluginClient(transport);
    const unsubscribeLegacy = client.events.onRoomEvent(() => undefined);
    const unsubscribeAuthoritative = client.events.onAuthoritativeRoomEvent(() => undefined);

    expect(subscriptions.map((item) => item.event)).toEqual(["room.event", "room.authoritativeEvent"]);
    unsubscribeLegacy();
    expect(subscriptions.map((item) => item.event)).toEqual(["room.authoritativeEvent"]);
    unsubscribeAuthoritative();
    expect(subscriptions).toHaveLength(0);
  });
});
