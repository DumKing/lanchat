import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const modulePath = resolve('src/utils/alertCredibility.ts');
assert.ok(existsSync(modulePath), '真实度算法应拆到 src/utils/alertCredibility.ts，避免散落在 App.vue');

const source = readFileSync(modulePath, 'utf8');
const appVue = readFileSync(resolve('src/App.vue'), 'utf8');

assert.match(source, /export function alertTruthScore/, '应导出单条告警真实度函数');
assert.match(source, /export function senderCredibility/, '应导出发送人真实度函数');
assert.match(source, /export function alertTemperature/, '应导出青蛙温度映射函数');
assert.match(source, /\(realCount \+ 1\) \/ \(feedbackCount \+ 2\)/, '单条告警应使用 Beta(1,1) 平滑公式');
assert.match(source, /Math\.min\(1,\s*feedbackCount \/ 5\)/, '单条告警权重应包含反馈人数置信度，上限为 5 人');
assert.match(source, /ageMs <= SEVEN_DAYS_MS[\s\S]*?return 1/, '7 天内告警时间权重应为 1');
assert.match(source, /ageMs <= THIRTY_DAYS_MS[\s\S]*?return 0\.7/, '7-30 天告警时间权重应为 0.7');
assert.match(source, /return 0\.4/, '30 天以上告警时间权重应为 0.4');
assert.match(source, /feedbackCount === 0[\s\S]*?weight: 0/, '无反馈告警不应参与个人真实度计算');
assert.match(source, /return Math\.round\(weightedScore \/ totalWeight\)/, '个人真实度应为加权平均后的百分比');
assert.match(source, /credibility === null[\s\S]*?return 100/, '无历史有效反馈时青蛙温度应默认 100°C');
assert.match(source, /credibility >= 80[\s\S]*?return Math\.round\(90 \+/, '高真实度应映射到 90-100°C');
assert.match(source, /credibility >= 60[\s\S]*?return Math\.round\(70 \+/, '60-79% 应映射到 70-89°C');
assert.match(source, /credibility >= 40[\s\S]*?return Math\.round\(45 \+/, '40-59% 应映射到 45-69°C');
assert.match(source, /return Math\.round\(20 \+/, '40% 以下应映射到 20-44°C');

assert.match(appVue, /from "\.\/utils\/alertCredibility"/, 'App.vue 应使用统一真实度算法模块');
assert.doesNotMatch(appVue, /function alertTruthProbability/, 'App.vue 不应再保留旧的简单比例真实度函数');
assert.doesNotMatch(appVue, /function senderTruthProbability/, 'App.vue 不应再保留旧的简单平均个人真实度函数');
assert.match(appVue, /senderCredibility\(alertRecords\.value,\s*alert\.senderDeviceId/, '桌宠温度应按发送人历史真实度计算');
assert.match(appVue, /alertTruthScore\(alert,\s*nowTick\.value\)/, '最近告警标签应使用新单条真实度算法');
assert.match(appVue, /senderCredibility\(alertRecords\.value,\s*row\.deviceId/, '排行榜应使用新个人真实度算法');
assert.match(appVue, /QUICK_ALERT_TRUST_RESET_ALL_TARGET = "__all__"/, '超管应使用特殊目标广播清空全部狼来了记录');
assert.match(appVue, /alertRecords\.value = \[\]/, '收到全量清空指令后应清空本地狼来了记录');
assert.match(appVue, /resetAllAlertCredibilityRecords/, '设置页应提供超管一键清空全部记录入口');

console.log('alert credibility checks passed');
