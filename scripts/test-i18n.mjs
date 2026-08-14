import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile(new URL("../src/App.vue", import.meta.url), "utf8");
const i18n = await readFile(new URL("../src/i18n.ts", import.meta.url), "utf8");

assert.match(i18n, /export type LanguagePreference = "system" \| "zh-CN" \| "en-US"/, "语言偏好必须支持跟随系统、中文和英文");
assert.match(i18n, /navigator\.language/, "跟随系统必须读取操作系统语言");
assert.match(i18n, /localStorage/, "手动语言选择必须持久化");
assert.match(i18n, /export function t\(/, "必须提供统一翻译函数");
assert.match(app, /naiveLocale/, "Naive UI 控件必须跟随当前语言");
assert.match(app, /dateLocale/, "日期控件必须跟随当前语言");
assert.match(app, /\{ label: t\("language\.system"\), key: "system" \}/, "语言选择必须提供跟随系统");
assert.match(app, /t\("nav\.chat"\)/, "主导航必须接入国际化");
assert.match(app, /t\("settings\.basic"\)/, "设置分类必须接入国际化");
assert.match(app, /watch\(effectiveLocale/, "切换语言后必须同步更新文档语言");

console.log("i18n guards passed");
