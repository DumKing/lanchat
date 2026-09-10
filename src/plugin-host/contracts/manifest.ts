export const PLUGIN_MANIFEST_VERSION = 1 as const;
export const PLUGIN_API_VERSION = "1.0" as const;

export const PLUGIN_CAPABILITIES = [
  "devices.read",
  "chat.read",
  "chat.send",
  "rooms.read",
  "rooms.write",
  "leaderboard.read",
  "leaderboard.write",
  "storage.private",
  "theme.read",
  "ui.notify",
  "ui.filePicker",
  "network.lan",
  "network.internet",
  "logger.write",
  "native.sidecar",
] as const;

export type PluginCapability = typeof PLUGIN_CAPABILITIES[number];
export type PluginType = "web" | "web-sidecar";

export interface NavigationContribution {
  id: string;
  title: string;
  icon?: string;
  order?: number;
}

export interface GameContribution {
  id: string;
  minPlayers: number;
  maxPlayers: number;
  ranking?: {
    type: "win-loss" | "score" | "time";
    minHumanPlayers?: number;
  };
}

export interface CommandContribution {
  id: string;
  title: string;
  description?: string;
}

export interface PluginContributions {
  navigation?: NavigationContribution[];
  games?: GameContribution[];
  commands?: CommandContribution[];
}

export interface PluginManifestV1 {
  manifestVersion: typeof PLUGIN_MANIFEST_VERSION;
  id: string;
  name: string;
  version: string;
  apiVersion: typeof PLUGIN_API_VERSION;
  minHostVersion: string;
  type: PluginType;
  entry: string;
  icon: string;
  singleton: boolean;
  capabilities: PluginCapability[];
  contributes?: PluginContributions;
}

export interface ManifestValidationError {
  field: string;
  code: "REQUIRED" | "INVALID_FORMAT" | "UNSUPPORTED" | "DUPLICATE" | "OUT_OF_RANGE";
  message: string;
}

export type ManifestValidationResult =
  | { ok: true; manifest: PluginManifestV1 }
  | { ok: false; errors: ManifestValidationError[] };
