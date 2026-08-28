# 超管批量远程更新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持向指定设备或当前在线 Windows 设备批量下发带版本策略和下载进度的远程更新。

**Architecture:** 发送端将全员目标冻结为在线 Windows peer 快照，并逐台发送更新帧。接收端统一处理局域网文件服务与 GitHub Release 两种下载来源，在每个阶段回传进度帧；Vue 维护批次内的目标状态并展示进度。

**Tech Stack:** Tauri 2、Rust/Tokio、JSON Lines LAN 协议、Vue 3、TypeScript、Naive UI。

---

### Task 1: 协议与版本策略

**Files:**
- Modify: `src-tauri/src/protocol.rs`
- Modify: `src/types/lanchat.ts`
- Test: `src-tauri/src/protocol_tests.rs`

- [ ] **Step 1: Write failing protocol tests**
  - 覆盖默认模式、强制模式和进度帧 JSON 往返。
- [ ] **Step 2: Run tests to verify failure**
  - `cargo test --manifest-path src-tauri/Cargo.toml protocol`
- [ ] **Step 3: Add `force`、`delivery_id` 与进度帧**
- [ ] **Step 4: Re-run protocol tests**
- [ ] **Step 5: Commit protocol change**

### Task 2: 后端批量下发与接收端执行

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/network.rs`
- Modify: `src-tauri/src/storage.rs` if command/result persistence is needed
- Test: `src-tauri/src/lib.rs` unit tests or focused test module

- [ ] **Step 1: Write failing tests for version comparison and online Windows target snapshot**
- [ ] **Step 2: Verify tests fail**
- [ ] **Step 3: Add target scope resolution, per-target direct send, and version policy**
- [ ] **Step 4: Emit transfer progress and result frames while downloading**
- [ ] **Step 5: Re-run focused Rust tests**
- [ ] **Step 6: Commit backend change**

### Task 3: Tauri API and event bridge

**Files:**
- Modify: `src/services/tauri-api.ts`
- Modify: `src/services/event-bus.ts`
- Modify: `src/types/lanchat.ts`
- Test: `scripts/test-admin-remote-update.mjs`

- [ ] **Step 1: Write failing front-end contract guards**
- [ ] **Step 2: Verify guards fail**
- [ ] **Step 3: Expose scope, force, batch result and progress events**
- [ ] **Step 4: Re-run guards**
- [ ] **Step 5: Commit bridge change**

### Task 4: 超管设置交互与逐台进度面板

**Files:**
- Modify: `src/App.vue`
- Modify: `src/styles/global.css`
- Test: `scripts/test-admin-remote-update.mjs`

- [ ] **Step 1: Add failing UI guards for scope selector, force checkbox, and result list**
- [ ] **Step 2: Verify guards fail**
- [ ] **Step 3: Implement controls and event-driven per-target status UI**
- [ ] **Step 4: Re-run UI guards and `npm run build`**
- [ ] **Step 5: Commit UI change**

### Task 5: Integrated verification

**Files:**
- Modify: `docs/superpowers/specs/2026-08-28-admin-remote-update-dispatch-design.md` only if implementation differs

- [ ] **Step 1: Run focused Rust and Node tests**
- [ ] **Step 2: Run `npm run build` and `cargo check --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 3: Inspect `git diff --check` and verify each acceptance item**
- [ ] **Step 4: Commit final documentation adjustments**
