import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, lib, runtime, storage, types] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/face_monitor.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/storage.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/types/face-monitor.ts", import.meta.url), "utf8"),
]);

// 匿名“检测到人脸”告警链路已彻底移除，不再产生任何告警事件和数据。
assert.doesNotMatch(lib, /publish_camera_face_presence/, "匿名人脸出现告警链路必须完全移除");
assert.doesNotMatch(app, /camera_face_presence/, "前端不应再处理匿名人脸出现告警");

// 识别编排：检测→逐脸特征→模板比对，仅识别命中才告警。
assert.match(runtime, /pub fn recognize_frame/, "运行时必须提供识别编排入口");
assert.match(runtime, /pub fn extract_embedding/, "运行时必须支持对齐后的 SFace 特征提取");
assert.match(runtime, /pub fn embedding_from_photo_bytes/, "录入照片必须先检测再提取特征");
assert.match(runtime, /fn best_match/, "识别命中必须按余弦相似度取最佳匹配");
assert.match(lib, /load_recognition_templates/, "识别帧必须加载本机启用人员特征模板");
assert.match(lib, /recognizer_ready/, "识别模型未就绪时不应产生识别告警");
assert.match(lib, /source_kind: "camera_face"/, "识别告警必须使用具名来源标识");

// 录入时提取特征并落库，特征不出本机。
assert.match(lib, /update_face_person_embedding/, "录入人员后必须把特征写入本机数据库");
assert.match(storage, /embedding_model_version/, "特征必须携带模型版本以便升级重提取");
assert.match(storage, /source_kind/, "识别告警记录必须保留来源类型列");

// 前端状态展示：识别模型与人员特征状态可见，缺失时给出提示。
assert.match(app, /recognizerReady/, "设置页必须展示识别模型状态");
assert.match(app, /hasEmbedding/, "设置页必须展示人员特征提取状态");
assert.match(app, /尚未录入可用的识别人员/, "无可用人员时必须提示不会产生告警");
assert.match(types, /sourceKind/, "告警类型必须包含来源类型字段");

// 桌宠介入：识别告警独立驱动桌宠，并与狼来了严格区分。
assert.match(app, /facePetAlert\.value = \{/, "识别告警必须独立驱动桌宠覆盖状态");
assert.match(app, /【人脸识别】/, "桌宠识别告警文案必须与狼来了区分");
assert.match(app, /检测到 \$\{face\.personName\}/, "桌宠识别告警必须显示具名人员");

// 外部推送：识别告警走独立推送路径，不复用狼来了模板。
assert.match(lib, /fn render_camera_face_push_text/, "识别告警必须有独立推送文案");
assert.match(lib, /fn send_camera_face_external_push/, "识别告警必须有独立外部推送路径");
assert.match(lib, /fn send_external_push_alert/, "狼来了外部推送入口签名保持不变");
assert.match(lib, /\[人脸识别告警\]/, "识别告警推送文案必须带独立前缀");

console.log("face recognition alert guards passed");
