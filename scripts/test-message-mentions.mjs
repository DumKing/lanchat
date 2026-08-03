import assert from "node:assert/strict";
import fs from "node:fs";
import ts from "typescript";

const source = fs.readFileSync("src/utils/messageMentions.ts", "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
const { detectMentionKind, trayConversationTitle } = await import(moduleUrl);

assert.equal(detectMentionKind("王二 请处理", "王二"), null, "没有 @ 符号时不应判定为提及");
assert.equal(detectMentionKind("@王二 请处理", "王二"), "me", "应识别对当前昵称的提及");
assert.equal(detectMentionKind("请注意，@王二 请处理", "王二"), "me", "中文标点后紧邻的提及也应识别");
assert.equal(detectMentionKind("请 @所有人 注意", "王二"), "all", "应优先识别 @所有人");
assert.equal(detectMentionKind("@王二号 请处理", "王二"), null, "相似昵称不应误判");
assert.equal(trayConversationTitle("局域网频道", "group"), "局域网频道", "已有频道后缀不应重复追加");
assert.equal(trayConversationTitle("研发群", "group"), "研发群频道", "群聊托盘标题应标明频道");
assert.equal(trayConversationTitle("张三", "direct"), "张三", "私聊托盘标题应保持昵称");

console.log("message mention checks passed");
