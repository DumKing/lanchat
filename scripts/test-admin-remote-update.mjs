import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile(new URL("../src/App.vue", import.meta.url), "utf8");
const api = await readFile(new URL("../src/services/tauri-api.ts", import.meta.url), "utf8");
const types = await readFile(new URL("../src/types/lanchat.ts", import.meta.url), "utf8");
const protocol = await readFile(new URL("../src-tauri/src/protocol.rs", import.meta.url), "utf8");
const network = await readFile(new URL("../src-tauri/src/network.rs", import.meta.url), "utf8");
const backend = await readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

assert.match(app, /title="指定设备强制更新"/, "超管设置需要指定设备强制更新入口");
assert.match(app, /adminRemoteUpdateTargetId/, "强制更新必须选择指定设备");
assert.match(app, /adminRemoteUpdateVersion/, "强制更新必须填写目标版本");
assert.match(app, /adminRemoteUpdatePackagePath/, "强制更新必须支持可选本地安装包");
assert.match(api, /sendAdminRemoteUpdate/, "前端 API 必须支持下发远程更新");
assert.match(api, /executeAdminRemoteUpdate/, "目标设备必须执行收到的远程更新");
assert.match(types, /AdminRemoteUpdate/, "前端必须声明远程更新协议类型");
assert.match(protocol, /AdminRemoteUpdate\(AdminRemoteUpdateFrame\)/, "线协议必须包含远程更新帧");
assert.match(protocol, /package_sha256/, "携带安装包必须包含 SHA-256 校验值");
assert.match(network, /send_admin_remote_update/, "网络层必须向指定设备发送更新命令");
assert.match(backend, /download_remote_update_package/, "目标端必须通过后端分块下载安装包");
assert.match(backend, /execute_admin_remote_update/, "目标端必须提供自动安装入口");

console.log("admin remote update guards passed");
