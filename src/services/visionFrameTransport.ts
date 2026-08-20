import type { VisionFrameSample } from "../types/vision";

const HEADER_LENGTH = 60;
const MAGIC = [0x4c, 0x43, 0x56, 0x46];
const SCHEMA_VERSION = 1;

type VisionFrameInput = Omit<VisionFrameSample, "streamId" | "streamGeneration" | "frameId">;

function uuidToBytes(value: string): Uint8Array {
  const normalized = value.replace(/-/g, "").toLowerCase();
  if (!/^[0-9a-f]{32}$/.test(normalized)) throw new Error("Vision stream id must be a UUID");
  const bytes = new Uint8Array(16);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(normalized.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function bytesToUuid(bytes: Uint8Array): string {
  const hex = [...bytes].map((value) => value.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function createStreamId(): string {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  throw new Error("当前环境不支持安全的视觉流标识生成");
}

function toSafeBigInt(value: number, name: string): bigint {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`${name} must be a positive safe integer`);
  return BigInt(value);
}

/** 编码格式与 Rust `decode_raw_frame` 的 60 字节 LCVF 头完全一致。 */
export function encodeVisionFrameEnvelope(frame: VisionFrameSample): Uint8Array {
  const stride = frame.stride ?? frame.width * 4;
  if (!Number.isInteger(frame.width) || !Number.isInteger(frame.height) || frame.width < 1 || frame.height < 1) {
    throw new Error("Vision frame dimensions are invalid");
  }
  if (stride < frame.width * 4 || frame.rgba.byteLength !== stride * frame.height) {
    throw new Error("Vision frame payload length does not match dimensions");
  }
  const bytes = new Uint8Array(HEADER_LENGTH + frame.rgba.byteLength);
  const view = new DataView(bytes.buffer);
  bytes.set(MAGIC, 0);
  view.setUint16(4, SCHEMA_VERSION, true);
  view.setUint8(6, 1); // RGBA8
  view.setUint8(7, 0);
  bytes.set(uuidToBytes(frame.streamId), 8);
  view.setBigUint64(24, toSafeBigInt(frame.streamGeneration, "stream generation"), true);
  view.setBigUint64(32, toSafeBigInt(frame.frameId, "frame id"), true);
  view.setBigUint64(40, toSafeBigInt(frame.capturedAt, "capture timestamp"), true);
  view.setUint16(48, frame.width, true);
  view.setUint16(50, frame.height, true);
  view.setUint32(52, stride, true);
  view.setUint32(56, frame.rgba.byteLength, true);
  bytes.set(frame.rgba, HEADER_LENGTH);
  return bytes;
}

export function readVisionFrameEnvelope(bytes: Uint8Array): VisionFrameSample {
  if (bytes.byteLength < HEADER_LENGTH) throw new Error("Vision frame header is invalid");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (!MAGIC.every((value, index) => bytes[index] === value)) throw new Error("Vision frame magic is invalid");
  if (view.getUint16(4, true) !== SCHEMA_VERSION || view.getUint8(6) !== 1) throw new Error("Vision frame schema is unsupported");
  const width = view.getUint16(48, true);
  const height = view.getUint16(50, true);
  const stride = view.getUint32(52, true);
  const payloadLength = view.getUint32(56, true);
  if (!width || !height || stride < width * 4 || payloadLength !== stride * height || bytes.byteLength !== HEADER_LENGTH + payloadLength) {
    throw new Error("Vision frame payload length is invalid");
  }
  return {
    streamId: bytesToUuid(bytes.slice(8, 24)),
    streamGeneration: Number(view.getBigUint64(24, true)),
    frameId: Number(view.getBigUint64(32, true)),
    capturedAt: Number(view.getBigUint64(40, true)),
    width,
    height,
    stride,
    rgba: bytes.slice(HEADER_LENGTH),
  };
}

/** 维护前端摄像头流身份；每次重新获取视频流都必须调用 reset。 */
export class VisionFrameTransport {
  private streamId: string;
  private streamGeneration = 1;
  private frameId = 0;

  constructor(streamId = createStreamId()) {
    this.streamId = streamId;
  }

  reset(streamId = createStreamId()): Pick<VisionFrameSample, "streamId" | "streamGeneration"> {
    this.streamId = streamId;
    this.streamGeneration += 1;
    this.frameId = 0;
    return { streamId: this.streamId, streamGeneration: this.streamGeneration };
  }

  next(sample: VisionFrameInput): VisionFrameSample {
    this.frameId += 1;
    return {
      ...sample,
      streamId: this.streamId,
      streamGeneration: this.streamGeneration,
      frameId: this.frameId,
    };
  }

  encode(sample: VisionFrameInput): Uint8Array {
    return encodeVisionFrameEnvelope(this.next(sample));
  }

  readVisionFrameEnvelope(bytes: Uint8Array): VisionFrameSample {
    return readVisionFrameEnvelope(bytes);
  }
}
