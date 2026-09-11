class CallMediaCoordinator {
  private videoStream: MediaStream | null = null;
  private audioStream: MediaStream | null = null;

  async acquireForCall(media: "audio" | "video"): Promise<MediaStream> {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error(media === "video" ? "当前环境不支持摄像头访问" : "当前环境不支持麦克风访问");
    }
    this.releaseCall(media);
    if (media === "video") {
      this.videoStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
    }
    this.audioStream = await navigator.mediaDevices.getUserMedia({ video: false, audio: true });
    return new MediaStream([
      ...this.audioStream.getAudioTracks(),
      ...(this.videoStream?.getVideoTracks() ?? []),
    ]);
  }

  releaseCall(_media: "audio" | "video") {
    this.stopStream(this.videoStream);
    this.stopStream(this.audioStream);
    this.videoStream = null;
    this.audioStream = null;
  }

  dispose() {
    this.releaseCall("video");
  }

  private stopStream(stream: MediaStream | null) {
    stream?.getTracks().forEach((track) => track.stop());
  }
}

export const callMediaCoordinator = new CallMediaCoordinator();
