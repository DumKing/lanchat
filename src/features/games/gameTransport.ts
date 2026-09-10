export interface GameTransportEnvelope {
  roomId: string;
  gameId: string;
  senderPeerId: string;
  schemaVersion: number;
  idempotencyKey: string;
  type: string;
  payload: unknown;
}

export type GameTransportRejectionReason =
  | "invalid-envelope"
  | "unsupported-game"
  | "unsupported-schema"
  | "duplicate";

export type GameTransportAcceptance =
  | { accepted: true }
  | { accepted: false; reason: GameTransportRejectionReason };

export interface GameTransportInbox {
  accept(envelope: GameTransportEnvelope): GameTransportAcceptance;
  clearRoom(roomId: string): void;
}

function isValidEnvelope(envelope: GameTransportEnvelope) {
  return Boolean(
    envelope.roomId
    && envelope.gameId
    && envelope.senderPeerId
    && envelope.idempotencyKey
    && envelope.type
    && Number.isInteger(envelope.schemaVersion)
    && envelope.schemaVersion > 0,
  );
}

export function createGameTransportInbox(
  supportedSchemas: Readonly<Record<string, readonly number[]>>,
  options: { maxRemembered?: number } = {},
): GameTransportInbox {
  const maxRemembered = options.maxRemembered ?? 2048;
  if (!Number.isInteger(maxRemembered) || maxRemembered < 1) {
    throw new Error("maxRemembered 必须是大于等于 1 的整数");
  }

  const remembered = new Set<string>();
  const insertionOrder: string[] = [];

  return {
    accept(envelope) {
      if (!isValidEnvelope(envelope)) return { accepted: false, reason: "invalid-envelope" };
      const schemas = supportedSchemas[envelope.gameId];
      if (!schemas) return { accepted: false, reason: "unsupported-game" };
      if (!schemas.includes(envelope.schemaVersion)) return { accepted: false, reason: "unsupported-schema" };

      const key = `${envelope.roomId}\u0000${envelope.idempotencyKey}`;
      if (remembered.has(key)) return { accepted: false, reason: "duplicate" };

      remembered.add(key);
      insertionOrder.push(key);
      while (insertionOrder.length > maxRemembered) {
        const oldest = insertionOrder.shift();
        if (oldest !== undefined) remembered.delete(oldest);
      }
      return { accepted: true };
    },
    clearRoom(roomId) {
      const prefix = `${roomId}\u0000`;
      for (let index = insertionOrder.length - 1; index >= 0; index -= 1) {
        const key = insertionOrder[index];
        if (!key.startsWith(prefix)) continue;
        insertionOrder.splice(index, 1);
        remembered.delete(key);
      }
    },
  };
}
