import assert from "node:assert/strict";
import fs from "node:fs";

const app = fs.readFileSync("src/App.vue", "utf8");

assert.match(app, /nativeUpdateProgress/, "自动更新应维护下载进度状态");
assert.match(app, /download\s*\(\s*\(event\)/, "自动更新应使用带事件回调的 download() 接收进度");
assert.doesNotMatch(app, /downloadAndInstall\s*\(/, "自动更新不应使用无法展示进度的 downloadAndInstall() 快捷调用");
assert.match(app, /<NProgress[\s\S]*nativeUpdateProgressPercent/, "更新弹窗应展示下载进度条");
assert.match(app, /nativeUpdateProgressLabel/, "更新弹窗应展示下载进度文本");

console.log("update progress UI checks passed");
