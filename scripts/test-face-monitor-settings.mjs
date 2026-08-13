import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, api, rust] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/face_monitor.rs", import.meta.url), "utf8"),
]);

assert.match(app, /title="摄像头自动告警"/, "设置页需要提供本机摄像头告警入口");
assert.match(app, /视频通话期间暂停识别/, "设置页需要提供通话期间隐私开关");
assert.match(app, /initializeFaceMonitor\(\)/, "应用启动时需要恢复本机摄像头设置");
assert.match(app, /cameraMediaCoordinator\.subscribeFrames/, "低清采样帧必须走统一协调器");
assert.match(app, /submitFaceMonitorFrame\(sample\)/, "采样帧必须经由有界 Rust 入口处理");
assert.match(api, /get_face_monitor_status/, "前端需要读取识别运行状态");
assert.match(rust, /accepted_frames/, "Rust 运行时需要统计已接收帧");
assert.match(rust, /bytes\.is_empty\(\)/, "Rust 运行时必须拒绝无效帧");
assert.match(api, /sendFacePersonPolicy/, "前端需要提供超管人员规则下发命令");
assert.match(api, /sendFaceMonitorPolicy/, "前端需要提供超管识别策略下发命令");
assert.match(api, /listCameraFaceAlerts/, "自动识别告警必须有独立历史读取接口");
assert.match(api, /sendCameraFaceAlertFeedback/, "自动识别告警必须有独立反馈接口");
assert.match(app, /摄像头人脸识别告警（独立于狼来了）/, "UI 必须明确区分自动识别与狼来了告警");
assert.match(app, /上传本地照片/, "本机需要支持从本地上传人员参考照片");
assert.match(app, /摄像头拍照/, "本机需要支持摄像头采集人员参考照片");
assert.match(app, /删除本机配置/, "本机人员列表需要支持删除本地配置");
assert.match(app, /cameraMediaCoordinator\.acquirePreview/, "摄像头采集必须复用统一媒体协调器");

console.log("face monitor settings guards passed");
