import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [backend, api, store, app] = await Promise.all([
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/stores/lanchat.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
]);

assert.match(backend, /Get-Process -Id \$appProcessId/, "绿色版更新应等待旧进程实际退出");
assert.match(backend, /is_super_admin_authenticated/, "后端应提供超管会话查询");
assert.match(api, /isSuperAdminAuthenticated/, "前端 API 应暴露超管会话查询");
assert.match(app, /restoreSavedSuperAdminSession\(\)/, "页面初始化应恢复已验证的超管会话");
assert.match(store, /peerRefreshRevision/, "设备刷新应防止旧请求覆盖新状态");
assert.match(store, /conversationRefreshRevision/, "会话刷新应防止旧请求覆盖新状态");
assert.match(store, /privateChannelIds/, "刷新会话时应同步私有频道成员缓存");

console.log("state recovery contracts passed");
