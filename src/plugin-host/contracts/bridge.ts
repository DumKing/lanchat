export const PLUGIN_BRIDGE_ERROR_CODES = [
  "BAD_REQUEST",
  "UNSUPPORTED_API_VERSION",
  "UNKNOWN_METHOD",
  "INVALID_INSTANCE_TOKEN",
  "PERMISSION_DENIED",
  "SCHEMA_VALIDATION_FAILED",
  "RATE_LIMITED",
  "HOST_TIMEOUT",
  "PLUGIN_STOPPED",
  "INTERNAL_ERROR",
] as const;

export type PluginBridgeErrorCode = typeof PLUGIN_BRIDGE_ERROR_CODES[number];

export interface PluginBridgeRequest<TParams = unknown> {
  id: string;
  apiVersion: "1.0";
  instanceToken: string;
  method: string;
  params: TParams;
}

export interface PluginBridgeError {
  code: PluginBridgeErrorCode;
  message: string;
  retryable: boolean;
}

export type PluginBridgeResponse<TResult = unknown> =
  | { id: string; ok: true; result: TResult }
  | { id: string; ok: false; error: PluginBridgeError };

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
  stateHash: string;
  terminal?: { resultHash: string };
}

export type PluginAuthoritativeRoomEvent = PluginRoomIntent | PluginAuthoritativeCommit;

export interface PluginRoomCheckpoint {
  roomId: string;
  revision: number;
  protocolVersion: number;
  payload: unknown;
  stateHash: string;
  checksum: string;
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

export interface PluginRoomPresenceEvent {
  roomId: string;
  peerId: string;
  status: "online" | "offline" | "recovered";
  occurredAt: number;
}

export type PluginUnsubscribe = () => void;

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
  stateHash: string;
  result: unknown;
}

// The built-in SDK is the contract source. These assertions keep the host bridge mirror structurally compatible.
import type {
  PluginAuthoritativeCommit as SdkPluginAuthoritativeCommit,
  PluginAuthoritativeLeaderboardSubmitInput as SdkPluginAuthoritativeLeaderboardSubmitInput,
  PluginAuthoritativeRoomEvent as SdkPluginAuthoritativeRoomEvent,
  PluginAuthoritativeRoomSummary as SdkPluginAuthoritativeRoomSummary,
  PluginLegacyLeaderboardSubmitInput as SdkPluginLegacyLeaderboardSubmitInput,
  PluginRoomCheckpoint as SdkPluginRoomCheckpoint,
  PluginRoomEvent as SdkPluginRoomEvent,
  PluginRoomInvite as SdkPluginRoomInvite,
  PluginRoomInviteResolution as SdkPluginRoomInviteResolution,
  PluginRoomRecovery as SdkPluginRoomRecovery,
} from "../../../packages/plugin-sdk/src/types";

type IsExactly<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends
  (<Value>() => Value extends Right ? 1 : 2)
    ? (<Value>() => Value extends Right ? 1 : 2) extends
      (<Value>() => Value extends Left ? 1 : 2)
      ? true
      : false
    : false;
type Assert<Condition extends true> = Condition;

export type PluginBridgeContractCompatibilityAssertions = [
  Assert<IsExactly<PluginRoomEvent, SdkPluginRoomEvent>>,
  Assert<IsExactly<PluginAuthoritativeRoomEvent, SdkPluginAuthoritativeRoomEvent>>,
  Assert<IsExactly<PluginAuthoritativeCommit, SdkPluginAuthoritativeCommit>>,
  Assert<IsExactly<PluginRoomCheckpoint, SdkPluginRoomCheckpoint>>,
  Assert<IsExactly<PluginAuthoritativeRoomSummary, SdkPluginAuthoritativeRoomSummary>>,
  Assert<IsExactly<PluginRoomInvite, SdkPluginRoomInvite>>,
  Assert<IsExactly<PluginRoomRecovery, SdkPluginRoomRecovery>>,
  Assert<IsExactly<PluginRoomInviteResolution, SdkPluginRoomInviteResolution>>,
  Assert<IsExactly<PluginLegacyLeaderboardSubmitInput, SdkPluginLegacyLeaderboardSubmitInput>>,
  Assert<IsExactly<PluginAuthoritativeLeaderboardSubmitInput, SdkPluginAuthoritativeLeaderboardSubmitInput>>,
];
