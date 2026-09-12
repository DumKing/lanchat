import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const root = await readFile("src/App.vue", "utf8");
const source = await readFile("src/app/AppShell.vue", "utf8");
const gamesPage = await readFile("src/pages/GamesPage.vue", "utf8");
const sidebar = await readFile("src/components/plugins/PluginGamesSidebar.vue", "utf8");

assert.match(source, /import PluginGamesSidebar from/, "主界面应使用独立的插件游戏导航组件");
assert.match(root, /<AppShell\s*\/>/, "根组件应只挂载应用外壳");
assert.ok(root.split(/\r?\n/).length < 30, "根组件不应重新承载页面业务");
assert.match(source, /const GamesPage = defineAsyncComponent/, "应用外壳应按需加载独立的插件游戏页面");
assert.match(gamesPage, /import PluginViewport from/, "插件游戏页面应使用隔离的插件运行视图");
assert.match(source, /<PluginGamesSidebar[\s\S]*@select="openPluginGame"/, "游戏入口应只列出已启用插件贡献的游戏");
assert.match(source, /<GamesPage[\s\S]*:plugin-id="activePluginGame\.pluginId"/, "应用外壳应把当前游戏交给独立页面");
assert.match(gamesPage, /<PluginViewport[\s\S]*:plugin-id="pluginId"/, "游戏内容应由插件视图加载");
assert.match(gamesPage, /\.plugin-game-workspace\s*\{[\s\S]*display:\s*block;[\s\S]*height:\s*100%/, "插件游戏页面必须覆盖旧游戏三行网格并占满内容区");
assert.match(sidebar, /v-for="game in games"/, "插件游戏导航应根据清单贡献动态渲染");
assert.doesNotMatch(source, /openBuiltinGame|createRoomOpen|class="game-catalog-board"/, "主程序不应保留旧版内置游戏目录和建房入口");
assert.doesNotMatch(source, /LANCHAT_GAME_INVITE:/, "主程序不应保留旧版内置游戏邀请协议");
assert.match(source, /LANCHAT_PLUGIN_GAME_INVITE:/, "主程序应使用标准插件游戏邀请卡片协议");
assert.match(source, /pluginGameInvitePayload\(message\)/, "聊天消息应识别插件游戏邀请卡片");

console.log("plugin game catalog view ok");
