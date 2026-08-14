import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [coordinator, app, types] = await Promise.all([
  readFile(new URL("../src/services/cameraMediaCoordinator.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/types/face-monitor.ts", import.meta.url), "utf8"),
]);

assert.match(types, /CameraMonitorSettings/, "需要定义本机摄像头识别配置");
assert.match(types, /CameraMonitorStatus/, "需要定义摄像头诊断状态");
assert.match(types, /CameraFrameSample/, "需要定义低频帧契约");
assert.match(coordinator, /acquireForCall/, "协调器需要提供通话媒体流");
assert.match(coordinator, /sourceVideoTrack\.clone\(\)/, "视频通话必须克隆本地检测轨道，关闭发送不能停掉检测源");
assert.match(coordinator, /callVideoTrack/, "协调器需要独立管理通话发送轨道的生命周期");
assert.match(coordinator, /subscribeFrames/, "协调器需要提供识别采样订阅");
assert.match(coordinator, /effectiveSampleFps\(\).*videoCallActive/s, "通话期间需要降频采样");
assert.match(coordinator, /longest = 320/, "采样帧必须缩放，不能传递原始视频帧");
assert.match(coordinator, /samplingBusy/, "推理侧繁忙时需要丢弃新帧");
assert.match(coordinator, /samplingAllowed/, "模型未就绪时必须停止采样，不能持续编码空帧");
assert.match(app, /setSamplingAllowed\(Boolean\(faceMonitorRuntimeStatus\.value\?\.modelReady\)\)/, "前端必须根据原生模型状态启停采样");
assert.match(app, /cameraMediaCoordinator\.acquireForCall/, "通话必须通过统一协调器申请媒体流");
assert.doesNotMatch(app, /prepareLocalCallMedia[\s\S]{0,350}navigator\.mediaDevices\.getUserMedia/, "通话代码不能绕过协调器直接申请摄像头");

console.log("face monitor media coordinator guards passed");
