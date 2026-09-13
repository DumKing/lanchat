export type PluginCapability =
  | "devices.read"
  | "chat.read"
  | "chat.send"
  | "rooms.read"
  | "rooms.write"
  | "leaderboard.read"
  | "leaderboard.write"
  | "storage.private"
  | "theme.read"
  | "ui.notify"
  | "ui.filePicker"
  | "network.lan"
  | "network.internet"
  | "logger.write"
  | "native.sidecar";

export type PluginUnsubscribe = () => void;
export type PluginEntrySource = "navigation" | "command" | "room" | "restore" | "developer";
export type PluginExitReason = "hidden" | "navigation" | "disabled" | "uninstalled" | "terminated";

export interface PluginEnterContext {
  featureCode: string;
  payload?: unknown;
  source: PluginEntrySource;
  instanceId: string;
  hostVersion: string;
}

export interface PluginExitContext {
  reason: PluginExitReason;
  isTerminated: boolean;
}

export interface PluginThemeSnapshot {
  mode: "light" | "dark";
  tokens: Readonly<Record<string, string>>;
}

export interface PluginNetworkSnapshot {
  online: boolean;
  lanAvailable: boolean;
}

export const PLUGIN_HASH_CONTRACT = Object.freeze({
  algorithm: "sha256",
  canonicalization: "canonical-json-v1",
} as const);

export interface PluginRoomEvent {
  sequence: number;
  roomId: string;
  gameId: string;
  senderPeerId: string;
  schemaVersion: number;
  idempotencyKey: string;
  type: string;
  payload: unknown;
}

export type PluginRoomAdmission = "lobby" | "closed";

export interface PluginRoomIntent {
  kind: "intent";
  roomId: string;
  commandId: string;
  baseRevision: number;
  protocolVersion: number;
  senderPeerId: string;
  payload: unknown;
}

export interface PluginAuthoritativeCommit {
  kind: "commit";
  roomId: string;
  commitId: string;
  commandId: string;
  baseRevision: number;
  revision: number;
  protocolVersion: number;
  events: unknown[];
  /** SHA-256 digest of canonical JSON as defined by {@link PLUGIN_HASH_CONTRACT}. */
  stateHash: string;
  terminal?: {
    /** SHA-256 digest of canonical JSON as defined by {@link PLUGIN_HASH_CONTRACT}. */
    resultHash: string;
  };
}

export type PluginAuthoritativeRoomEvent = PluginRoomIntent | PluginAuthoritativeCommit;

export interface PluginRoomCheckpoint {
  roomId: string;
  revision: number;
  protocolVersion: number;
  payload: unknown;
  /** SHA-256 digest of canonical JSON as defined by {@link PLUGIN_HASH_CONTRACT}. */
  stateHash: string;
  /** SHA-256 digest of canonical JSON as defined by {@link PLUGIN_HASH_CONTRACT}. */
  checksum: string;
}

export interface PluginRoomPresenceEvent {
  roomId: string;
  peerId: string;
  status: "online" | "offline" | "recovered";
  occurredAt: number;
}

export interface PluginEventApi {
  onPluginEnter(callback: (context: PluginEnterContext) => void): PluginUnsubscribe;
  onPluginOut(callback: (context: PluginExitContext) => void): PluginUnsubscribe;
  onThemeChanged(callback: (theme: PluginThemeSnapshot) => void): PluginUnsubscribe;
  onNetworkChanged(callback: (network: PluginNetworkSnapshot) => void): PluginUnsubscribe;
  onRoomEvent(callback: (event: PluginRoomEvent) => void): PluginUnsubscribe;
  onAuthoritativeRoomEvent(callback: (event: PluginAuthoritativeRoomEvent) => void): PluginUnsubscribe;
  onRoomPresenceChanged(callback: (event: PluginRoomPresenceEvent) => void): PluginUnsubscribe;
  onVisibilityChanged(callback: (visible: boolean) => void): PluginUnsubscribe;
}

export interface PluginAppApi {
  getVersion(): Promise<string>;
  getInstance(): Promise<{ pluginId: string; instanceId: string }>;
  close(): Promise<void>;
}

export interface PluginThemeApi {
  current(): Promise<PluginThemeSnapshot>;
}

export interface PluginDeviceSummary {
  id: string;
  name: string;
  online: boolean;
  platform?: string;
}

export interface PluginDeviceApi {
  list(): Promise<PluginDeviceSummary[]>;
}

export interface PluginChatSendInput {
  conversationId: string;
  text: string;
  metadata?: Readonly<Record<string, unknown>>;
}

export interface PluginChatApi {
  send(input: PluginChatSendInput): Promise<{ messageId: string }>;
}

export interface PluginRoomSummary {
  roomId: string;
  gameId: string;
  ownerPeerId: string;
  memberPeerIds: string[];
  schemaVersion: number;
  admission?: PluginRoomAdmission;
  roomVersion?: number;
  revision?: number;
}

export interface PluginAuthoritativeRoomSummary extends PluginRoomSummary {
  admission: PluginRoomAdmission;
  roomVersion: number;
  revision: number;
}

