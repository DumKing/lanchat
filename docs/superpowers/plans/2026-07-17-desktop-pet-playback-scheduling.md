# Desktop Pet Playback Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 降低桌宠动作切换频率，并让动作数量、停顿、拖动动作和蹦迪移动方式可配置。

**Architecture:** `manifest.json` 的每个状态声明动作批次数量、单动作持续时间和动作间停顿；Rust 资源模型负责规范化配置，原生运行时负责动作组调度。桌宠本机设置保存蹦迪移动方式，Vue 设置页提供折叠资源列表、图标展示和右键配置编辑。

**Tech Stack:** Rust、serde_json、eframe/egui、Tauri 2、Vue 3、TypeScript、Naive UI

---

### Task 1: 状态播放配置

**Files:**
- Modify: `src-tauri/src/desktop_pet.rs`
- Modify: `src-tauri/src/desktop_pet_tests.rs`
- Modify: `src-tauri/resources/desktop-pets/*/manifest.json`

- [ ] 添加旧 manifest 默认值和自定义范围的失败测试。
- [ ] 实现状态播放配置解析、范围归一化和默认值。
- [ ] 验证并使用 `icon.png` 图标。
- [ ] 运行 Rust 单元测试。

### Task 2: 原生运行时动作调度

**Files:**
- Modify: `src-tauri/src/desktop_pet_runtime.rs`
- Modify: `scripts/test-desktop-pet-runtime.mjs`

- [ ] 添加动作停留、Interact 单动作、Life 动作组和拖动 Move 的失败检查。
- [ ] 实现动作组、随机间隔和状态完成通知。
- [ ] 实现拖动方向动作组和线性/跳跃蹦迪移动。
- [ ] 运行桌宠运行时专项测试。

### Task 3: 设置与 manifest 编辑

**Files:**
- Modify: `src-tauri/src/desktop_pet.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/types/desktop-pet.ts`
- Modify: `src/types/lanchat.ts`
- Modify: `src/services/tauri-api.ts`
- Modify: `src/stores/desktopPet.ts`
- Modify: `src/App.vue`
- Modify: `src/styles/global.css`
- Modify: `scripts/test-desktop-pet-ui.mjs`

- [ ] 添加设置兼容、折叠列表、图标和右键编辑的失败测试。
- [ ] 持久化 `discoMovementMode` 并同步到原生运行时。
- [ ] 实现资源列表折叠、图片条目和右键编辑弹窗。
- [ ] 内置资源编辑时复制为用户覆盖包，再保存并热刷新。
- [ ] 运行前端专项测试和构建。

### Task 4: manifest 文档与整体验证

**Files:**
- Create: `docs/desktop-pet-manifest.md`

- [ ] 列出全部字段、类型、可选值、默认值和目录映射。
- [ ] 运行 `cargo test`、`cargo check`、专项 Node 测试、`npm run build`。
- [ ] 运行 `git diff --check`。
