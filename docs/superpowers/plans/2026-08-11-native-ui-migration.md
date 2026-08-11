# LanChat 全量原生 UI 迁移 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 LanChat 主程序迁移为 Slint + Rust 原生桌面应用，同时保持现有本地数据、局域网协议、桌宠、游戏和 Windows 原生通话兼容。

**Architecture:** 新增独立的 `native_app` 应用层，承接现有存储、网络和桌宠服务，并把状态映射为 Slint 模型。迁移期保留旧 Tauri/Vue 入口用于回归；原生入口先成为可独立运行的 Windows 可执行程序，全部页面迁完后再替换默认入口与打包链路。

**Tech Stack:** Rust、Slint、SQLite、Tokio、现有 mDNS/TCP 协议、eframe 桌宠、Windows 原生媒体设备与 Rust WebRTC。

---

### Task 1: 原生应用壳与构建开关

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/src/native_app/mod.rs`
- Create: `src-tauri/src/native_app/app.rs`
- Create: `src-tauri/ui/main.slint`
- Test: `src-tauri/src/native_app/app.rs`

- [x] 添加 Slint 运行时与构建依赖，保留 Tauri 依赖供旧入口回归。
- [x] 增加 `--native-ui` 启动参数，启动 Slint 原生主窗口。
- [x] 创建原生三栏空壳，包含导航、会话列表、聊天内容和输入区占位。
- [x] 写入原生应用配置与启动单元测试。
- [x] 运行 `cargo test native_app` 与 `cargo check`。
- [ ] 提交 `feat: add native Slint application shell`。

### Task 2: 应用服务层与数据兼容桥

**Files:**
- Create: `src-tauri/src/native_app/services.rs`
- Create: `src-tauri/src/native_app/models.rs`
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/network.rs`
- Test: `src-tauri/src/native_app/services.rs`

- [x] 先写失败测试：旧数据库 profile、会话和设备可映射为原生模型。
- [x] 提取 `NativeAppServices`，封装 profile、会话、peer 与消息读取操作。
- [ ] 在后续设备/频道页面迁移中补齐频道写入与成员管理操作。
- [x] 复用 MAC 标识、SQLite 路径和消息协议，不引入并行数据源。
- [x] 运行针对性测试与 `cargo test --lib`。
- [ ] 提交 `feat: bridge existing services to native UI`。

### Task 3: 原生聊天与分页媒体缓存

**Files:**
- Create: `src-tauri/src/native_app/chat.rs`
- Create: `src-tauri/ui/chat.slint`
- Modify: `src-tauri/src/storage.rs`
- Test: `src-tauri/src/native_app/chat.rs`

- [x] 写失败测试：初始消息页仅返回最近 20 条。
- [x] 添加 `list_messages_page(conversation_id, before, limit)`，按时间倒序查询后正序呈现。
- [ ] 实现原生消息列表、向上加载、文本发送、状态图标和系统消息。
- [ ] 实现缩略图索引、按需原图解码与 LRU 释放。
- [ ] 运行消息分页与存储回归测试。
- [ ] 提交 `feat: add native paged chat timeline`。

### Task 4: 设备、频道与设置页面

**Files:**
- Create: `src-tauri/src/native_app/contacts.rs`
- Create: `src-tauri/src/native_app/settings.rs`
- Create: `src-tauri/ui/contacts.slint`
- Create: `src-tauri/ui/settings.slint`
- Test: `src-tauri/src/native_app/contacts.rs`

- [ ] 迁移设备列表、详情、备注、在线排序和管理员设备操作。
- [ ] 迁移公共/私有频道、成员、公告、禁言、邀请和退出能力。
- [ ] 迁移个人资料、头像压缩、桌宠、外部推送与管理员设置。
- [ ] 运行旧数据库读取与频道权限回归测试。
- [ ] 提交 `feat: migrate contacts channels and settings to Slint`。

### Task 5: 告警、桌宠与排行榜

**Files:**
- Create: `src-tauri/src/native_app/alerts.rs`
- Create: `src-tauri/ui/alerts.slint`
- Modify: `src-tauri/src/desktop_pet_runtime.rs`
- Test: `src-tauri/src/native_app/alerts.rs`

- [ ] 迁移狼来了排行榜、可信度、反馈与管理员下发配置。
- [ ] 通过单一服务状态同步桌宠角标、详情和来电提示。
- [ ] 验证反馈后的角标/详情立即清理，旧快照不能回写。
- [ ] 提交 `feat: migrate alerts and desktop pet controls`。

### Task 6: 原生游戏 UI

**Files:**
- Create: `src-tauri/src/native_app/games.rs`
- Create: `src-tauri/ui/games/*.slint`
- Test: `src-tauri/src/native_app/games.rs`

- [ ] 迁移房间列表与创建房间交互。
- [ ] 分别迁移斗地主、五子棋、象棋、扫雷竞速的绘制与输入。
- [ ] 复用现有游戏帧、回合、排行榜与聊天规则。
- [ ] 运行规则和双机房间集成测试。
- [ ] 提交 `feat: migrate built-in games to native UI`。

### Task 7: Windows 原生音视频通话

**Files:**
- Create: `src-tauri/src/native_app/call/mod.rs`
- Create: `src-tauri/src/native_app/call/media_windows.rs`
- Create: `src-tauri/ui/call.slint`
- Modify: `src-tauri/src/protocol.rs`
- Test: `src-tauri/src/native_app/call/mod.rs`

- [ ] 写失败测试：CallService 对 offer/answer/ICE/hangup 的状态转换。
- [ ] 将现有 TCP `CallSignalFrame` 接入原生 CallService。
- [ ] 接入 Windows 摄像头、麦克风和扬声器，建立原生 WebRTC 轨道。
- [ ] 实现独立、不透明、可跨屏移动的通话窗口与桌宠快捷接听。
- [ ] 对权限拒绝、设备占用、断线重连和资源释放做回归。
- [ ] 提交 `feat: add Windows native audio and video calls`。

### Task 8: 切换默认入口与移除 WebView 依赖

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/build.rs`
- Modify: `.github/workflows/release.yml`
- Remove: `src/`
- Remove: `package.json`
- Test: `src-tauri/tests/native_upgrade.rs`

- [ ] 在旧数据库副本上验证升级、启动、消息、频道、游戏和桌宠配置。
- [ ] 验证主窗口在长时间聊天和大图历史下没有持续内存增长。
- [ ] 切换默认入口为原生 UI，保留 `--legacy-web-ui` 仅用于临时回归。
- [ ] 最后移除 Vue、Vite、Tauri 主界面构建依赖与发布资产。
- [ ] 运行全量测试、Windows 构建与双机验收。
- [ ] 提交 `feat: complete native LanChat migration`。
