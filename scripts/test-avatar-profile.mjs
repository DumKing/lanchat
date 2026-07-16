import assert from "node:assert/strict";
import fs from "node:fs";

const app = fs.readFileSync("src/App.vue", "utf8");
const lib = fs.readFileSync("src-tauri/src/lib.rs", "utf8");
const network = fs.readFileSync("src-tauri/src/network.rs", "utf8");
const protocol = fs.readFileSync("src-tauri/src/protocol.rs", "utf8");
const storage = fs.readFileSync("src-tauri/src/storage.rs", "utf8");

assert.match(protocol, /pub struct HelloFrame[\s\S]{0,180}pub avatar: Option<String>/, "握手协议应携带头像");
assert.match(network, /HelloFrame \{[\s\S]{0,180}avatar: profile\.avatar\.clone\(\)/, "TCP 握手应发送本机头像");
assert.match(network, /remote_hello\.avatar\.clone\(\)/, "收到握手后应缓存对方头像");
assert.match(network, /fn status_frame_without_avatar/, "周期在线广播应支持不携带头像");
assert.match(network, /STATUS_BROADCAST_SECONDS[\s\S]{0,900}status_frame_without_avatar\(&profile\)/, "周期广播不应反复发送头像");
assert.match(lib, /500 \* 1024/, "后端保存头像应兜底限制 500KB");
assert.match(lib, /broadcast_profile_status\(app, &profile\)/, "头像或资料变更后应立即推送在线状态");
assert.match(storage, /avatar = COALESCE\(excluded\.avatar, peers\.avatar\)/, "收到无头像状态时不应清空本地头像缓存");
assert.match(app, /AVATAR_MAX_BYTES = 500 \* 1024/, "前端头像选择应限制 500KB");
assert.match(app, /readAsDataURL/, "前端头像应转成 base64 data URL 保存和广播");
assert.match(app, /accept="image\/\*"/, "头像选择器应只选择图片");

console.log("avatar profile checks passed");
