import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const app = readFileSync(resolve(process.cwd(), "src/App.vue"), "utf8");
const store = readFileSync(resolve(process.cwd(), "src/stores/lanchat.ts"), "utf8");

if (!store.includes("appendOrUpdateMessage(message);")) {
  throw new Error("发送后必须立即写入当前会话消息缓存");
}
if (!app.includes("const messagePaneFollowingLatest = ref(true);")) {
  throw new Error("消息视图需要明确记录是否正在跟随最新消息");
}
if (!app.includes("if (messagePaneFollowingLatest.value)")) {
  throw new Error("新消息到达时，处于底部的会话必须自动推进到最新消息");
}
if (!app.includes("messagePaneFollowingLatest.value = isMessagePaneAtBottom();")) {
  throw new Error("滚动事件需要根据真实位置维护最新消息跟随状态");
}

console.log("realtime message refresh guards passed");
