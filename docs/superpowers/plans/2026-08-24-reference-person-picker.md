# 多人参考照片目标选择 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 允许本机识别人员录入使用包含多人的照片，由用户选择目标人物后只保存该人物的裁剪参考图和特征。

**Architecture:** Rust 运行时复用现有人脸与人体检测结果，将人脸与包含它的人体框关联为可选候选项。前端先请求候选项；单目标自动入库，多目标以覆盖框弹窗让用户选择，再将候选编号回传，由 Rust 重新检测、裁剪并原子保存。

**Tech Stack:** Tauri 2 commands, Rust image/ONNX Runtime, Vue 3 + TypeScript, Naive UI。

---

### Task 1: 参考照片候选项模型

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`
- Test: `src-tauri/src/face_monitor.rs`

- [ ] **Step 1: 写候选框归一化和人脸-人体关联的失败测试。**
- [ ] **Step 2: 实现公开的候选项、选择项和分析结果类型。**
- [ ] **Step 3: 复用检测器生成稳定候选编号，优先用包含人脸的人体框作为裁剪框。**
- [ ] **Step 4: 实现按候选编号裁剪 JPEG/PNG 输入为单人参考图，并验证候选编号。**
- [ ] **Step 5: 运行 `cargo test face_monitor::tests`。**

### Task 2: Tauri 保存链路

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/services/tauri-api.ts`

- [ ] **Step 1: 新增“分析候选项”和“保存选中裁剪图”命令。**
- [ ] **Step 2: 保留原 `save_face_reference_photo` 兼容入口，创建人员时仅处理已裁剪样本。**
- [ ] **Step 3: 注册命令并增加 TypeScript API 声明。**
- [ ] **Step 4: 运行 `cargo check`。**

### Task 3: 本机录入选择界面

**Files:**
- Modify: `src/types/face-monitor.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: 定义候选项前端类型和暂存上传状态。**
- [ ] **Step 2: 上传或拍照后先分析；单候选自动加入，多候选打开紧凑选择弹窗。**
- [ ] **Step 3: 在原图中按归一化坐标绘制候选框，支持点选、取消和确认。**
- [ ] **Step 4: 确认后只展示裁剪后的预览图；失败时不污染待保存列表。**
- [ ] **Step 5: 运行 `npm run build`。**

### Task 4: 回归验证与交付

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`
- Modify: `src/App.vue`

- [ ] **Step 1: 验证单人照片自动保存、多人人像需选择、无可用人脸保持可读错误。**
- [ ] **Step 2: 运行 `cargo test face_monitor::tests`、`cargo check`、`npm run build`。**
- [ ] **Step 3: 审查 diff，提交功能分支并推送远程。**
