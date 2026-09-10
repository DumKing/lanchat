import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { transform } from "esbuild";

const source = await readFile(new URL("../src/plugin-host/manifest/validateManifest.ts", import.meta.url), "utf8");
const compiled = await transform(source, { loader: "ts", format: "esm", target: "es2020" });
const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled.code).toString("base64")}`;
const { validatePluginManifest } = await import(moduleUrl);

const validManifest = {
  manifestVersion: 1,
  id: "com.lanchat.gomoku",
  name: "五子棋",
  version: "0.8.0",
  apiVersion: "1.0",
  minHostVersion: "0.8.0",
  type: "web",
  entry: "dist/index.html",
  icon: "icon.png",
  singleton: true,
  capabilities: ["rooms.read", "rooms.write", "storage.private", "theme.read"],
  contributes: {
    navigation: [{ id: "gomoku", title: "五子棋", icon: "icon.png", order: 210 }],
    games: [{ id: "gomoku", minPlayers: 2, maxPlayers: 2, ranking: { type: "win-loss", minHumanPlayers: 2 } }],
  },
};

assert.deepEqual(validatePluginManifest(validManifest), { ok: true, manifest: validManifest });

function expectError(patch, field, message) {
  const result = validatePluginManifest({ ...validManifest, ...patch });
  assert.equal(result.ok, false, message);
  assert.ok(result.errors.some((error) => error.field === field), `${message}：应定位 ${field}`);
}

expectError({ id: "Gomoku" }, "id", "插件 ID 必须使用反向域名格式");
expectError({ entry: "C:/plugins/index.html" }, "entry", "入口不能使用绝对路径");
expectError({ entry: "dist/../secret.html" }, "entry", "入口不能包含目录穿越");
expectError({ icon: "../icon.png" }, "icon", "图标不能越出插件目录");
expectError({ apiVersion: "2.0" }, "apiVersion", "宿主应拒绝不支持的 API 主版本");
expectError({ capabilities: ["rooms.read", "system.shell"] }, "capabilities[1]", "未知能力必须被拒绝");
expectError({ contributes: { navigation: [{ id: "same", title: "A" }, { id: "same", title: "B" }] } }, "contributes.navigation[1].id", "贡献 ID 不能重复");
expectError({ contributes: { games: [{ id: "gomoku", minPlayers: 3, maxPlayers: 2 }] } }, "contributes.games[0].maxPlayers", "最大人数不能小于最小人数");
expectError({ contributes: { games: [{ id: "gomoku", minPlayers: 2, maxPlayers: 2, ranking: { type: "win-loss", minHumanPlayers: 3 } }] } }, "contributes.games[0].ranking.minHumanPlayers", "计榜真人阈值不能超过房间人数");

const sanitized = validatePluginManifest(JSON.parse(JSON.stringify(validManifest)));
assert.equal(sanitized.ok && sanitized.manifest.id, "com.lanchat.gomoku", "JSON 清单应返回可使用的类型化结果");

const rootPackage = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
assert.deepEqual(rootPackage.workspaces, ["packages/*", "plugins/*"], "根项目应声明 SDK 与官方插件 workspace");

const sdkPackageUrl = new URL("../packages/plugin-sdk/package.json", import.meta.url);
await access(sdkPackageUrl);
const sdkPackage = JSON.parse(await readFile(sdkPackageUrl, "utf8"));
assert.equal(sdkPackage.name, "@lanchat/plugin-sdk", "标准 API 必须由独立 SDK 包提供");
assert.equal(sdkPackage.exports?.["."]?.types, "./dist/index.d.ts", "SDK 应发布类型声明入口");

const sdkSource = await readFile(new URL("../packages/plugin-sdk/src/index.ts", import.meta.url), "utf8");
assert.doesNotMatch(sdkSource, /(?:\.\.\/){2,}src\//, "SDK 不得反向引用宿主 src 目录");
assert.match(sdkSource, /createLanChatPluginClient/, "SDK 应提供标准客户端工厂");

console.log("plugin contracts ok");
