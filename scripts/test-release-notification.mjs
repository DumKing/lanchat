import assert from "node:assert/strict";
import fs from "node:fs";

const workflow = fs.readFileSync(".github/workflows/release.yml", "utf8");

assert.match(workflow, /WECHAT_RELEASE_NOTIFY_ENABLED/, "发布通知应支持仓库变量开关");
assert.match(workflow, /secrets\.WECHAT_RELEASE_BOT_WEBHOOK/, "机器人 Webhook 应从 GitHub Secret 读取");
assert.match(workflow, /"msgtype":\s*"markdown"|--arg\s+content[\s\S]*msgtype:\s*"markdown"/, "企业微信通知必须使用 Markdown 报文");
assert.match(workflow, /Create release[\s\S]*Notify WeCom group/, "通知步骤必须位于 Release 创建之后");
assert.equal(workflow.match(/- name: Notify WeCom group/g)?.length, 1, "每次发布只能发送一次企业微信通知");

console.log("release notification checks passed");
