# Desktop Pet Icon And Random Clips Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用资源包 `icon.png` 展示桌宠图标，并让每种状态在动作周期结束后继续等概率随机播放对应动作。

**Architecture:** 资源扫描器负责验证图标并提供回退路径；通用桌宠资源模型提供候选过滤、周期计算和按动作取帧；原生运行时维护当前随机动作与周期起点。随机选择不使用 manifest 权重。

**Tech Stack:** Rust、eframe/egui、Vue 3、TypeScript、Node 静态检查

---

### Task 1: 资源图标优先级

**Files:**
- Modify: `src-tauri/src/desktop_pet.rs`
- Modify: `src-tauri/src/desktop_pet_tests.rs`
- Modify: `src/App.vue`
- Modify: `scripts/test-desktop-pet-ui.mjs`

- [ ] 添加 `icon.png` 优先和损坏回退测试。
- [ ] 运行测试并确认现有实现失败。
- [ ] 验证图标资源并实现 `icon -> preview -> Idle` 回退。
- [ ] 设置页优先显示 `icon_path`。
- [ ] 运行相关测试并确认通过。

### Task 2: 等概率动作调度

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/desktop_pet.rs`
- Modify: `src-tauri/src/desktop_pet_runtime.rs`
- Modify: `src-tauri/src/desktop_pet_tests.rs`
- Modify: `scripts/test-desktop-pet-runtime.mjs`

- [ ] 添加候选过滤、动作周期和运行时重新选择的失败测试。
- [ ] 运行测试并确认现有实现失败。
- [ ] 引入均匀随机索引，运行时维护当前动作。
- [ ] 动作一轮结束且状态未改变时重新选择。
- [ ] 运行 Rust、前端构建和桌宠专项测试。
