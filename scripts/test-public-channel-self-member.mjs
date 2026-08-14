import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile(new URL("../src/App.vue", import.meta.url), "utf8");

assert.match(app, /const publicChannelMembers = computed/, "公开局域网频道应构建包含本机的成员列表");
assert.match(app, /channel_id:\s*DEFAULT_GROUP_ID/, "本机成员记录应属于默认局域网频道");
assert.match(app, /online:\s*true/, "本机在频道成员列表中应始终显示在线");
assert.match(app, /chatCapablePeers\.value\.filter\(\(peer\) => !sameDeviceId\(peer\.device_id, profile\.value\?\.device_id\)\)/, "公开频道成员应按设备标识去重本机");
assert.match(app, /channel\.is_private\s*\?[^:]+:\s*publicChannelMembers\.value/s, "设备通讯录中的公开频道也应使用含本机的数据源");
assert.match(app, /activeConversation\.value\?\.is_private\s*\?[^:]+:\s*publicChannelMembers\.value/s, "聊天右侧公开频道成员列表应使用含本机的数据源");

console.log("public channel self member guards passed");
