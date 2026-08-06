import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const read = (file) => readFileSync(resolve(process.cwd(), file), "utf8");
const protocol = read("src-tauri/src/protocol.rs");
const storage = read("src-tauri/src/storage.rs");
const network = read("src-tauri/src/network.rs");
const backend = read("src-tauri/src/lib.rs");
const app = read("src/App.vue");

for (const [name, source, needle] of [
  ["protocol", protocol, "AdminNotificationSubmission"],
  ["storage", storage, "admin_notifications"],
  ["network", network, "admin_notification_received"],
  ["backend", backend, "ensure_super_admin_session(&state)?;"],
  ["all online", backend, 'target_scope == "all_online"'],
  ["recipient lock", app, "blockingAdminNotification"],
  ["recipient submit", app, "提交已完成"],
  ["admin review", app, "超管通知审核"],
]) {
  if (!source.includes(needle)) throw new Error(`${name} missing: ${needle}`);
}

console.log("admin notification integration guards passed");
