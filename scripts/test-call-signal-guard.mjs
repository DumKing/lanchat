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

console.log("call signal guard checks passed");
