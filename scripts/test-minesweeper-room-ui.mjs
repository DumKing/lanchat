import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const appVue = readFileSync(resolve('src/App.vue'), 'utf8');
const start = appVue.indexOf(`activeGameRoom?.gameType === 'minesweeper'`);
const end = appVue.indexOf("activeGameRoom?.gameType === 'gomoku'", start);

assert.notEqual(start, -1, '应该存在扫雷竞速房间视图');
assert.notEqual(end, -1, '应该能定位扫雷竞速房间视图结束位置');

const minesweeperRoom = appVue.slice(start, end);

assert.ok(!minesweeperRoom.includes('minesweeper-status-panel'), '扫雷竞速不应再展示顶部横向状态框');
assert.ok(!minesweeperRoom.includes('未开始'), '扫雷竞速房间内不应再出现顶部“未开始”状态文案');
assert.ok(minesweeperRoom.includes('class="minesweeper-board-meta"'), '扫雷棋盘上方应保留信息栏');
assert.ok(minesweeperRoom.includes('class="minesweeper-meta-chip difficulty"'), '扫雷难度应显示为棋盘上方可点击类型胶囊');
assert.ok(minesweeperRoom.includes('@select="selectMinesweeperDifficulty"'), '扫雷类型胶囊应复用难度切换逻辑');
assert.ok(
  minesweeperRoom.indexOf('class="minesweeper-board-meta"') < minesweeperRoom.indexOf('class="minesweeper-board"'),
  '扫雷类型切换应放在棋盘上方'
);

console.log('minesweeper room ui checks passed');