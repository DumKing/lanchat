import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appVue = readFileSync("src/App.vue", "utf8");

assert.match(appVue, /async function scrollActiveChatToBottom\(\)/, "聊天滚到底应有统一函数");
assert.match(appVue, /requestAnimationFrame[\s\S]{0,180}messagePane\.value\.scrollTop = messagePane\.value\.scrollHeight/, "滚动应等到下一帧，确保消息容器高度已稳定");
assert.match(appVue, /watch\(activeMessages,[\s\S]{0,220}if \(!hasLaterMessages\.value\)[\s\S]{0,120}scrollActiveChatToBottom/, "仅在用户位于最新消息时自动滚动，阅读历史时不应被强制跳回底部");
assert.match(appVue, /watch\(\(\) => activeConversationId\.value,[\s\S]{0,220}scrollActiveChatToBottom/, "切换聊天会话后应滚动到最新消息");
assert.match(appVue, /watch\(activeSection,[\s\S]{0,160}section === "chat"[\s\S]{0,120}scrollActiveChatToBottom/, "从其他界面切回聊天时应滚动到最新消息");
assert.match(appVue, /if \(section === "chat"\) \{[\s\S]{0,180}scrollActiveChatToBottom/, "侧边栏进入聊天时应主动滚动到底");

console.log("chat scroll checks passed");
