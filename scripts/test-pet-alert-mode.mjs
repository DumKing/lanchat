import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const appVue = readFileSync("src/App.vue", "utf8");
const store = readFileSync("src/stores/lanchat.ts", "utf8");
const api = readFileSync("src/services/tauri-api.ts", "utf8");
const types = readFileSync("src/types/lanchat.ts", "utf8");
const protocol = readFileSync("src-tauri/src/protocol.rs", "utf8");
const runtime = readFileSync("src-tauri/src/desktop_pet_runtime.rs", "utf8");
const lib = readFileSync("src-tauri/src/lib.rs", "utf8");

assert.match(types, /export type PetAlertMode = "normal" \| "disco"/, "前端应定义通用桌宠报警模式");
assert.match(types, /QuickAlert[\s\S]{0,220}mode: PetAlertMode/, "告警消息应携带报警模式");
assert.match(protocol, /pub mode: String/, "Rust 告警帧应携带报警模式");
assert.match(api, /sendQuickAlert: \(content: string, mode: PetAlertMode/, "Tauri API 应允许发送报警模式");
assert.match(store, /sendQuickAlert\(content = "呱呱~呱~~", mode: PetAlertMode = "normal"\)/, "store 应接收报警模式");
assert.match(appVue, /const petAlertMode = ref<PetAlertMode>\(readSavedPetAlertMode\(\)\)/, "设置页应读取本机报警模式");
assert.match(appVue, /NRadioGroup v-model:value="petAlertMode"/, "设置页应提供本机报警模式选择");
assert.match(appVue, /broadcast_disco_alert[\s\S]{0,120}sendPetQuickAlert\("disco"\)/, "Ctrl 双击应发送蹦迪报警");
assert.match(appVue, /normalizePetAlertMode\(alert\.mode\) === "disco"[\s\S]{0,160}discoModeUntil/, "收到蹦迪报警应更新运行时状态");
assert.match(runtime, /fn ctrl_pressed[\s\S]{0,180}modifiers\.ctrl/, "桌宠运行时应读取 Ctrl 状态");
assert.match(runtime, /ctrl_pressed\(ctx\)[\s\S]{0,220}broadcast_disco_alert/, "桌宠运行时应识别 Ctrl 双击");
assert.match(lib, /send_admin_alert_mode/, "Rust 应暴露超管报警模式命令");
assert.doesNotMatch(appVue, /FrogAlertMode|sendFrogQuickAlert|normalizeFrogAlertMode/, "前端不应保留旧青蛙报警接口");

console.log("desktop pet alert mode checks passed");
