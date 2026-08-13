import assert from "node:assert/strict";
import fs from "node:fs";

const runtimePath = "src-tauri/src/desktop_pet_runtime.rs";
const legacyRuntimePath = "src-tauri/src/native_frog_pet.rs";
const builtinRoot = "src-tauri/resources/desktop-pets/frog-buddy";

assert.ok(fs.existsSync(runtimePath), "原生桌宠运行时应使用角色无关的 desktop_pet_runtime.rs");
assert.ok(!fs.existsSync(legacyRuntimePath), "旧 native_frog_pet.rs 应被移除");

const runtime = fs.readFileSync(runtimePath, "utf8");
const lib = fs.readFileSync("src-tauri/src/lib.rs", "utf8");
const manifest = JSON.parse(fs.readFileSync(`${builtinRoot}/manifest.json`, "utf8"));

assert.equal(manifest.id, "frog-buddy", "内置青蛙资源包 ID 应为 frog-buddy");
for (const state of ["Idle", "Alert", "Move", "Interact", "Life"]) {
  assert.ok(fs.existsSync(`${builtinRoot}/${state}`), `内置桌宠缺少 ${state} 状态目录`);
}

assert.match(lib, /mod desktop_pet_runtime;/, "Tauri 应注册通用桌宠运行时模块");
assert.match(lib, /DesktopPetController::start/, "应用启动时应启动通用桌宠控制器");
assert.match(lib, /register_desktop_pet_stop_hotkey/, "停止告警快捷键命令应使用通用桌宠命名");
assert.match(lib, /desktop_pet_stop_hotkey_received/, "全局快捷键事件应使用通用桌宠命名");
assert.doesNotMatch(lib, /native_frog|NativeFrog|set_frog_pet|register_frog/, "Rust 入口不应保留旧青蛙兼容接口");

assert.match(runtime, /pub struct DesktopPetRuntimeState/, "运行时状态应与角色无关");
assert.match(runtime, /pub struct DesktopPetController/, "控制器应与角色无关");
assert.match(runtime, /eframe::run_native/, "桌宠仍应使用原生透明窗口");
assert.match(runtime, /draw_package_frame/, "角色图片必须从资源包实际帧绘制");
assert.match(runtime, /desktop_pet_action/, "桌宠交互应只发送通用事件");
assert.match(runtime, /raw_scroll_delta/, "桌宠应保留滚轮缩放");
assert.match(runtime, /ViewportCommand::StartDrag/, "桌宠应保留拖动");
assert.match(runtime, /clip_by_uniform_index/, "运行时应按等概率索引选择当前动作");
assert.match(runtime, /clip_cycle_seconds/, "运行时应按动作完整周期播放");
assert.match(runtime, /active_clip_duration/, "运行时应让随机动作在配置时长内停留");
assert.match(runtime, /sequence_target_count/, "运行时应按状态配置随机动作数量");
assert.match(runtime, /sequence_interval/, "运行时应支持动作间随机停顿");
assert.match(runtime, /disco_movement_mode/, "原生运行时应接收线性或跳跃蹦迪移动方式");
assert.match(runtime, /pet_press_dragged[\s\S]{0,300}ViewportCommand::StartDrag/, "拖动桌宠时应切换到 Move 动作");
assert.match(runtime, /emit_action\("open_main_window"/, "普通状态单击桌宠应请求打开主程序");
assert.match(runtime, /alert_active[\s\S]{0,500}emit_action\("stop_visuals"/, "告警状态单击桌宠应停止告警而不是打开主程序");
assert.doesNotMatch(runtime, /\.weight\b/, "运行时动作选择不应读取资源权重");
assert.doesNotMatch(runtime, /include_bytes!\([^)]*frog|FROG_SHEET|frog_pose_index|draw_frog_image/, "运行时不应再嵌入旧青蛙图集或固定姿态回退");
assert.doesNotMatch(runtime, /native_frog_action/, "运行时不应再发送旧青蛙兼容事件");

console.log("desktop pet runtime checks passed");
