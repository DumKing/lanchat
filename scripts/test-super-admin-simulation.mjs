import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const protocol = readFileSync("src-tauri/src/protocol.rs", "utf8");
const network = readFileSync("src-tauri/src/network.rs", "utf8");
const backend = readFileSync("src-tauri/src/lib.rs", "utf8");
const storage = readFileSync("src-tauri/src/storage.rs", "utf8");
const api = readFileSync("src/services/tauri-api.ts", "utf8");
const types = readFileSync("src/types/lanchat.ts", "utf8");
const app = readFileSync("src/App.vue", "utf8");

assert.match(protocol, /pub struct SimulationMeta/, "协议需要声明模拟操作元数据");
assert.match(protocol, /pub simulation: Option<SimulationMeta>/, "聊天和告警帧需要可选模拟元数据");
assert.match(backend, /async fn simulate_message/, "后端需要提供模拟文本消息命令");
assert.match(backend, /async fn simulate_quick_alert/, "后端需要提供模拟告警命令");
assert.match(backend, /需要超级管理员权限/, "后端必须拒绝非超管调用");
assert.match(network, /authorization_device_id/, "频道权限必须以真实操作人校验");
assert.match(storage, /simulation_audits/, "模拟操作需要写入本机审计记录");
assert.match(types, /export type SimulationMeta/, "前端需要识别模拟操作元数据");
assert.match(api, /simulateMessage/, "前端 API 需要暴露模拟消息能力");
assert.match(api, /simulateQuickAlert/, "前端 API 需要暴露模拟告警能力");
assert.match(app, /超管模拟发送/, "设备详情需要提供模拟发送入口");
assert.match(app, /显示超管模拟发送/, "模拟发送应支持显示标签开关");
assert.match(app, /模拟私聊|模拟频道消息|模拟普通告警|模拟蹦迪告警/, "界面必须限定支持的模拟类型");

console.log("super admin simulation contracts passed");
