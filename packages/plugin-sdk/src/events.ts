export const PLUGIN_EVENTS = Object.freeze({
  enter: "plugin.enter",
  out: "plugin.out",
  themeChanged: "theme.changed",
  networkChanged: "network.changed",
  roomEvent: "room.event",
  visibilityChanged: "visibility.changed",
} as const);

export type PluginEventName = typeof PLUGIN_EVENTS[keyof typeof PLUGIN_EVENTS];
