# LanChat 大富翁游戏 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 LanChat 局域网游戏房间内实现可同步、可断线恢复的平面大富翁游戏。

**Architecture:** 新增独立的纯 TypeScript 规则引擎，集中维护 40 格棋盘、房产、联排、神明、随机事件、道具、回合和结算；`App.vue` 仅保留房间适配、房主动作裁决和界面渲染。继续使用现有 Rust `GameFrame` 转发通道，房主广播快照，客户端不自行产生随机结果。

**Tech Stack:** Vue 3、TypeScript、Pinia、Naive UI、现有 Tauri 2/Rust GameFrame、Node assert + esbuild 规则测试。

---

### Task 1: 大富翁领域模型与固定棋盘

**Files:**
- Create: `src/games/monopoly.ts`
- Create: `scripts/test-monopoly-rules.mjs`
- Modify: `package.json`

- [ ] **Step 1: 写入失败测试**

验证 `createMonopolyBoard()` 生成 40 格棋盘、四个角落位置正确、每边八块地产与中央事件格正确、地产区最多四格。

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: FAIL，提示 `src/games/monopoly.ts` 或棋盘 API 尚不存在。

- [ ] **Step 3: 实现最小棋盘与类型**

实现棋盘常量、地块类别、建筑等级、玩家/房间状态、统一价格表、`createMonopolyBoard()` 与棋盘位置工具函数。

- [ ] **Step 4: 运行通过测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: PASS，棋盘为 40 格，32 地产、4 事件、4 角落。

- [ ] **Step 5: 提交**

```bash
git add src/games/monopoly.ts scripts/test-monopoly-rules.mjs package.json
git commit -m "feat: add monopoly board model"
```

### Task 2: 现金、地产、联排与回合引擎

**Files:**
- Modify: `src/games/monopoly.ts`
- Modify: `scripts/test-monopoly-rules.mjs`

- [ ] **Step 1: 写入失败测试**

覆盖空地 350 购买即小屋、小屋/二级/三级的统一过路费与升级价、银行暂持建筑直接购买、2 至 4 格联排收费、查封切断联排、金币唯一结算依据。

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: FAIL，地产交易、联排或查封 API 缺失。

- [ ] **Step 3: 实现最小规则**

实现房产购买/升级/出售/破产、银行暂持、过路费、联排、查封回合计数、30 秒回合、单骰移动、起点、机场、翻倍角和监狱。

- [ ] **Step 4: 运行通过测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: PASS，包含查封 `1+2+3+4` 后只按 `1+2` 收费的场景。

- [ ] **Step 5: 提交**

```bash
git add src/games/monopoly.ts scripts/test-monopoly-rules.mjs
git commit -m "feat: add monopoly economy and turns"
```

### Task 3: 神明、事件、动态道具概率与确定性随机

**Files:**
- Modify: `src/games/monopoly.ts`
- Modify: `scripts/test-monopoly-rules.mjs`

- [ ] **Step 1: 写入失败测试**

覆盖神明优先级、财神/穷鬼/天使/恶魔、三回合刷新、初始三张道具、三张满背包不发卡、手动弃卡、抢占卡 3% 基础概率、落后玩家最高 8% 抢占概率、路障对布置者生效、转向永久生效。

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: FAIL，神明、随机事件或动态发卡 API 缺失。

- [ ] **Step 3: 实现最小规则**

实现可序列化的种子随机源、神明刷新与覆盖、十种随机事件、十种道具、强力卡上限、补偿点数和基础/落后卡池插值。

- [ ] **Step 4: 运行通过测试**

Run: `node scripts/test-monopoly-rules.mjs`

Expected: PASS，所有随机结果都可用相同种子复现。

- [ ] **Step 5: 提交**

```bash
git add src/games/monopoly.ts scripts/test-monopoly-rules.mjs
git commit -m "feat: add monopoly gods and cards"
```

### Task 4: 房间注册、状态快照与房主裁决

**Files:**
- Modify: `src/games/registry.ts`
- Modify: `src/games/gameLeaderboard.ts`
- Modify: `src/App.vue`
- Modify: `scripts/test-game-catalog-view.mjs`
- Create: `scripts/test-monopoly-room.mjs`

- [ ] **Step 1: 写入失败测试**

验证游戏目录显示“大富翁”、房间最多 4 人、创建参数支持 5000 至 50000 金币与 5 至 50 回合、客户端动作只发给房主、房主更新后广播快照。

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-monopoly-room.mjs`

Expected: FAIL，注册项、房间状态或大富翁动作分派不存在。

- [ ] **Step 3: 实现房间适配**

在 `App.vue` 增加 Monopoly 状态表、房主动作分派、加入/准备/开始/离开/超时、快照编解码、结果写入通用胜率排行榜，并保证不改 Rust `GameFrame` 结构。

- [ ] **Step 4: 运行通过测试**

Run: `node scripts/test-monopoly-room.mjs && node scripts/test-game-catalog-view.mjs`

Expected: PASS，现有游戏目录断言保持通过。

- [ ] **Step 5: 提交**

```bash
git add src/games/registry.ts src/games/gameLeaderboard.ts src/App.vue scripts/test-game-catalog-view.mjs scripts/test-monopoly-room.mjs
git commit -m "feat: integrate monopoly game rooms"
```

### Task 5: 平面棋盘界面与房间交互

**Files:**
- Modify: `src/App.vue`
- Create: `scripts/test-monopoly-ui.mjs`

- [ ] **Step 1: 写入失败测试**

验证大富翁房间拥有独立平面棋盘、四角、四边地块、玩家状态区、30 秒倒计时、骰子/道具操作、半透明房间聊天和结算层的稳定选择器。

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-monopoly-ui.mjs`

Expected: FAIL，界面选择器和大富翁模板不存在。

- [ ] **Step 3: 实现界面**

复用既有房间布局和头像工具，新增响应式平面棋盘、地块状态颜色、联排/查封/翻倍/神明/路障标识、玩家侧栏、道具背包、机场目标选择、事件日志与现金结算层。

- [ ] **Step 4: 运行通过测试与构建**

Run: `node scripts/test-monopoly-ui.mjs && npm run build`

Expected: PASS，Vite 与 TypeScript 构建通过。

- [ ] **Step 5: 提交**

```bash
git add src/App.vue scripts/test-monopoly-ui.mjs
git commit -m "feat: add monopoly board interface"
```

### Task 6: 回归、文档与交付

**Files:**
- Create: `docs/monopoly-game-design.md`
- Modify: `README.md`（仅在项目已有游戏清单位置补充大富翁时）

- [ ] **Step 1: 将已确认设计文档纳入功能分支**

保留统一价格、现金结算、银行暂持、查封联排、初始三卡与动态概率等已确认规则。

- [ ] **Step 2: 运行规则与现有游戏回归**

Run: `npm run test:gomoku && npm run test:xiangqi && npm run test:minesweeper && node scripts/test-monopoly-rules.mjs && node scripts/test-monopoly-room.mjs && node scripts/test-monopoly-ui.mjs && npm run build && cargo check --manifest-path src-tauri/Cargo.toml`

Expected: 全部通过。

- [ ] **Step 3: 提交**

```bash
git add docs/monopoly-game-design.md README.md
git commit -m "docs: add monopoly game design"
```
