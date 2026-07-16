import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile("src/App.vue", "utf8");

assert.match(source, /function openBuiltinGame\(type: GameType\)/, "内置游戏点击应有独立入口函数");
assert.match(source, /@click="openBuiltinGame\(game\.type\)"/, "左侧内置游戏卡片应调用入口函数");
assert.match(source, /activeGameRoomId\.value = ""/, "点击内置游戏应清空当前房间选择");
assert.match(source, /class="game-catalog-board"/, "未选中房间时右侧应展示游戏目录排行榜视图");
assert.match(source, /<NTabs[\s\S]*class="minesweeper-leaderboard-tabs"/, "扫雷排行榜应使用难度 Tab 展示");
assert.match(source, /v-for="difficulty in MINESWEEPER_DIFFICULTIES"/, "扫雷排行榜 Tab 应来自难度列表");
assert.doesNotMatch(source, /v-if="activeGameRoom\?\.gameType === 'minesweeper' \|\| selectedGameType === 'minesweeper'" class="leaderboard-grid"/, "扫雷目录排行榜不应再使用多个难度卡片");

console.log("game catalog view ok");