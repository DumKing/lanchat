import { readFileSync } from "node:fs";

const source = readFileSync(new URL("../src/App.vue", import.meta.url), "utf8");

if (!source.includes("callPeerConnection.remoteDescription")) {
  throw new Error("ICE 候选必须在远端描述可用后才加入连接");
}
if (!source.includes("flushQueuedCallCandidates")) {
  throw new Error("远端描述就绪后必须补处理已缓存的 ICE 候选");
}
if (!source.includes("handleCallSignal(signal).catch")) {
  throw new Error("通话信令异常必须被局部处理，不能触发应用错误页");
}
if (!source.includes("你抖了抖 ${peerDisplayName(peer)}")) {
  throw new Error("发送抖一抖成功后应在发送方私聊中写入系统通知");
}
if (!source.includes("formatCallMediaPermissionError")) {
  throw new Error("语音视频权限拒绝应转换为可展示的提示");
}
if (!source.includes('connectionState === "failed"')) {
  throw new Error("通话连接失败应保留窗口并进入可恢复状态");
}
if (!source.includes("handleDesktopPetCallAction")) {
  throw new Error("桌宠接听和拒绝应通过带兜底的通话动作处理器执行");
}
if (!source.includes("remoteCallAudio") || !source.includes("ensureCallMediaPlaying")) {
  throw new Error("语音通话必须挂载远端音频流并主动恢复播放");
}

console.log("call signal guard checks passed");
