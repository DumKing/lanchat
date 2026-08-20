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
  lifecycle: "running" | "paused" | "starved" | "failed";
  samplingState: "running" | "paused_by_user" | "starved";
  performanceState: "normal" | "degraded" | "overloaded";
  activeProfileId?: string | null;
  activeProfileVersion?: string | null;
  acceptedFrames: number;
  droppedFrames: number;
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
