import type {
  LanChatPluginApiV1,
  PluginBridgeTransport,
  PluginEnterContext,
  PluginExitContext,
  PluginNetworkSnapshot,
  PluginRoomEvent,
  PluginThemeSnapshot,
} from "./types";

export type * from "./types";
export { PLUGIN_EVENTS, type PluginEventName } from "./events";

export function createLanChatPluginClient(transport: PluginBridgeTransport): LanChatPluginApiV1 {
  const client: LanChatPluginApiV1 = {
    apiVersion: "1.0" as const,
    app: {
      getVersion: () => transport.request<string>("app.getVersion"),
      getInstance: () => transport.request<{ pluginId: string; instanceId: string }>("app.getInstance"),
      close: () => transport.request<void>("app.close"),
    },
    events: {
      onPluginEnter: (callback: (context: PluginEnterContext) => void) => transport.subscribe("plugin.enter", callback),
      onPluginOut: (callback: (context: PluginExitContext) => void) => transport.subscribe("plugin.out", callback),
      onThemeChanged: (callback: (theme: PluginThemeSnapshot) => void) => transport.subscribe("theme.changed", callback),
      onNetworkChanged: (callback: (network: PluginNetworkSnapshot) => void) => transport.subscribe("network.changed", callback),
      onRoomEvent: (callback: (event: PluginRoomEvent) => void) => transport.subscribe("room.event", callback),
      onVisibilityChanged: (callback: (visible: boolean) => void) => transport.subscribe("visibility.changed", callback),
    },
    devices: {
      list: () => transport.request("devices.list"),
    },
    chat: {
      send: (input) => transport.request("chat.send", input),
    },
    rooms: {
      list: (input) => transport.request("rooms.list", input),
      create: (input) => transport.request("rooms.create", input),
      join: (input) => transport.request("rooms.join", input),
      leave: (input) => transport.request("rooms.leave", input),
      send: (input) => transport.request("rooms.send", input),
      snapshot: (input) => transport.request("rooms.snapshot", input),
    },
    leaderboard: {
      list: (input) => transport.request("leaderboard.list", input),
      submit: (input) => transport.request("leaderboard.submit", input),
    },
    storage: {
      get: (key) => transport.request("storage.get", { key }),
      set: (key, value) => transport.request("storage.set", { key, value }),
      delete: (key) => transport.request("storage.delete", { key }),
      keys: () => transport.request("storage.keys"),
    },
    theme: {
      current: () => transport.request<PluginThemeSnapshot>("theme.current"),
    },
    ui: {
      notify: (input) => transport.request("ui.notify", input),
      confirm: (input) => transport.request("ui.confirm", input),
      pickFile: (input) => transport.request("ui.pickFile", input),
    },
    logger: {
      debug: (message: string, details?: unknown) => transport.request<void>("logger.write", { level: "debug", message, details }),
      info: (message: string, details?: unknown) => transport.request<void>("logger.write", { level: "info", message, details }),
      warn: (message: string, details?: unknown) => transport.request<void>("logger.write", { level: "warn", message, details }),
      error: (message: string, details?: unknown) => transport.request<void>("logger.write", { level: "error", message, details }),
    },
  };
  return Object.freeze(client);
}

declare global {
  interface Window {
    readonly lanchat?: LanChatPluginApiV1;
  }
}
