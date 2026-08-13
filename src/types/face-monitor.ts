export type CameraMonitorSettings = {
  enabled: boolean;
  deviceId?: string | null;
  pauseDuringCall: boolean;
  sampleFps: number;
};

export type CameraMonitorStatus = {
  supported: boolean;
  enabled: boolean;
  cameraActive: boolean;
  callUsingCamera: boolean;
  sampling: boolean;
  sampleFps: number;
  modelReady?: boolean;
  queueBusy?: boolean;
  acceptedFrames?: number;
  droppedFrames?: number;
  lastError?: string | null;
};

export type FaceMonitorRuntimeStatus = {
  supported: boolean;
  enabled: boolean;
  modelAssetsReady?: boolean;
  modelReady: boolean;
  recognizerReady?: boolean;
  queueBusy: boolean;
  acceptedFrames: number;
  droppedFrames: number;
  modelVersion?: string | null;
  lastDetectionScore?: number | null;
  detectedFaces?: number;
  lastError?: string | null;
};

export type FacePersonAction = "upsert" | "disable" | "delete";

export type FacePersonPolicy = {
  personId: string;
  displayName: string;
  photoUrl?: string | null;
  photoSha256?: string | null;
  expiresAt?: number | null;
  enabled: boolean;
  version: number;
  action?: FacePersonAction;
  issuedByDeviceId: string;
  issuedByNickname: string;
  issuedAt: number;
  deletedAt?: number | null;
  embeddingModelVersion?: string | null;
  hasEmbedding?: boolean;
};

export type FaceMonitorPolicy = {
  targetDeviceId: string;
  minConfidence: number;
  consecutiveHits: number;
  cooldownSeconds: number;
  version: number;
  issuedByDeviceId: string;
  issuedByNickname: string;
  issuedAt: number;
};

export type CameraFaceAlert = {
  alertId: string;
  sourceKind?: string;
  sourceDeviceId: string;
  sourceNickname: string;
  sourceAddress?: string | null;
  personId: string;
  personName: string;
  confidence: number;
  consecutiveHits: number;
  policyVersion: number;
  createdAt: number;
  feedbackReal: number;
  feedbackFalse: number;
};

export type CameraFrameSample = {
  bytes: Uint8Array;
  mimeType: "image/jpeg";
  width: number;
  height: number;
  capturedAt: number;
};

export const DEFAULT_CAMERA_MONITOR_SETTINGS: CameraMonitorSettings = {
  enabled: false,
  deviceId: null,
  pauseDuringCall: false,
  sampleFps: 2,
};
