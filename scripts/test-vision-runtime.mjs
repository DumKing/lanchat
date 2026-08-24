import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [api, coordinator, visionTypes] = await Promise.all([
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/services/cameraMediaCoordinator.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/types/vision.ts", import.meta.url), "utf8"),
]);

assert.match(visionTypes, /VisionFrameSample/, "需要定义 Raw RGBA 帧契约");
assert.match(api, /submitVisionFrameRaw/, "Tauri API 需要提交原始视觉帧");
assert.doesNotMatch(api, /submit_face_monitor_frame/, "新前端路径不得继续发送 JPEG 帧");
assert.match(coordinator, /getImageData\(0, 0, width, height\)/, "采样必须直接读取 RGBA 像素");
assert.doesNotMatch(coordinator, /toBlob\(/, "新采样路径不得重新编码 JPEG");

console.log("vision runtime API contract passed");