export interface PluginRoomInvite {
  inviteId: string;
  /** Opaque capability token minted and validated by the host. */
  token: string;
  roomId: string;
  roomVersion: number;
  expiresAt: number;
  pluginId: string;
}

export interface PluginRoomRecovery {
  room: PluginAuthoritativeRoomSummary;
  headRevision: number;
  checkpoint: PluginRoomCheckpoint | null;
  commits: PluginAuthoritativeCommit[];
}

export type PluginRoomInviteFailureReason = "expired" | "full" | "closed" | "not_found" | "incompatible" | "stale";

export type PluginRoomInviteResolution =
  | { ok: true; room: PluginAuthoritativeRoomSummary }
  | { ok: false; reason: PluginRoomInviteFailureReason; message: string };

export type PluginRoomIntentInput = Omit<PluginRoomIntent, "kind" | "senderPeerId">;
export type PluginAuthoritativeCommitInput = Omit<PluginAuthoritativeCommit, "kind" | "commitId" | "revision">;
export type PluginRoomCheckpointInput = Omit<PluginRoomCheckpoint, "checksum">;

export interface PluginRoomApi {
  list(input?: { gameId?: string }): Promise<PluginRoomSummary[]>;
  create(input: { gameId: string; name: string; maxPlayers: number; schemaVersion: number }): Promise<PluginRoomSummary>;
  join(input: { roomId: string }): Promise<PluginRoomSummary>;
  leave(input: { roomId: string }): Promise<void>;
  send(input: { roomId: string; type: string; payload: unknown; idempotencyKey: string }): Promise<void>;
  snapshot<TResult = unknown>(input: { roomId: string }): Promise<TResult>;
  invite(input: { roomId: string }): Promise<PluginRoomInvite>;
  sendIntent(input: PluginRoomIntentInput): Promise<void>;
  commit(input: PluginAuthoritativeCommitInput): Promise<PluginAuthoritativeCommit>;
  publishCheckpoint(input: PluginRoomCheckpointInput): Promise<PluginRoomCheckpoint>;
  recover(input: { roomId: string; afterRevision: number }): Promise<PluginRoomRecovery>;
  setAdmission(input: { roomId: string; admission: PluginRoomAdmission }): Promise<PluginAuthoritativeRoomSummary>;
  resolveInvite(input: { inviteId: string; token: string }): Promise<PluginRoomInviteResolution>;
}

export interface PluginLeaderboardEntry {
  peerId: string;
  nickname: string;
  score: number;
  wins?: number;
  losses?: number;
  updatedAt: number;
}

export interface PluginLegacyLeaderboardSubmitInput {
  gameId: string;
  roomId: string;
  idempotencyKey: string;
  result: unknown;
}

export interface PluginAuthoritativeLeaderboardSubmitInput {
  gameId: string;
  roomId: string;
  terminalRevision: number;
  terminalCommitId: string;
  /** SHA-256 digest of canonical JSON as defined by {@link PLUGIN_HASH_CONTRACT}. */
  stateHash: string;
  result: unknown;
}

export interface PluginLeaderboardApi {
  list(input: { gameId: string; limit?: number }): Promise<PluginLeaderboardEntry[]>;
  submit(input: PluginLegacyLeaderboardSubmitInput): Promise<void>;
  submit(input: PluginAuthoritativeLeaderboardSubmitInput): Promise<void>;
}

export interface PluginStorageApi {
  get<TValue = unknown>(key: string): Promise<TValue | null>;
  set<TValue = unknown>(key: string, value: TValue): Promise<void>;
  delete(key: string): Promise<void>;
  keys(): Promise<string[]>;
}

export interface PluginUiApi {
  notify(input: { title: string; body: string; level?: "info" | "success" | "warning" | "error" }): Promise<void>;
  confirm(input: { title: string; body: string; confirmText?: string; cancelText?: string }): Promise<boolean>;
  pickFile(input?: { title?: string; extensions?: string[]; multiple?: boolean }): Promise<string[]>;
}

export interface PluginLoggerApi {
  debug(message: string, details?: unknown): Promise<void>;
  info(message: string, details?: unknown): Promise<void>;
  warn(message: string, details?: unknown): Promise<void>;
  error(message: string, details?: unknown): Promise<void>;
}

export interface LanChatPluginApiV1 {
  readonly apiVersion: "1.0";
  readonly app: PluginAppApi;
  readonly events: PluginEventApi;
  readonly devices: PluginDeviceApi;
  readonly chat: PluginChatApi;
  readonly rooms: PluginRoomApi;
  readonly leaderboard: PluginLeaderboardApi;
  readonly storage: PluginStorageApi;
  readonly theme: PluginThemeApi;
  readonly ui: PluginUiApi;
  readonly logger: PluginLoggerApi;
}

export interface PluginBridgeTransport {
  request<TResult = unknown>(method: string, params?: unknown): Promise<TResult>;
  subscribe<TPayload>(event: string, callback: (payload: TPayload) => void): PluginUnsubscribe;
}
