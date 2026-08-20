/** 原生视觉运行时的 Raw RGBA 采样契约。像素只在本机 IPC 中传递。 */
export type VisionFrameSample = {
  streamId: string;
  streamGeneration: number;
  frameId: number;
  capturedAt: number;
  width: number;
  height: number;
  stride: number;
  rgba: Uint8Array;
};

export type VisionRuntimeSnapshot = {
  lifecycle: "disabled" | "initializing" | "ready" | "rebuildingSession" | "rollingBack" | "failed";
  sampling: "running" | "pausedByUser" | "pausedByResourceConflict" | "starved";
  performance: "normal" | "degraded" | "recovering";
  activeProfileId?: string | null;
  activeProfileVersion?: string | null;
  revision: number;
  reasonCode?: string | null;
};

export type VisionRuntimeDiagnostics = {
  acceptedFrames: number;
  droppedFrames: number;
  processedFrames: number;
  p50ProcessingMs: number;
  p95ProcessingMs: number;
  estimatedMemoryBytes: number;
  workerQueueDepth: number;
  streamResets: number;
};

export type VisionProfileSummary = {
  profileId: string;
  profileVersion: string;
  displayName: string;
  tier: "lightweight" | "balanced" | "experimental";
  installed: boolean;
  active: boolean;
  compatible: boolean;
  compatibilityReason?: string | null;
};
