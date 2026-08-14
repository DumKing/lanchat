import type { CameraFrameSample, CameraMonitorSettings, CameraMonitorStatus } from "../types/face-monitor";

type StatusListener = (status: CameraMonitorStatus) => void;
type FrameListener = (sample: CameraFrameSample) => void | Promise<void>;

const clampFps = (value: number) => Math.max(1, Math.min(5, Math.round(value || 2)));

/** Owns the only camera track used by both face monitoring and WebRTC calls. */
class CameraMediaCoordinator {
  private settings: CameraMonitorSettings = {
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
  private videoStream: MediaStream | null = null;
  private callVideoTrack: MediaStreamTrack | null = null;
  private callAudioStream: MediaStream | null = null;
  private videoCallActive = false;
  private previewActive = false;
  private frameTimer: number | null = null;
  private samplingBusy = false;
  private samplingAllowed = false;
  private statusListeners = new Set<StatusListener>();
  private frameListeners = new Set<FrameListener>();
  private lastError: string | null = null;
  private samplerVideo: HTMLVideoElement | null = null;
  private samplerCanvas: HTMLCanvasElement | null = null;

  getStatus(): CameraMonitorStatus {
    const sampling = !!this.frameTimer && this.monitoringActive() && !(this.videoCallActive && this.settings.pauseDuringCall);
    return {
      supported: true,
      enabled: this.settings.enabled,
      cameraActive: !!this.videoStream?.getVideoTracks().some((track) => track.readyState === "live"),
      callUsingCamera: this.videoCallActive,
      sampling,
      sampleFps: this.effectiveSampleFps(),
      lastError: this.lastError,
    };
  }

  subscribeStatus(listener: StatusListener) {
    this.statusListeners.add(listener);
    listener(this.getStatus());
    return () => this.statusListeners.delete(listener);
  }

  subscribeFrames(listener: FrameListener) {
    this.frameListeners.add(listener);
    this.updateSampling();
    return () => {
      this.frameListeners.delete(listener);
      this.updateSampling();
    };
  }

  async updateMonitorSettings(next: Partial<CameraMonitorSettings>) {
    this.settings = { ...this.settings, ...next, sampleFps: clampFps(next.sampleFps ?? this.settings.sampleFps) };
    if (this.monitoringActive()) await this.ensureVideoTrack();
    this.releaseUnusedTracks();
    this.updateSampling();
    this.emitStatus();
  }

  /** The native runtime enables sampling only after local model validation. */
  setSamplingAllowed(allowed: boolean) {
    this.samplingAllowed = allowed;
    this.updateSampling();
    this.emitStatus();
  }

  async acquireForCall(media: "audio" | "video"): Promise<MediaStream> {
    try {
      const tracks: MediaStreamTrack[] = [];
      if (media === "video") {
        this.videoCallActive = true;
        const sourceVideoTrack = await this.ensureVideoTrack();
        this.callVideoTrack?.stop();
        this.callVideoTrack = sourceVideoTrack.clone();
        tracks.push(this.callVideoTrack);
      }
      const audio = await this.ensureAudioTrack();
      tracks.unshift(audio);
      this.lastError = null;
      this.updateSampling();
      this.emitStatus();
      return new MediaStream(tracks);
    } catch (error) {
      this.callVideoTrack?.stop();
      this.callVideoTrack = null;
      this.videoCallActive = false;
      this.lastError = error instanceof Error ? error.message : String(error);
      this.emitStatus();
      throw error;
    }
  }

  async acquirePreview(): Promise<MediaStream> {
    this.previewActive = true;
    const video = await this.ensureVideoTrack();
    this.emitStatus();
    return new MediaStream([video]);
  }

  releasePreview() {
    this.previewActive = false;
    this.releaseUnusedTracks();
    this.emitStatus();
  }

  releaseCall(media: "audio" | "video") {
    if (media === "video") {
      this.videoCallActive = false;
      this.callVideoTrack?.stop();
      this.callVideoTrack = null;
    }
    this.stopStream(this.callAudioStream);
    this.callAudioStream = null;
    this.releaseUnusedTracks();
    this.updateSampling();
    this.emitStatus();
  }

  dispose() {
    if (this.frameTimer !== null) window.clearInterval(this.frameTimer);
    this.frameTimer = null;
    this.stopStream(this.videoStream);
    this.stopStream(this.callAudioStream);
    this.callVideoTrack?.stop();
    this.videoStream = null;
    this.callAudioStream = null;
    this.callVideoTrack = null;
    this.frameListeners.clear();
    this.statusListeners.clear();
  }

  private async ensureVideoTrack() {
    const current = this.videoStream?.getVideoTracks().find((track) => track.readyState === "live");
    if (current) return current;
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("当前环境不支持摄像头访问");
    const deviceId = this.settings.deviceId?.trim();
    this.videoStream = await navigator.mediaDevices.getUserMedia({ video: deviceId ? { deviceId: { exact: deviceId } } : true, audio: false });
    return this.videoStream.getVideoTracks()[0];
  }

  private async ensureAudioTrack() {
    const current = this.callAudioStream?.getAudioTracks().find((track) => track.readyState === "live");
    if (current) return current;
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("当前环境不支持麦克风访问");
    this.callAudioStream = await navigator.mediaDevices.getUserMedia({ video: false, audio: true });
    return this.callAudioStream.getAudioTracks()[0];
  }

  private effectiveSampleFps() {
    return this.videoCallActive ? 1 : clampFps(this.settings.sampleFps);
  }

  private monitoringActive() {
    return this.settings.enabled && (this.settings.faceRecognitionEnabled || this.settings.bodyRecognitionEnabled);
  }

  private updateSampling() {
    const enabled = this.samplingAllowed && this.monitoringActive() && this.frameListeners.size > 0 && !!this.videoStream && !(this.videoCallActive && this.settings.pauseDuringCall);
    if (this.frameTimer !== null) {
      window.clearInterval(this.frameTimer);
      this.frameTimer = null;
    }
    if (!enabled) return;
    const interval = Math.round(1000 / this.effectiveSampleFps());
    this.frameTimer = window.setInterval(() => void this.captureFrame(), interval);
    void this.captureFrame();
  }

  private async captureFrame() {
    if (this.samplingBusy || !this.videoStream || this.frameListeners.size === 0) return;
    const track = this.videoStream.getVideoTracks()[0];
    if (!track || track.readyState !== "live") return;
    this.samplingBusy = true;
    try {
      const video = this.ensureSamplerVideo();
      if (video.srcObject !== this.videoStream) video.srcObject = this.videoStream;
      if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) await video.play().catch(() => undefined);
      if (!video.videoWidth || !video.videoHeight) return;
      const longest = 320;
      const scale = Math.min(1, longest / Math.max(video.videoWidth, video.videoHeight));
      const width = Math.max(1, Math.round(video.videoWidth * scale));
      const height = Math.max(1, Math.round(video.videoHeight * scale));
      const canvas = this.ensureSamplerCanvas();
      canvas.width = width;
      canvas.height = height;
      canvas.getContext("2d", { alpha: false })?.drawImage(video, 0, 0, width, height);
      const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.72));
      if (!blob) return;
      const sample: CameraFrameSample = { bytes: new Uint8Array(await blob.arrayBuffer()), mimeType: "image/jpeg", width, height, capturedAt: Date.now() };
      for (const listener of this.frameListeners) await listener(sample);
    } catch (error) {
      this.lastError = error instanceof Error ? error.message : String(error);
      this.emitStatus();
    } finally {
      this.samplingBusy = false;
    }
  }

  private ensureSamplerVideo() {
    if (!this.samplerVideo) {
      this.samplerVideo = document.createElement("video");
      this.samplerVideo.muted = true;
      this.samplerVideo.playsInline = true;
    }
    return this.samplerVideo;
  }

  private ensureSamplerCanvas() {
    this.samplerCanvas ??= document.createElement("canvas");
    return this.samplerCanvas;
  }

  private releaseUnusedTracks() {
    if (!this.monitoringActive() && !this.videoCallActive && !this.previewActive) {
      this.stopStream(this.videoStream);
      this.videoStream = null;
    }
  }

  private stopStream(stream: MediaStream | null) {
    stream?.getTracks().forEach((track) => track.stop());
  }

  private emitStatus() {
    const status = this.getStatus();
    for (const listener of this.statusListeners) listener(status);
  }
}

export const cameraMediaCoordinator = new CameraMediaCoordinator();
