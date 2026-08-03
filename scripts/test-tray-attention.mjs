import assert from "node:assert/strict";
import fs from "node:fs";

const lib = fs.readFileSync("src-tauri/src/lib.rs", "utf8");

assert.match(lib, /tray\s*\.set_icon\(None\)/, "闪烁隐藏帧应清空图像并保留托盘项");
assert.match(lib, /tray\.set_icon\(Some\(image\)\)/, "闪烁显示帧应恢复完整 LanChat 图标");
assert.doesNotMatch(lib, /set_tray_icon_visible|tray\.set_visible\(/, "闪烁不应移除托盘项，否则悬停区域会消失");
assert.match(lib, /format!\("\{\}：\{\}条未读消息"/, "托盘悬停应逐项展示未读消息数");

console.log("tray attention checks passed");
