import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile("src/App.vue", "utf8");
const backend = await readFile("src-tauri/src/lib.rs", "utf8");
const workflow = await readFile(".github/workflows/release.yml", "utf8");

assert.match(app, /const UPDATE_CHECK_INTERVAL_MS = 12 \* 60 \* 60 \* 1000;/, "运行中的客户端应至少每 12 小时检查一次更新");
assert.match(app, /function scheduleAutomaticUpdateChecks\(\)[\s\S]*void checkUpdates\(false\)/, "启动后应立即自动检查更新");
assert.match(app, /window\.setInterval\([\s\S]*UPDATE_CHECK_INTERVAL_MS/, "客户端运行期间应持续安排更新检查");
assert.match(app, /const forceUpdateRequired = computed\(\(\) => updateInfo\.value\?\.forceRequired === true\)/, "前端应显式识别强制更新状态");
assert.match(app, /:mask-closable="!forceUpdateRequired"/, "强制更新窗口不能通过遮罩关闭");
assert.match(app, /v-if="!forceUpdateRequired"[\s\S]{0,120}稍后提醒/, "强制更新时不能显示稍后提醒按钮");
assert.match(backend, /let force_required = force && update_available;/, "只要发布标记强更且有新版本，就必须更新");
assert.match(workflow, /force_update/, "发布工作流应支持声明本次版本为强制更新");

console.log("force update checks passed");
