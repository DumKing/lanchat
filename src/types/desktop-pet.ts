export type PetStateKind = "Idle" | "Alert" | "Move" | "Interact" | "Life";
export type PetPackageSource = "built_in" | "portable" | "user";

export type PetStatePlaybackConfig = {
  minDurationMs: number;
  maxDurationMs: number;
  minActionCount: number;
  maxActionCount: number;
  minIntervalMs: number;
  maxIntervalMs: number;
};

export type PetStateManifestConfig = Partial<PetStatePlaybackConfig> & {
  loop?: "repeat" | "once" | "ping-pong";
};

export type PetManifest = {
  schemaVersion: number;
  id: string;
  name: string;
  version: string;
  author?: string;
  description?: string;
  resolution: number;
  fps: number;
  transparent: boolean;
  defaultState: string;
  states: Partial<Record<PetStateKind, PetStateManifestConfig>>;
  clips?: Record<string, {
    fps?: number;
    loop?: "repeat" | "once" | "ping-pong";
    direction?: "left" | "right";
    weight?: number;
  }>;
};

export type PetFrame = {
  path: string;
  width: number;
  height: number;
};

export type PetClip = {
  id: string;
  state: PetStateKind;
  frames: PetFrame[];
  fps: number;
  loop_mode: string;
  direction?: string | null;
  weight: number;
};

export type DesktopPetPackage = {
  manifest: PetManifest;
  source: PetPackageSource;
  root: string;
  preview_path?: string | null;
  icon_path?: string | null;
  states: Partial<Record<PetStateKind, PetClip[]>>;
  warnings: string[];
};

export type DesktopPetPackageIssue = {
  root: string;
  source: PetPackageSource;
  error: string;
};

export type DesktopPetRegistrySnapshot = {
  packages: DesktopPetPackage[];
  issues: DesktopPetPackageIssue[];
};

export type ExternalPushKind = "wechat_work" | "dingtalk";

export type ExternalPushConfig = {
  id: string;
  name: string;
  kind: ExternalPushKind;
  webhook: string;
  enabled: boolean;
  mentionAll: boolean;
  template: string;
};

export type DesktopPetSettings = {
  enabled: boolean;
  selectedPetId?: string | null;
  scale: number;
  positionX?: number | null;
  positionY?: number | null;
  monitorId?: string | null;
  alertMode: string;
  sendHotkey: string;
  stopHotkey: string;
  randomMoveEnabled: boolean;
  randomLifeEnabled: boolean;
  discoMovementMode: "linear" | "jump";
  externalPushEnabled: boolean;
  externalPushMinCredibility: number;
  externalPushMinCredibilityLocked: boolean;
  externalPushConfigs: ExternalPushConfig[];
};
