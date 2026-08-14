import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, packageJson, cargoToml, lib, capability] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../package.json", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
  readFile(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8"),
]);

assert.match(packageJson, /"@tauri-apps\/plugin-autostart"/, "前端必须安装 Tauri 官方自启动插件");
assert.match(cargoToml, /tauri-plugin-autostart/, "Rust 必须安装 Tauri 官方自启动插件");
assert.match(lib, /tauri_plugin_autostart::init/, "应用启动时必须注册自启动插件");
assert.match(capability, /autostart:allow-enable/, "主窗口必须有开启自启动权限");
assert.match(capability, /autostart:allow-disable/, "主窗口必须有关闭自启动权限");
assert.match(capability, /autostart:allow-is-enabled/, "主窗口必须有读取自启动状态权限");
assert.match(app, /from "@tauri-apps\/plugin-autostart"/, "设置页必须调用官方自启动 API");
assert.match(app, /AUTOSTART_INITIALIZED_KEY/, "首次默认开启后必须记录初始化状态，避免覆盖用户主动关闭");
assert.match(app, /!initialized\s*&&\s*import\.meta\.env\.PROD/, "开发调试时不能把临时可执行文件注册到系统启动项");
assert.match(app, /async function initializeAutostart/, "应用启动时必须恢复并初始化自启动状态");
assert.match(app, /title="启动设置"/, "基础设置必须提供启动设置卡片");
assert.match(app, /开机自动启动 LanChat/, "设置页必须提供明确的开机自启开关");

console.log("autostart settings guards passed");
