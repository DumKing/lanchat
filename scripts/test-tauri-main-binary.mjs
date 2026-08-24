import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const cargoManifest = readFileSync(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");

assert.match(cargoManifest, /^autobins\s*=\s*false$/m, "Cargo 必须关闭自动二进制发现，避免把发布辅助工具当成桌面主程序。");
assert.match(
  cargoManifest,
  /\[\[bin\]\][\s\S]*?name\s*=\s*"lanchat"[\s\S]*?path\s*=\s*"src\/main\.rs"/m,
  "lanchat 主二进制必须显式指向 src/main.rs。",
);

console.log("Tauri 主二进制配置校验通过。");
