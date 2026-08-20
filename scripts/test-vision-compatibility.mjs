import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, api, coordinator, lib] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/services/cameraMediaCoordinator.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
]);

assert.match(api, /submitVisionFrameRaw/, "新帧入口必须存在");
assert.doesNotMatch(api, /submit_face_monitor_frame/, "前端不得继续调用旧 JPEG 命令");
assert.match(app, /camera_face_alert_received/, "新运行时必须继续发布并消费既有自动告警事件");
assert.match(app, /feedbackCameraFaceAlert/, "自动告警必须继续支持真实或虚假反馈");
assert.match(app, /latestCameraFrameSample/, "仅本机告警才生成临时预览图");
assert.match(coordinator, /sourceVideoTrack\.clone\(\)/, "视频通话必须使用摄像头克隆轨道");
assert.match(coordinator, /this\.videoCallActive = false/, "结束通话必须显式释放通话轨道");
assert.match(coordinator, /this\.monitoringActive\(\)/, "监控 Lease 必须独立于通话状态");
assert.match(lib, /process_face_monitor_frame/, "Raw Worker 必须复用兼容告警处理逻辑");
assert.match(lib, /VisionWorker::start/, "识别必须在专用 Worker 中运行");

console.log("vision compatibility guards passed");
