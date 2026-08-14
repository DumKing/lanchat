import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, api, rust, types, protocol, storage, lib, network] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/face_monitor.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/types/face-monitor.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/protocol.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/storage.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/network.rs", import.meta.url), "utf8"),
]);

assert.match(app, /title="摄像头自动告警"/, "设置页需要提供本机摄像头告警入口");
assert.match(app, /class="camera-face-settings-card"/, "摄像头设置应跨越设置网格整行展示");
assert.match(app, /previous === "camera" && next !== "camera"/, "离开摄像头设置时需要自动关闭检测预览");
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
assert.match(app, /摄像头人物识别告警（独立于狼来了）/, "UI 必须明确区分自动识别与狼来了告警");
assert.match(app, /上传本地照片/, "本机需要支持从本地上传人员参考照片");
assert.match(app, /摄像头拍照/, "本机需要支持摄像头采集人员参考照片");
assert.match(app, /删除本机配置/, "本机人员列表需要支持删除本地配置");
assert.match(app, /cameraMediaCoordinator\.acquirePreview/, "摄像头采集必须复用统一媒体协调器");
assert.match(types, /faceMinConfidence: number/, "本机设置应提供独立人脸识别阈值");
assert.match(types, /bodyMinConfidence: number/, "本机设置应提供独立人体增强阈值");
assert.match(types, /faceRecognitionEnabled: boolean/, "本机设置应提供人脸确认独立开关");
assert.match(types, /bodyRecognitionEnabled: boolean/, "本机设置应提供人体特征独立开关");
assert.match(types, /faceCooldownSeconds: number/, "本机设置应提供人脸确认独立冷却");
assert.match(types, /bodyCooldownSeconds: number/, "本机设置应提供人体特征独立冷却");
assert.match(types, /bodyCooldownSeconds:\s*300/, "人体特征默认冷却应为 5 分钟");
assert.match(rust, /body_cooldown_seconds:\s*300/, "Rust 人体特征默认冷却应为 5 分钟");
assert.match(app, /faceAdminBodyCooldownSeconds = ref\(300\)/, "超管人体冷却默认值应为 5 分钟");
assert.match(types, /consecutiveHits: number/, "本机设置应允许调整连续命中次数");
assert.match(types, /settingsLocked: boolean/, "远程策略应携带可锁定标志");
assert.match(protocol, /body_min_confidence: u8/, "局域网策略应传输人体增强阈值");
assert.match(protocol, /sample_fps: u8/, "局域网策略应传输检测频率");
assert.match(protocol, /face_cooldown_seconds: u32/, "局域网策略应传输人脸确认冷却");
assert.match(protocol, /body_cooldown_seconds: u32/, "局域网策略应传输人体特征冷却");
assert.match(protocol, /settings_locked: bool/, "局域网策略应传输锁定状态");
assert.match(storage, /body_min_confidence/, "SQLite 应保存人体增强阈值");
assert.match(storage, /face_cooldown_seconds/, "SQLite 应保存人脸确认冷却");
assert.match(storage, /body_cooldown_seconds/, "SQLite 应保存人体特征冷却");
assert.match(storage, /settings_locked/, "SQLite 应保存策略锁定状态");
assert.match(lib, /policy\.body_min_confidence/, "人体疑似识别应使用独立人体阈值");
assert.match(lib, /settings\.face_recognition_enabled/, "人脸确认开关必须参与运行时门控");
assert.match(lib, /settings\.body_recognition_enabled/, "人体特征开关必须参与运行时门控");
assert.match(lib, /policy\.face_cooldown_seconds/, "人脸确认应使用独立冷却");
assert.match(lib, /policy\.body_cooldown_seconds/, "人体特征应使用独立冷却");
assert.match(app, /legacyCooldownSeconds[^\n]*saved\.cooldownSeconds/, "旧版单冷却配置必须保留迁移来源");
assert.match(app, /faceCooldownSeconds:[^\n]*legacyCooldownSeconds/, "旧版单冷却必须迁移为人脸冷却");
assert.match(app, /bodyCooldownSeconds:[^\n]*legacyCooldownSeconds/, "旧版单冷却必须迁移为人体冷却");
assert.match(app, /人脸确认识别/, "设置页应提供人脸确认独立开关");
assert.match(app, /人体特征识别/, "设置页应提供人体特征独立开关");
assert.match(app, /人脸重复冷却/, "设置页应提供人脸确认独立冷却");
assert.match(app, /人体重复冷却/, "设置页应提供人体特征独立冷却");
assert.match(app, /重复冷却不参与锁定/, "设置页应明确冷却时间始终可本机修改");
assert.match(app, /:disabled="[^"]*faceMonitorPolicy\?\.settingsLocked[^"]*"/, "被超管锁定后本机策略控件应禁用");
assert.match(app, /mentionAll: true/, "新建外部推送配置应默认勾选提醒所有人");
assert.match(lib, /send_camera_face_external_push[\s\S]*external_push_payload\(&config\.kind, true, &content\)/, "人物识别推送应通过机器人协议字段提醒所有人");
assert.match(app, /faceAdminPhotoPreviews/, "超管选择多张参考照片后必须生成可见预览");
assert.match(app, /class="face-admin-photo-preview-list"/, "超管人员策略必须展示已选照片缩略图列表");
assert.match(app, /removeFaceAdminPhoto/, "已选参考照片必须支持单张移除");
assert.doesNotMatch(app, /参考照片不能超过 5MB/, "人员参考照片不应再有 5MB 前端限制");
assert.doesNotMatch(lib, /每张人员照片必须是 5MB 以内的本地文件/, "超管下发人员照片不应再有 5MB 后端限制");
assert.doesNotMatch(network, /人员参考照片超过 5MB 或内容为空/, "接收下发人员照片时不应再按 5MB 拒绝");
assert.match(app, /'admin-settings-grid': settingsCategory === 'admin'/, "超管设置必须切换为独立全宽单列布局");
assert.match(app, /\.admin-settings-grid\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/, "超管设置卡片必须从上到下铺满整行");
const adminNotificationIndex = app.indexOf('title="超管通知"');
const adminRemoteUpdateIndex = app.indexOf('title="指定设备强制更新"');
const cameraPolicyIndex = app.indexOf('title="摄像头人物识别策略"');
assert.ok(adminNotificationIndex >= 0 && adminNotificationIndex < adminRemoteUpdateIndex && adminRemoteUpdateIndex < cameraPolicyIndex, "摄像头任务策略必须排在超管设置第三项");

console.log("face monitor settings guards passed");
