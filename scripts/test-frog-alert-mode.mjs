import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appVue = readFileSync("src/App.vue", "utf8");
const store = readFileSync("src/stores/lanchat.ts", "utf8");
const api = readFileSync("src/services/tauri-api.ts", "utf8");
const types = readFileSync("src/types/lanchat.ts", "utf8");
const protocol = readFileSync("src-tauri/src/protocol.rs", "utf8");
const native = readFileSync("src-tauri/src/native_frog_pet.rs", "utf8");
const lib = readFileSync("src-tauri/src/lib.rs", "utf8");

assert.match(types, /export type FrogAlertMode = "normal" \| "disco"/, "前端应定义普通/蹦迪两种报警模式");
assert.match(types, /QuickAlert[\s\S]{0,220}mode: FrogAlertMode/, "告警消息应携带报警模式");
assert.match(protocol, /pub mode: String/, "Rust 告警帧应携带报警模式");
assert.match(protocol, /mode: "disco"\.to_string\(\)/, "协议 round-trip 应覆盖蹦迪告警模式");
assert.match(api, /sendQuickAlert: \(content: string,\s*mode: FrogAlertMode/, "Tauri API 应允许发送告警模式");
assert.match(store, /sendQuickAlert\(content = "呱呱~呱~~",\s*mode: FrogAlertMode = "normal"\)/, "store 发送告警应接收报警模式");
assert.match(appVue, /const frogAlertMode = ref<FrogAlertMode>\(readSavedFrogAlertMode\(\)\)/, "设置页应保存本机报警模式");
assert.match(appVue, /readSavedFrogAlertMode/, "应读取本机报警模式");
assert.match(appVue, /saveFrogAlertMode/, "应持久化本机报警模式");
assert.match(appVue, /NRadioGroup v-model:value="frogAlertMode"/, "设置页应提供本机普通/蹦迪报警开关");
assert.match(appVue, /sendFrogQuickAlert\("disco"\)/, "Ctrl 双击应强制发送全员蹦迪报警");
assert.match(native, /modifiers\.ctrl[\s\S]{0,220}broadcast_disco_alert/, "原生青蛙 Ctrl+双击应发全员蹦迪报警动作");
assert.match(appVue, /broadcast_disco_alert[\s\S]{0,180}sendFrogQuickAlert\("disco"\)/, "主应用应处理全员蹦迪报警动作");
assert.match(appVue, /normalizeFrogAlertMode\(alert\.mode\) === "disco"[\s\S]{0,220}discoModeUntil/, "收到蹦迪告警时应触发蹦迪视觉");
assert.match(appVue, /sendFrogQuickAlert\(frogAlertMode\.value\)/, "普通双击应按本机报警模式发送");
assert.match(appVue, /adminAlertModeTargetId/, "超管应选择目标设备下发报警模式");
assert.match(appVue, /adminAlertModeDraft/, "超管应选择下发的报警模式");
assert.match(appVue, /sendAdminAlertModeToPeer/, "设置页应提供超管下发报警模式方法");
assert.doesNotMatch(appVue, /title="蹦迪模式"/, "设置页不应再把蹦迪模式下发拆成单独卡片");
assert.doesNotMatch(appVue, /sendDiscoModeToPeer|discoModeTargetId|discoModeDurationMinutes/, "超管下发应合并到报警模式下发入口");
assert.match(appVue, /title="报警模式下发"[\s\S]{0,700}NRadioButton value="disco"/, "报警模式下发入口应同时支持普通和蹦迪");
assert.match(lib, /send_admin_alert_mode/, "Rust 应暴露超管下发报警模式命令");

console.log("frog alert mode checks passed");
