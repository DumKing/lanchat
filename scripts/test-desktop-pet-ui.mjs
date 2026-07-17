import assert from "node:assert/strict";
import fs from "node:fs";

const appVue = fs.readFileSync("src/App.vue", "utf8");
const api = fs.readFileSync("src/services/tauri-api.ts", "utf8");
const types = fs.readFileSync("src/types/lanchat.ts", "utf8");
const store = fs.readFileSync("src/stores/lanchat.ts", "utf8");

assert.match(types, /export type PetAlertMode = "normal" \| "disco"/, "报警模式类型应使用通用桌宠命名");
assert.match(types, /export type DesktopPetRuntimeState/, "前端应定义通用桌宠运行时状态");
assert.doesNotMatch(types, /FrogAlertMode|NativeFrogPetState/, "前端类型不应保留旧青蛙命名");

assert.match(api, /updateDesktopPetState: \(petState: DesktopPetRuntimeState\)/, "API 应同步通用桌宠状态");
assert.match(api, /registerDesktopPetStopHotkey/, "API 应注册通用桌宠停止快捷键");
assert.doesNotMatch(api, /setFrogPetEnabled|updateNativeFrogPet|registerFrogStopHotkey|update_native_frog_pet|register_frog_stop_hotkey/, "API 不应保留旧青蛙兼容调用");

assert.match(store, /mode: PetAlertMode = "normal"/, "告警 store 应使用通用报警模式类型");
assert.match(appVue, /listen<\{ action: string; alert_id\?: string \| null \}>\("desktop_pet_action"/, "主界面应只监听通用桌宠动作事件");
assert.match(appVue, /event\.payload\.action === "open_main_window"[\s\S]{0,160}api\.showFromTray/, "桌宠普通单击应打开主程序");
assert.match(appVue, /listen\("desktop_pet_stop_hotkey_received"/, "主界面应监听通用快捷键事件");
assert.match(appVue, /const ALERT_SEND_COOLDOWN_MS = 20_000;/, "告警仍应保留本地频率限制");
assert.match(appVue, /api\.updateDesktopPetState\(runtimeState\)/, "主界面应同步通用桌宠状态");
assert.match(appVue, /pet\.icon_path[\s\S]{0,180}pet\.preview_path[\s\S]{0,260}PetStateKind\.Idle|pet\.states\.Idle/, "桌宠列表应按 icon、preview、Idle 首帧回退");
assert.match(appVue, /desktopPetPackagesExpanded/, "桌宠资源列表应支持折叠展开");
assert.match(appVue, /@contextmenu\.prevent="openDesktopPetManifestEditor\(pet\)"/, "桌宠条目右键应打开 manifest 动作配置编辑器");
assert.match(appVue, /discoMovementMode/, "桌宠设置应提供蹦迪移动方式");
assert.match(appVue, /线性移动/, "蹦迪移动方式应包含线性移动");
assert.match(appVue, /跳跃移动/, "蹦迪移动方式应包含跳跃移动");
assert.match(api, /updateDesktopPetPlaybackConfig/, "API 应支持保存桌宠 manifest 播放配置");
assert.doesNotMatch(appVue, /isPetWindow|frog-pet-window|frog-alert-body|NativeFrog|syncNativeFrog|syncFrogPetWindowSize/, "主界面不应保留旧 WebView 青蛙桌宠分支");

console.log("desktop pet ui checks passed");
