import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const config = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
const scope = config.app.security.assetProtocol.scope;
const source = await readFile("src/App.vue", "utf8");
const backend = await readFile("src-tauri/src/lib.rs", "utf8");

assert.ok(scope.includes("$APPLOCALDATA/**"), "图片缓存位于应用本地数据目录，必须允许 asset protocol 读取");
assert.match(backend, /app_local_data_dir\(\)/, "图片缓存应写入应用本地数据目录");
assert.match(source, /convertFileSrc\(cachedPath\)/, "缓存图片应通过 Tauri asset protocol 读取");

console.log("preview media cache checks passed");
