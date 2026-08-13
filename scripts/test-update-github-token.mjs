import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const read = (file) => readFileSync(resolve(process.cwd(), file), "utf8");
const backend = read("src-tauri/src/lib.rs");
const api = read("src/services/tauri-api.ts");
const app = read("src/App.vue");

for (const [name, source, needle] of [
  ["credential storage", backend, 'UPDATE_GITHUB_TOKEN_SERVICE'],
  ["token read", backend, "read_update_github_token"],
  ["authorized update client", backend, ".bearer_auth(token)"],
  ["token info command", backend, "get_update_github_token_info"],
  ["token save command", backend, "save_update_github_token"],
  ["token clear command", backend, "clear_update_github_token"],
  ["frontend api", api, "getUpdateGithubTokenInfo"],
  ["frontend save", api, "saveUpdateGithubToken"],
  ["settings input", app, "GitHub API Token"],
  ["settings status", app, "GitHub Token 已配置"],
]) {
  if (!source.includes(needle)) throw new Error(`${name} missing: ${needle}`);
}

console.log("github update token integration guards passed");
