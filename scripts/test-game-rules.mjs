import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "game-rules-"));
const outfile = path.join(tempDir, "rules.mjs");

try {
  await build({ entryPoints: [path.join(root, "src/games/rules.ts")], outfile, bundle: true, format: "esm", platform: "node", logLevel: "silent" });
  const { gameRuleBookOf } = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  for (const game of ["doudizhu", "gomoku", "xiangqi", "minesweeper", "monopoly"]) {
    assert.ok(gameRuleBookOf(game).tabs.length > 0, `${game} 应提供可阅读的玩法规则`);
  }
  const monopoly = gameRuleBookOf("monopoly");
  assert.deepEqual(monopoly.tabs.map((tab) => tab.key), ["rules", "tiles", "buildings", "cards", "events", "gods"], "大富翁规则弹窗应按玩法、地块、建筑、道具、事件和神明分栏");
  assert.match(monopoly.tabs.find((tab) => tab.key === "tiles")?.items.join(" ") ?? "", /神明|路障|查封|翻倍/, "地块说明应包含可见状态标记");
  assert.match(monopoly.tabs.find((tab) => tab.key === "buildings")?.items.join(" ") ?? "", /起点|飞机场|地价翻倍|监狱|摇奖机/, "建筑图鉴说明应覆盖所有特殊设施");
  assert.equal(monopoly.tabs.find((tab) => tab.key === "cards")?.items.length, 10, "大富翁应完整说明十张道具卡");
  assert.equal(monopoly.tabs.find((tab) => tab.key === "events")?.items.length, 10, "大富翁应完整说明十种随机事件");
  console.log("game rules catalog ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
