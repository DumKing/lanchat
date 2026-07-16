export type PetStateKind = "Idle" | "Alert" | "Move" | "Interact" | "Life";
export type PetPackageSource = "built_in" | "portable" | "user";

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

export type DesktopPetSettings = {
  enabled: boolean;
  selectedPetId?: string | null;
  scale: number;
  positionX?: number | null;
  positionY?: number | null;
  monitorId?: string | null;
  alertMode: string;
  stopHotkey: string;
  randomMoveEnabled: boolean;
  randomLifeEnabled: boolean;
};

