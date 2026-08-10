import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [store, storage, network, fileServer] = await Promise.all([
  readFile(new URL("../src/stores/lanchat.ts", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/storage.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/network.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/file_server.rs", import.meta.url), "utf8"),
]);

assert.match(store, /MAX_MESSAGES_PER_CONVERSATION = 500/, "前端单会话消息缓存需要有上限");
assert.match(store, /MAX_CACHED_CONVERSATIONS = 12/, "前端会话缓存数量需要有上限");
assert.match(storage, /MESSAGE_PAGE_LIMIT: i64 = 500/, "数据库读取消息需要分页上限");
assert.match(network, /mpsc::channel::<WireFrame>\(128\)/, "TCP 发送队列需要有上限");
assert.doesNotMatch(network, /unbounded_channel::<WireFrame>/, "TCP 发送队列不能无上限");
assert.match(fileServer, /tokio::io::copy\(&mut file_handle, &mut stream\)/, "文件下载需要流式传输");
assert.doesNotMatch(fileServer, /tokio::fs::read\(&file\.path\)/, "文件下载不能整文件读入内存");

console.log("memory bounds contracts passed");
