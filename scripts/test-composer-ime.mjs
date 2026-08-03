import assert from "node:assert/strict";
import fs from "node:fs";
import ts from "typescript";

const helperSource = fs.readFileSync("src/utils/composerKeyboard.ts", "utf8");
const compiled = ts.transpileModule(helperSource, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
const { shouldSendComposerMessage } = await import(moduleUrl);

assert.equal(shouldSendComposerMessage({ key: "Enter", shiftKey: false, isComposing: false, keyCode: 13 }), true);
assert.equal(shouldSendComposerMessage({ key: "Enter", shiftKey: true, isComposing: false, keyCode: 13 }), false);
assert.equal(shouldSendComposerMessage({ key: "Enter", shiftKey: false, isComposing: true, keyCode: 229 }), false);
assert.equal(shouldSendComposerMessage({ key: "Enter", shiftKey: false, isComposing: false, keyCode: 229 }), false);
assert.equal(shouldSendComposerMessage({ key: "A", shiftKey: false, isComposing: false, keyCode: 65 }), false);

const appSource = fs.readFileSync("src/App.vue", "utf8");
const componentSource = fs.readFileSync("src/components/ChatComposerInput.vue", "utf8");
assert.match(appSource, /<ChatComposerInput\b/, "聊天输入框应隔离为独立组件，避免页面计时刷新打断输入法合成态");
assert.match(componentSource, /<NInput\b/, "独立输入组件应保留 Naive UI 输入框");
assert.match(componentSource, /@compositionstart=/, "输入组件应显式跟踪输入法合成开始");
assert.match(componentSource, /@compositionend=/, "输入组件应显式跟踪输入法合成结束");

console.log("composer IME checks passed");
