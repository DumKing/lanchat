import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [store, app, storage, network, fileServer, backend] = await Promise.all([
  readFile(new URL("../src/stores/lanchat.ts", import.meta.url), "utf8"),
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/storage.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/network.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/file_server.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
]);

assert.match(store, /MAX_MESSAGES_PER_CONVERSATION = 500/, "前端单会话消息缓存需要有上限");
assert.match(store, /MAX_CACHED_CONVERSATIONS = 12/, "前端会话缓存数量需要有上限");
assert.match(store, /MAX_ADMIN_NOTIFICATIONS = 120/, "前端公告缓存需要有上限");
assert.match(app, /VISIBLE_MESSAGE_WINDOW = 60/, "聊天视口消息节点需要有上限");
assert.match(app, /compressAvatarImage/, "头像保存前需要压缩");
assert.match(app, /loading="lazy"/, "聊天图片需要延迟加载");
assert.match(app, /revokeObjectURL/, "清理图片缓存时需要释放 Blob URL");
assert.match(app, /内存诊断/, "设置中需要提供内存诊断入口");
assert.match(store, /function stopRuntime\(\)/, "应用卸载时需要释放局域网事件订阅和轮询任务");
assert.match(store, /document\.visibilityState === "hidden"/, "窗口隐藏时不应继续轮询设备列表");
assert.match(store, /samePeerSnapshot/, "设备列表未变化时不应重复触发响应式渲染");
assert.match(app, /store\.stopRuntime\(\)/, "根组件卸载时需要停止局域网运行时资源");
assert.match(storage, /list_messages_page/, "数据库读取消息需要支持按页加载");
assert.match(network, /mpsc::channel::<WireFrame>\(128\)/, "TCP 发送队列需要有上限");
assert.doesNotMatch(network, /unbounded_channel::<WireFrame>/, "TCP 发送队列不能无上限");
assert.match(fileServer, /tokio::io::copy\(&mut file_handle, &mut stream\)/, "文件下载需要流式传输");
assert.doesNotMatch(fileServer, /tokio::fs::read\(&file\.path\)/, "文件下载不能整文件读入内存");
assert.match(backend, /PREVIEW_MEDIA_CACHE_TOTAL_LIMIT_BYTES/, "图片缓存总容量需要限制");
assert.match(backend, /enforce_preview_media_cache_limit/, "图片缓存需要按 LRU 清理");
assert.match(backend, /\.chunk\(\)[\s\S]{0,60}\.await/, "图片缓存下载应分块处理，不能一次性读入整张图片");
assert.doesNotMatch(backend, /response\s*\.bytes\(\)\s*\.await/, "图片缓存不能一次性读入整张图片");

console.log("memory bounds contracts passed");
