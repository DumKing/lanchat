import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [app, styles] = await Promise.all([
  readFile(new URL("../src/App.vue", import.meta.url), "utf8"),
  readFile(new URL("../src/styles/global.css", import.meta.url), "utf8"),
]);

assert.match(app, /const operationErrorMessage = computed/, "所有可见的操作失败信息应汇总到统一弹窗");
assert.match(app, /class="operation-error-modal"/, "操作失败应使用独立紧凑弹窗");
assert.doesNotMatch(app, /<NAlert v-if="error"[^>]*title="操作失败"/, "操作失败不应继续铺在设置页面中");
assert.match(styles, /\.operation-error-modal\.n-card\s*\{[^}]*width:\s*min\(380px,/s, "失败弹窗宽度应保持紧凑");
assert.match(styles, /\.super-admin-auth-modal\.n-card\s*\{[^}]*width:\s*min\(360px,/s, "超管密码弹窗应缩窄");

console.log("operation feedback ui guards passed");
