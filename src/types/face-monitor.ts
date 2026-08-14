export type CameraMonitorSettings = {
  enabled: boolean;
  faceRecognitionEnabled: boolean;
  bodyRecognitionEnabled: boolean;
  deviceId?: string | null;
  pauseDuringCall: boolean;
  sampleFps: number;
  faceMinConfidence: number;
  bodyMinConfidence: number;
  consecutiveHits: number;
  faceCooldownSeconds: number;
  bodyCooldownSeconds: number;
  appliedPolicyVersion?: number;
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
  personDetectorReady?: boolean;
  personRecognizerReady?: boolean;
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
  photoUrls?: string[];
  photoSha256?: string | null;
  photoSha256s?: string[];
  sampleCount?: number;
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
  hasBodyEmbedding?: boolean;
};

export type FaceMonitorPolicy = {
  targetDeviceId: string;
  minConfidence: number;
  bodyMinConfidence: number;
  sampleFps: number;
  consecutiveHits: number;
  /** Compatibility value for older peers; new clients use the split fields. */
  cooldownSeconds?: number;
  faceCooldownSeconds: number;
  bodyCooldownSeconds: number;
  settingsLocked: boolean;
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
  recognitionLevel?: "confirmed" | "suspected";
  faceConfidence?: number | null;
  bodyConfidence?: number | null;
  consecutiveHits: number;
  policyVersion: number;
  createdAt: number;
  feedbackReal: number;
  feedbackFalse: number;
  localFeedback?: "real" | "false" | null;
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
  faceRecognitionEnabled: true,
  bodyRecognitionEnabled: true,
  deviceId: null,
  pauseDuringCall: false,
  sampleFps: 2,
  faceMinConfidence: 60,
  bodyMinConfidence: 68,
  consecutiveHits: 1,
  faceCooldownSeconds: 60,
  bodyCooldownSeconds: 300,
  appliedPolicyVersion: 0,
};
