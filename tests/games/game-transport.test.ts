import { describe, expect, it } from "vitest";
import { createGameTransportInbox, type GameTransportEnvelope } from "../../src/features/games/gameTransport";

const envelope = (overrides: Partial<GameTransportEnvelope> = {}): GameTransportEnvelope => ({
  roomId: "room-1",
  gameId: "gomoku",
  senderPeerId: "peer-a",
  schemaVersion: 1,
  idempotencyKey: "move-1",
  type: "move",
  payload: { x: 1, y: 2 },
  ...overrides,
});

describe("gameTransport", () => {
  it("接受宿主支持的协议版本", () => {
    const inbox = createGameTransportInbox({ gomoku: [1] });

    expect(inbox.accept(envelope())).toEqual({ accepted: true });
  });

  it("拒绝同一房间内重复的幂等键", () => {
    const inbox = createGameTransportInbox({ gomoku: [1] });
    inbox.accept(envelope());

    expect(inbox.accept(envelope())).toEqual({ accepted: false, reason: "duplicate" });
    expect(inbox.accept(envelope({ roomId: "room-2" }))).toEqual({ accepted: true });
  });

  it("拒绝未知游戏和不兼容协议版本", () => {
    const inbox = createGameTransportInbox({ gomoku: [1] });

    expect(inbox.accept(envelope({ gameId: "unknown" }))).toEqual({ accepted: false, reason: "unsupported-game" });
    expect(inbox.accept(envelope({ schemaVersion: 2 }))).toEqual({ accepted: false, reason: "unsupported-schema" });
  });

  it("无效信封不会占用幂等键", () => {
    const inbox = createGameTransportInbox({ gomoku: [1] });

    expect(inbox.accept(envelope({ senderPeerId: "" }))).toEqual({ accepted: false, reason: "invalid-envelope" });
    expect(inbox.accept(envelope())).toEqual({ accepted: true });
  });

  it("按容量淘汰最早的幂等键", () => {
    const inbox = createGameTransportInbox({ gomoku: [1] }, { maxRemembered: 2 });
    inbox.accept(envelope({ idempotencyKey: "move-1" }));
    inbox.accept(envelope({ idempotencyKey: "move-2" }));
    inbox.accept(envelope({ idempotencyKey: "move-3" }));

    expect(inbox.accept(envelope({ idempotencyKey: "move-1" }))).toEqual({ accepted: true });
  });
});
