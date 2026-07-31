import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

function pad(value) {
  return String(value).padStart(2, "0");
}

const now = new Date();
const timestamp = [
  now.getFullYear(),
  pad(now.getMonth() + 1),
  pad(now.getDate()),
  pad(now.getHours()),
  pad(now.getMinutes()),
  pad(now.getSeconds()),
].join("");

const target = resolve("src-tauri/build-timestamp.txt");
mkdirSync(dirname(target), { recursive: true });
writeFileSync(target, `${timestamp}\n`, "utf8");
console.log(`LanChat build timestamp: ${timestamp}`);
