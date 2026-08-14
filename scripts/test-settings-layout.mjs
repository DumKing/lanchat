import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile(new URL("../src/App.vue", import.meta.url), "utf8");
const css = await readFile(new URL("../src/styles/global.css", import.meta.url), "utf8");

assert.match(app, /'basic-settings-grid': settingsCategory === 'basic'/, "基础设置必须使用独立的单列布局");
assert.match(css, /\.basic-settings-grid\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/, "基础设置卡片必须从上到下铺满整行");

assert.match(app, /title="内存诊断"[^>]*class="basic-memory-diagnostic-card"/, "内存诊断必须标记为末尾工具卡片");
assert.match(app, /title="图片缓存"[^>]*class="basic-image-cache-card"/, "图片缓存必须标记为末尾工具卡片");
assert.match(app, /title="网络修复"[^>]*class="basic-network-repair-card"/, "网络修复必须标记为末尾工具卡片");
assert.match(css, /\.basic-memory-diagnostic-card\{order:90\}\.basic-image-cache-card\{order:91\}\.basic-network-repair-card\{order:92\}/, "基础设置末尾必须依次为内存诊断、图片缓存、网络修复");
assert.match(app, /title="版本更新"[\s\S]*GitHub API Token/, "版本更新和 GitHub Token 必须合并到同一卡片");
assert.doesNotMatch(app, /title="GitHub API Token"/, "GitHub Token 不应继续占用独立卡片");

assert.match(app, /t\("settings\.admin"\)/, "设置分类应通过国际化资源显示超管设置");
assert.match(app, /title="超管通知"[\s\S]*查看通知审核记录/, "通知下发和审核入口必须合并到同一卡片");
assert.match(app, /class="admin-notification-review-modal"/, "通知审核记录必须使用独立弹窗");
assert.match(app, /<NPagination[\s\S]*adminNotificationReviewPage/, "通知审核记录必须分页展示");
assert.match(css, /\.admin-notification-review-modal\.n-card\s*\{[^}]*width:\s*min\(/, "通知审核弹窗宽度必须保持紧凑");

for (const title of ["告警真实度", "报警模式下发", "狼来了推送阈值下发"]) {
  const pattern = new RegExp(`settingsCategory === 'admin'[^>]*title="${title}"`);
  assert.match(app, pattern, `${title}必须迁移到超管设置`);
}

console.log("settings layout guards passed");
