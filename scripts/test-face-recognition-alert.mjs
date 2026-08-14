import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, lib, runtime, storage, types, petRuntime, api, globalCss] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/face_monitor.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/storage.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/types/face-monitor.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/desktop_pet_runtime.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/styles/global.css", import.meta.url), "utf8"),
]);
const cameraFeedbackHandler = app.slice(
  app.indexOf("async function feedbackCameraFaceAlert"),
  app.indexOf("const facePolicyTargetOptions"),
);

// 匿名“检测到人脸”告警链路已彻底移除，不再产生任何告警事件和数据。
assert.doesNotMatch(lib, /publish_camera_face_presence/, "匿名人脸出现告警链路必须完全移除");
assert.doesNotMatch(app, /camera_face_presence/, "前端不应再处理匿名人脸出现告警");

// 识别编排：检测→逐脸特征→模板比对，仅识别命中才告警。
assert.match(runtime, /pub fn recognize_frame/, "运行时必须提供识别编排入口");
assert.match(runtime, /pub fn extract_embedding/, "运行时必须支持对齐后的 SFace 特征提取");
assert.match(runtime, /pub fn embedding_from_photo_bytes/, "录入照片必须先检测再提取特征");
assert.match(runtime, /fn best_match/, "识别命中必须按余弦相似度取最佳匹配");
assert.match(runtime, /fn weighted_person_similarity/, "同一人员多样本必须采用加权融合评分");
assert.match(runtime, /TOP_K_WEIGHTS: \[f32; 3\] = \[0\.60, 0\.28, 0\.12\]/, "多样本评分必须使用 Top-3 衰减权重");
assert.match(lib, /load_recognition_templates/, "识别帧必须加载本机启用人员特征模板");
assert.match(lib, /recognizer_ready/, "识别模型未就绪时不应产生识别告警");
assert.match(lib, /source_kind: "camera_face"/, "识别告警必须使用具名来源标识");

// 录入时提取特征并落库，特征不出本机。
assert.match(lib, /update_face_person_embedding/, "录入人员后必须把特征写入本机数据库");
assert.match(storage, /embedding_model_version/, "特征必须携带模型版本以便升级重提取");
assert.match(storage, /face_person_samples/, "同一人员的多张照片必须使用独立样本表保存");
assert.match(storage, /source_kind/, "识别告警记录必须保留来源类型列");

