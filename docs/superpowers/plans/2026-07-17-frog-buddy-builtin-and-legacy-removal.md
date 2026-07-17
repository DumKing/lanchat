# Frog Buddy Built-in and Legacy Frog Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 内置 `frog-buddy` 并设为默认桌宠，同时移除旧版固定青蛙视觉链路和兼容接口。

**Architecture:** 继续使用现有 DesktopPet Package Registry 与 Rust 原生透明窗口，将原生渲染器彻底改为资源包驱动。告警业务保持不变，前后端命名改为角色无关的 desktop pet 术语。

**Tech Stack:** Tauri 2、Rust、eframe/egui、Vue 3、TypeScript、Node 静态回归测试。

---

### Task 1: 默认桌宠与失效回退

**Files:**
- Modify: `src-tauri/src/desktop_pet.rs`
- Modify: `src-tauri/src/desktop_pet_tests.rs`

- [ ] 先写 `frog-buddy` 默认选择、有效选择保留、无效选择回退测试。
- [ ] 运行 `cargo test desktop_pet_tests`，确认新增测试因旧默认逻辑失败。
- [ ] 实现默认选择和失效回退。
- [ ] 再次运行测试确认通过。

### Task 2: 内置新资源包

**Files:**
- Create: `src-tauri/resources/desktop-pets/frog-buddy/**`

- [ ] 将已校验的 `D:\lcpic\frog-buddy` 原样复制到内置资源目录。
- [ ] 运行 Package V2 校验工具验证内置副本。
- [ ] 核对五类状态实际帧数和 Manifest ID。

### Task 3: 原生桌宠运行时通用化

**Files:**
- Rename: `src-tauri/src/native_frog_pet.rs` -> `src-tauri/src/desktop_pet_runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `scripts/test-desktop-pet-runtime.mjs`

- [ ] 先写静态回归测试，要求运行时不再包含固定青蛙图集和 frog 兼容事件/API。
- [ ] 运行测试确认失败。
- [ ] 重命名控制器、状态、动作事件、快捷键和日志。
- [ ] 删除编译期图集、固定姿态索引和旧图集回退绘制。
- [ ] 保留动态包帧绘制、状态机、详情窗口、拖动、缩放和蹦迪位移。
- [ ] 运行静态测试、Rust 测试和 `cargo check`。

### Task 4: 前端桌宠接口通用化

**Files:**
- Modify: `src/App.vue`
- Modify: `src/types/lanchat.ts`
- Modify: `src/stores/lanchat.ts`
- Modify: `src/api.ts`
- Modify: `src/styles/global.css`
- Create: `scripts/test-desktop-pet-ui.mjs`

- [ ] 先写静态回归测试，要求前端只调用通用桌宠命令和事件。
- [ ] 运行测试确认失败。
- [ ] 将 Frog 类型和本地状态命名迁移为 DesktopPet/PetAlert。
- [ ] 删除不再创建的 `frog-pet` WebView 模板、尺寸同步和专属 CSS。
- [ ] 保留设置页、告警发送、状态同步和快捷键交互。
- [ ] 运行静态测试与前端构建。

### Task 5: 旧资源和脚本清理

**Files:**
- Delete: `public/pet-assets/**`
- Delete: `scripts/build-generated-frog-gifs.py`
- Delete: `scripts/generate-frog-gifs.py`
- Delete: `scripts/test-frog-*.mjs`
- Delete: `scripts/test-native-frog-pet.mjs`
- Modify: `package.json`

- [ ] 删除旧视觉资产、生成脚本和被新测试替代的旧静态测试。
- [ ] 更新 npm 测试脚本名称。
- [ ] 使用 `git ls-files` 和 `rg` 确认不存在旧图集/旧兼容接口引用。

### Task 6: 完整验证

**Files:**
- Verify: all changed files

- [ ] 运行桌宠专项 Node 测试。
- [ ] 运行 `cargo test desktop_pet_tests`。
- [ ] 运行 `cargo test`。
- [ ] 运行 `npm run build`。
- [ ] 运行 `cargo check`。
- [ ] 检查 `git diff --check`、工作区变更和内置资源清单。
