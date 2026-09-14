# LanChat Core Persistence and Optional Vision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 持久化合格游戏排行、清理瞬时上线通知、支持删除本机单聊，并生成不含任何视觉代码与资源的核心安装包。

**Architecture:** SQLite 作为聊天与排行榜的唯一持久化层，Pinia 仅保存运行期通知和界面缓存。视觉能力通过前端 feature 边界、Rust Cargo feature、Tauri 资源配置和 CI 构建矩阵共同裁剪。

**Tech Stack:** Vue 3、TypeScript、Pinia、Tauri 2、Rust、rusqlite、Vite、GitHub Actions。

---

### Task 1: 收尾现有大富翁 3D 改动

**Files:**
- Modify: `src/components/MonopolyBoard3D.vue`
- Modify: `src/App.vue`
- Test: `scripts/test-monopoly-ui.mjs`

- [ ] 运行大富翁 UI、规则、房间和构建检查。
- [ ] 确认等级信号位于城名侧边、神明文字不重复、规则页展示神明图片。

### Task 2: 瞬时在线通知与历史清理

**Files:**
- Modify: `src/stores/lanchat.ts`
- Modify: `src-tauri/src/storage.rs`
- Test: `scripts/test-message-realtime-refresh.mjs`
- Test: Rust storage tests

- [ ] 先写测试，要求在线/离线事件调用内存通知而不是 `save_system_notice`。
- [ ] 写 Rust 迁移测试，要求初始化删除公共频道历史 presence system 消息并保留其他系统消息。
- [ ] 实现 `addTransientSystemNotice` 和幂等清理 SQL。
- [ ] 运行消息与 Rust 存储测试。

### Task 3: 删除本机单聊会话

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/services/tauri-api.ts`
- Modify: `src/stores/lanchat.ts`
- Modify: `src/App.vue`
- Test: `scripts/test-private-channel-ui.mjs`
- Test: Rust storage tests

- [ ] 先写 Rust 测试，验证仅 direct 会话可被事务删除。
- [ ] 先写 UI 契约测试，验证单聊删除菜单、确认弹窗和 store 调用。
- [ ] 实现 `delete_direct_conversation` 命令及缓存清理。
- [ ] 在聊天列表加入删除入口和确认交互。
- [ ] 运行会话、消息和 Rust 测试。

### Task 4: SQLite 游戏排行榜与人数门槛

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/services/tauri-api.ts`
- Modify: `src/games/gameLeaderboard.ts`
- Modify: `src/App.vue`
- Test: `scripts/test-game-leaderboard.mjs`
- Test: `scripts/test-minesweeper-leaderboard.mjs`
- Test: Rust storage tests

- [ ] 先写资格函数测试：三名真人合格，两名真人加机器人不合格，扫雷不受限制。
- [ ] 写 SQLite upsert/list 测试，覆盖普通累计榜与扫雷明细。
- [ ] 创建幂等表结构和 Tauri 命令。
- [ ] 将启动读取、localStorage 一次迁移、结算写入和同步落库接到 API。
- [ ] 运行排行榜、房间状态和 Rust 测试。

### Task 5: 视觉识别构建级裁剪

**Files:**
- Create: `src/features/vision/index.ts`
- Create: `src/features/vision/disabled.ts`
- Modify: `src/App.vue`
- Modify: `src/services/tauri-api.ts`
- Modify: `vite.config.ts`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/tauri.vision.conf.json`
- Modify: `.github/workflows/release.yml`
- Create: `scripts/test-core-package-no-vision.mjs`

- [ ] 先写前端核心构建负向测试，扫描 bundle 不得包含视觉组件和命令。
- [ ] 先写 Rust `--no-default-features` 编译测试与资源清单测试。
- [ ] 提取视觉前端边界并接入 Vite 构建常量。
- [ ] 给 Rust 模块、状态、命令、存储类型和推理依赖添加 `vision` feature。
- [ ] 拆分核心与视觉 Tauri 资源配置。
- [ ] 增加 `build:core`、`build:vision` 与流水线矩阵。
- [ ] 运行核心负向检查及完整视觉回归测试。

### Task 6: 插件平台规划文档

**Files:**
- Create: `docs/plugin-platform-roadmap.md`

- [ ] 记录核心能力、插件 manifest、API、权限、签名、生命周期和版本兼容设计。
- [ ] 分阶段规划游戏 Web 插件、视觉原生插件、插件市场、升级与回滚。
- [ ] 明确现有游戏和视觉模块的迁移路径及验收门槛。

### Task 7: 全量验证

- [ ] 运行相关 Node 测试。
- [ ] 运行 `vue-tsc --noEmit` 和生产构建。
- [ ] 运行 Rust 测试、核心 feature 编译和视觉 feature 编译。
- [ ] 检查核心安装包/资源目录无视觉内容。
- [ ] 检查 `git diff --check` 并复核未修改用户无关文件。

