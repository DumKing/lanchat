import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const protocol = await readFile(new URL("../src-tauri/src/protocol.rs", import.meta.url), "utf8");
const runtime = await readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const network = await readFile(new URL("../src-tauri/src/network.rs", import.meta.url), "utf8");
const app = await readFile(new URL("../src/App.vue", import.meta.url), "utf8");

assert.match(
  protocol,
  /AdminRemoteUpdateProgress\(AdminRemoteUpdateProgressFrame\)/,
  "远程更新协议必须提供独立的下载进度帧",
);
assert.match(
  protocol,
  /pub delivery_id: String/,
  "远程更新命令必须携带批次 ID，便于归并逐台状态",
);
assert.match(
  protocol,
  /pub force: bool/,
  "远程更新命令必须明确携带强制更新策略",
);
assert.match(
  protocol,
  /fn default_remote_update_delivery_id\(command_id: &str\) -> String/,
  "旧命令缺少批次 ID 时必须回退到命令 ID",
);
assert.match(runtime, /all_online_windows: bool/, "后端必须接受全部在线 Windows 的目标范围");
assert.match(runtime, /peer\.platform_os == "windows"/, "批量下发必须只选择在线 Windows 设备");
assert.match(runtime, /force: bool/, "后端必须接收明确的强制更新策略");
assert.match(runtime, /should_execute_admin_remote_update/, "接收端必须按版本策略决定是否安装");
assert.match(runtime, /share_file\(path\)/, "局域网安装包必须只经本机文件服务共享");
assert.match(network, /send_admin_remote_update_progress/, "下载进度必须通过 TCP 回传给下发者");
assert.match(app, /all_online_windows/, "管理员界面必须提供全部在线 Windows 范围");
assert.match(app, /adminRemoteUpdateForce/, "管理员界面必须提供强制更新开关");
assert.match(app, /adminRemoteUpdateResults/, "管理员界面必须展示逐台状态");

console.log("admin remote update protocol guards passed");