// 前端状态展示：识别模型与人员特征状态可见，缺失时给出提示。
assert.match(app, /recognizerReady/, "设置页必须展示识别模型状态");
assert.match(app, /hasEmbedding/, "设置页必须展示人员特征提取状态");
assert.match(app, /尚未录入可用的识别人员/, "无可用人员时必须提示不会产生告警");
assert.match(types, /sourceKind/, "告警类型必须包含来源类型字段");
assert.match(types, /localFeedback\?:\s*"real"\s*\|\s*"false"\s*\|\s*null/, "告警类型必须携带本机已反馈状态");
assert.match(storage, /list_camera_face_alerts_for_responder/, "告警历史必须按当前设备恢复已反馈状态");
assert.match(lib, /list_camera_face_alerts_for_responder\(100,\s*&profile\.device_id\)/, "告警列表命令必须使用本机设备标识查询反馈状态");
assert.match(app, /cameraFaceFeedbackedAlertIds\.value\s*=\s*new Set\([\s\S]*?localFeedback/, "重载告警历史时必须重建桌宠已反馈集合");

// 桌宠介入：识别告警独立驱动桌宠，并与狼来了严格区分。
assert.match(app, /facePetAlert\.value = \{/, "识别告警必须独立驱动桌宠覆盖状态");
assert.match(app, /temperature: face \? .*face\.confidence/, "识别告警桌宠温度必须使用人脸置信度");
assert.match(app, /latest_alert_kind: face \? "camera_face"/, "桌宠必须标记人脸识别告警类型");
assert.match(app, /feedbackCameraFaceAlert\(cameraAlert/, "桌宠人脸识别告警必须支持真实或虚假反馈");
assert.match(app, /facePetAlert\.value = next \? facePetAlertFromRecord\(next\) : null/, "处理第一条自动告警后必须切换到下一条待处理告警");
assert.ok(
  cameraFeedbackHandler.indexOf("await syncDesktopPetRuntime()") < cameraFeedbackHandler.indexOf("await api.sendCameraFaceAlertFeedback"),
  "自动告警反馈应先在本地切换下一条，再后台提交反馈",
);
assert.match(petRuntime, /"◉ 人脸"/, "桌宠详情需要显示人脸确认自动检测标识");
assert.match(petRuntime, /"◇ 人体"/, "桌宠详情需要显示人体特征自动检测标识");
assert.match(petRuntime, /"\[手动\]"/, "桌宠详情需要显示手动告警标识");
assert.match(app, /visuallyStoppedCameraFaceAlertIds/, "自己的人脸告警点击停止后需要停止桌宠视觉动画");
assert.match(app, /flashing: \(\!\!alert && !visuallyStoppedAlertIds\.value\.has\(alert\.alertId\)\)/, "停止快捷键只能关闭普通告警动画");
assert.match(app, /disco: \(\!\!activeFacePetAlert\.value && !visuallyStoppedCameraFaceAlertIds\.value\.has\(activeFacePetAlert\.value\.alertId\)\)/, "停止快捷键和超时只能关闭人脸告警蹦迪动画");
assert.match(app, /pending_count: pendingAlertCount\.value \+ pendingCameraFaceAlertCount\.value/, "停止快捷键不能清空未处理告警角标");
assert.match(app, /\.filter\(\(item\) => !cameraFaceFeedbackedAlertIds\.value\.has\(item\.alertId\)/, "本机人脸自动告警也必须进入待反馈状态");

// 外部推送：识别告警走独立推送路径，不复用狼来了模板。
assert.match(lib, /fn render_camera_face_push_text/, "识别告警必须有独立推送文案");
assert.match(lib, /fn send_camera_face_external_push/, "识别告警必须有独立外部推送路径");
assert.match(lib, /fn send_external_push_alert/, "狼来了外部推送入口签名保持不变");
assert.match(lib, /\[人脸确认告警\]/, "人脸确认外部推送必须使用独立标题");
assert.match(lib, /\[人体特征告警\]/, "人体特征外部推送必须使用独立标题");
assert.match(lib, /人脸确认检测到/, "人脸确认推送正文必须明确识别依据");
assert.match(lib, /人体特征疑似检测到/, "人体特征推送正文必须保留疑似语义");
assert.match(app, /latest_alert_recognition_level:/, "桌宠运行状态必须携带人物识别级别");
assert.match(app, /人脸确认检测到/, "桌宠人脸确认详情必须明确识别依据");
assert.match(app, /人体特征疑似检测到/, "桌宠人体特征详情必须保留疑似语义");
assert.match(petRuntime, /\("◉ 人脸",\s*Color32::from_rgb\(28, 170, 106\)\)/, "桌宠人脸确认应使用绿色短标签");
assert.match(petRuntime, /\("◇ 人体",\s*Color32::from_rgb\(224, 104, 36\)\)/, "桌宠人体特征应使用橙色短标签");
assert.match(app, /【\$\{face\.personName\}】 在【\$\{face\.sourceNickname\}】附近游荡/, "桌宠识别告警详情必须使用附近游荡格式");
assert.match(app, /automaticAlertRankingRows/, "人脸自动告警需要独立排行榜数据");
assert.match(app, /faceAverageConfidence/, "识别率排行榜需要显示人脸平均置信度");
assert.match(app, /bodyAverageConfidence/, "识别率排行榜需要显示人体平均置信度");
assert.match(app, /faceTruthRate/, "识别率排行榜需要计算人脸反馈真实度");
assert.match(app, /bodyTruthRate/, "识别率排行榜需要计算人体反馈真实度");
assert.match(app, /手动排行榜/, "告警排行榜需要提供手动页签");
assert.match(app, /识别率排行榜/, "自动告警页签应命名为识别率排行榜");
assert.match(api, /clearCameraFaceAlerts/, "一键清空排行榜必须清除自动识别告警记录");
assert.match(lib, /clear_camera_face_alerts/, "后端必须提供清空自动识别排行榜的命令");
assert.match(app, /multiple @change="handleLocalFacePhotoSelected"/, "本机人员录入必须支持一次选择多张参考照片");
assert.match(api, /photoPaths: string\[\]/, "多图录入和超管下发必须通过照片数组传递");
assert.match(types, /photoUrls\?: string\[\]/, "人员详情类型必须返回全部参考照片");
assert.match(app, /facePersonImageSources/, "人员详情必须将全部参考照片转换为可展示地址");
assert.match(app, /face-person-detail-thumbnails/, "人员详情下方必须提供照片缩略图列表");
assert.match(app, /facePersonDetailSelectedPhoto/, "人员详情必须支持选择缩略图并在上方放大");
assert.match(globalCss, /\.face-person-detail-modal\.n-card\s*\{[^}]*width:\s*min\(540px/, "人员详情卡片必须使用紧凑宽度");
assert.match(globalCss, /\.camera-face-alert-modal\.n-card\s*\{[^}]*width:\s*min\(560px/, "告警检测画面必须使用紧凑宽度");
assert.match(app, /max-width:\s*100%;\s*max-height:\s*100%;\s*object-fit:\s*contain/, "人员详情大图必须完整等比显示");
assert.match(runtime, /best_body_match/, "远距离人物识别必须接入人体 ReID 匹配");
assert.match(runtime, /recognition_level: "suspected"/, "人体外观命中必须标记为疑似识别");
assert.match(runtime, /TOP_K_WEIGHTS: \[f32; 3\] = \[0\.60, 0\.28, 0\.12\]/, "多样本必须使用 Top-3 加权");
assert.match(app, /疑似检测到/, "前端必须明确展示疑似识别，不能冒充确认身份");

console.log("face recognition alert guards passed");
