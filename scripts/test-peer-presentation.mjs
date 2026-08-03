import assert from "node:assert/strict";
import fs from "node:fs";
import ts from "typescript";

const source = fs.readFileSync("src/utils/peerPresentation.ts", "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
const { peerDisplayName, sameDeviceId, sortPeersForDisplay } = await import(moduleUrl);

assert.equal(peerDisplayName({ nickname: "DESKTOP-A", note: "研发服务器" }), "研发服务器");
assert.equal(peerDisplayName({ nickname: "DESKTOP-A", note: "  " }), "DESKTOP-A");
assert.equal(sameDeviceId("AA-BB-CC-DD-EE-FF", "aa:bb:cc:dd:ee:ff"), true);
assert.equal(sameDeviceId("aabbccddeeff", "aa:bb:cc:dd:ee:ff"), true);

const peers = [
  { device_id: "c", nickname: "张三", note: null, online: true, last_seen_at: 300 },
  { device_id: "a", nickname: "李四", note: "阿尔法", online: false, last_seen_at: 100 },
  { device_id: "b", nickname: "王五", note: null, online: false, last_seen_at: 900 },
];
assert.deepEqual(
  sortPeersForDisplay(peers).map((peer) => peer.device_id),
  ["c", "a", "b"],
  "在线设备应优先展示，离线设备内部不受心跳时间影响",
);

console.log("peer presentation checks passed");
