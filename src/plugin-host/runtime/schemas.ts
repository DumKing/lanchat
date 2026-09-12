type Validator = (params: unknown) => boolean;

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function emptyOrObject(value: unknown) {
  return value === undefined || isObject(value);
}

function hasString(value: unknown, key: string) {
  return isObject(value) && typeof value[key] === "string" && String(value[key]).length > 0;
}

export const PLUGIN_METHOD_SCHEMAS: Readonly<Record<string, Validator>> = Object.freeze({
  "app.getVersion": emptyOrObject,
  "app.getInstance": emptyOrObject,
  "app.close": emptyOrObject,
  "devices.list": emptyOrObject,
  "chat.send": (value) => hasString(value, "conversationId") && hasString(value, "text"),
  "rooms.list": emptyOrObject,
  "rooms.create": (value) => hasString(value, "gameId") && hasString(value, "name")
    && isObject(value) && Number.isInteger(value.maxPlayers) && Number.isInteger(value.schemaVersion),
  "rooms.join": (value) => hasString(value, "roomId"),
  "rooms.leave": (value) => hasString(value, "roomId"),
  "rooms.send": (value) => hasString(value, "roomId") && hasString(value, "type") && hasString(value, "idempotencyKey"),
  "rooms.snapshot": (value) => hasString(value, "roomId"),
  "rooms.invite": (value) => hasString(value, "roomId"),
  "leaderboard.list": (value) => hasString(value, "gameId"),
  "leaderboard.submit": (value) => hasString(value, "gameId") && hasString(value, "roomId") && hasString(value, "idempotencyKey"),
  "storage.get": (value) => hasString(value, "key"),
  "storage.set": (value) => hasString(value, "key") && isObject(value) && "value" in value,
  "storage.delete": (value) => hasString(value, "key"),
  "storage.keys": emptyOrObject,
  "theme.current": emptyOrObject,
  "ui.notify": (value) => hasString(value, "title") && hasString(value, "body"),
  "ui.confirm": (value) => hasString(value, "title") && hasString(value, "body"),
  "ui.pickFile": emptyOrObject,
  "logger.write": (value) => hasString(value, "level") && hasString(value, "message"),
});
