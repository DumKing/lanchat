# LanChat 本地视觉识别运行时重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有同步、固定模型的人脸/人体识别改造成 CPU 优先、模型可插拔、可恢复且不影响通话共享摄像头的本地视觉识别运行时。

**Architecture:** 保留 WebView `MediaStream` 作为唯一摄像头所有者，前端只提交带流身份的最新 Raw RGBA 帧；Rust 专用 Worker 负责丢弃过期帧、模型推理、短时追踪、融合、告警和状态事件。新 `vision` 域与现有 `face_monitor` 并存，通过兼容适配层逐步接管命令、数据库和局域网策略，旧实现仅在完整迁移期间保留为降级路径。

**Tech Stack:** Tauri 2、Rust 2021、Tokio、rusqlite、ort/ONNX Runtime、serde、Ed25519、Vue 3、TypeScript、WebRTC MediaStream、Naive UI。

---

## 文件结构与职责

### 新建 Rust 模块

- `src-tauri/src/vision/mod.rs`：视觉域公共导出、运行时装配。
- `src-tauri/src/vision/types.rs`：版本化 DTO、状态机、错误码、EmbeddingSpace 与识别结果。
- `src-tauri/src/vision/manifest.rs`：Manifest V3、严格 SemVer、哈希与兼容性校验。
- `src-tauri/src/vision/registry.rs`：模型 Profile 注册、Catalog、缓存/LKG 选择。
- `src-tauri/src/vision/runtime.rs`：`ActiveVisionProfile` 原子快照、生命周期、会话恢复与状态发布。
- `src-tauri/src/vision/worker.rs`：单独 Rust Worker、LatestFrameMailbox、帧流切换与背压。
- `src-tauri/src/vision/tracking.rs`：IoU/中心点/人体外观的短时 Track，默认 2.5 秒 TTL。
- `src-tauri/src/vision/matching.rs`：质量门禁、多样本原型 + Top-K 一致性匹配、分数标准化。
- `src-tauri/src/vision/alert.rs`：身份融合、Temporal Gate、冷却与 `vision_alert_events` 写入。
- `src-tauri/src/vision/activation.rs`：安装、Smoke Test、Benchmark、特征重算、原子切换与回滚。
- `src-tauri/src/vision/protocol.rs`：`VisionPolicyFrame`、远程命令幂等收据和兼容转换。
- `src-tauri/src/vision/storage.rs`：V5 视觉表、备份迁移、任务/Activation/运行时持久化。

### 修改的现有 Rust 文件

- `src-tauri/src/lib.rs`：注册 `VisionService`、新 Tauri commands/events；旧 `submit_face_monitor_frame` 适配新服务直到旧 UI 删除。
- `src-tauri/src/storage.rs`：调用视觉迁移入口，保留旧表读取和历史告警兼容查询。
- `src-tauri/src/protocol.rs`：增加版本化 Vision Frame，同时解析旧 `FaceMonitorPolicyFrame`。
- `src-tauri/src/network.rs`：发送/接收新 Frame，事务幂等后异步调度。
- `src-tauri/src/face_monitor.rs`：仅保留旧模型和 DTO 的兼容适配；不再承载新 Worker 或新策略逻辑。
- `src-tauri/Cargo.toml`：加入签名、SemVer、ZIP 和加密导入所需的最小依赖。

### 新建/修改的前端文件

- `src/types/vision.ts`：新运行时、Profile、任务、参考图、告警及策略类型。
- `src/services/visionFrameTransport.ts`：Raw Frame Envelope 编码、`stream_id` 与 generation 管理。
- `src/components/VisionModelCenter.vue`：左侧“视觉识别”工作区，展示预设 Profile、兼容性和任务状态。
- `src/components/VisionPeoplePanel.vue`：人员库、3 至 30 张参考图和质量结果。
- `src/components/VisionRuntimeStatus.vue`：托盘/侧栏可复用状态与暂停恢复控件。
- `src/services/cameraMediaCoordinator.ts`：改为 `requestVideoFrameCallback` 优先采样，保持 preview/call/monitor Lease 语义。
- `src/services/tauri-api.ts`：追加新视觉命令，旧 API 仅保留过渡调用。
- `src/types/face-monitor.ts`、`src/App.vue`：逐步改接新契约，保留历史告警显示与通话摄像头复用。
- `src/i18n.ts`：补充中英文视觉识别文案。

### 测试文件

- `src-tauri/src/vision/*_tests.rs` 或各模块内 `#[cfg(test)]`：纯 Rust 单元/集成测试。
- `scripts/test-vision-runtime.mjs`：前端类型与命令封装契约检查。
- `scripts/test-vision-frame-transport.mjs`：Envelope、流切换和最新帧策略测试。
- `scripts/test-vision-ui.mjs`：入口、Profile/人员库/状态控件与中英文文案检查。

## 任务 1：建立视觉域类型与失败测试

**Files:**
- Create: `src-tauri/src/vision/mod.rs`
- Create: `src-tauri/src/vision/types.rs`
- Create: `src-tauri/src/vision/types_tests.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写入状态正交性失败测试**

```rust
#[test]
fn user_pause_survives_restart_but_resource_pause_does_not() {
    assert_eq!(restore_sampling_state(true, SamplingState::Running), SamplingState::PausedByUser);
    assert_eq!(restore_sampling_state(false, SamplingState::Starved), SamplingState::Running);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test vision::types_tests::user_pause_survives_restart_but_resource_pause_does_not`

Expected: FAIL，模块或函数不存在。

- [ ] **Step 3: 实现最小公共 DTO 与状态机**

```rust
pub enum VisionLifecycleState { Disabled, Initializing, Ready, RebuildingSession, RollingBack, Failed }
pub enum VisionSamplingState { Running, PausedByUser, PausedByResourceConflict, Starved }
pub enum VisionPerformanceState { Normal, Degraded, Recovering }
pub struct VisionRuntimeSnapshot { /* 生命周期、采样、性能、活动 Profile、revision、reason_code */ }
```

`RecognitionResult`、`RecognitionEvidence`、`ProfileActivation`、`VisionTaskSnapshot` 和所有错误码必须只定义在此层；禁止继续在 `face_monitor.rs` 扩散新 DTO。

- [ ] **Step 4: 运行模块测试**

Run: `cargo test vision::types_tests`

Expected: PASS。

- [ ] **Step 5: 提交类型边界**

```bash
git add src-tauri/src/vision src-tauri/src/lib.rs
git commit -m "feat: add vision runtime core types"
```

## 任务 2：实现 Manifest V3 与 Profile 兼容校验

**Files:**
- Create: `src-tauri/src/vision/manifest.rs`
- Create: `src-tauri/src/vision/manifest_tests.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/resources/object-models/manifest.json`

- [ ] **Step 1: 写入 Profile 版本和资产校验失败测试**

```rust
#[test]
fn rejects_profile_when_profile_version_differs_from_package_version() { /* assert invalid */ }

#[test]
fn embedding_space_changes_when_preprocessing_changes() { /* assert_ne */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::manifest_tests`

Expected: FAIL。

- [ ] **Step 3: 增加最小依赖并实现校验器**

增加 `semver`、`ed25519-dalek`、`zip`、`argon2`、`aes-gcm` 前先确认 feature 最小化；实现 `package.version == profile.version`、严格递增版本、组件 SHA-256、Adapter Registry、EmbeddingSpace 语义指纹和模型尺寸限制。官方包只允许数据资产，拒绝 DLL、EXE、WASM 和未知可执行文件。

- [ ] **Step 4: 把当前四个 ONNX 模型转换为 baseline Manifest V3**

保留原模型文件，补齐每个组件输入/输出、颜色空间、归一化、Adapter ID、许可证与资源声明；Smoke Test 未通过前不得标记兼容。

- [ ] **Step 5: 验证 Manifest 与 Rust 编译**

Run: `cargo test vision::manifest_tests; cargo check`

Expected: PASS。

- [ ] **Step 6: 提交 Manifest 层**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/resources/object-models src-tauri/src/vision
git commit -m "feat: validate vision model manifests"
```

## 任务 3：先完成可回滚的 SQLite V5 迁移

**Files:**
- Create: `src-tauri/src/vision/storage.rs`
- Create: `src-tauri/src/vision/storage_tests.rs`
- Modify: `src-tauri/src/storage.rs`

- [ ] **Step 1: 为旧库迁移写失败测试**

```rust
#[test]
fn migrates_legacy_face_samples_without_dropping_alert_history() { /* seed face_people/camera_face_alerts */ }

#[test]
fn remote_command_receipt_is_idempotent_per_issuer_target_and_command() { /* duplicate insert */ }
```

- [ ] **Step 2: 运行迁移测试确认失败**

Run: `cargo test vision::storage_tests`

Expected: FAIL。

- [ ] **Step 3: 实现备份与事务迁移**

在关闭采样后创建 SQLite 在线备份；使用 `BEGIN IMMEDIATE` 创建 `vision_*` 表、索引和版本校验。迁移真实的 `face_people`、`face_person_samples`、`camera_face_alerts`、`camera_face_alert_feedbacks`，绝不覆盖旧表。写入 `vision_schema_migrations` 后执行 `foreign_key_check` 和 `integrity_check`。

- [ ] **Step 4: 实现恢复状态和远程收据存储**

保存 `sampling_state`、`performance_state`、`user_paused`、Activation revision/Job 关联，以及 `(issuer_device_id,target_device_id,command_id)` 唯一收据与 `result_json`。

- [ ] **Step 5: 验证迁移与现有 Storage 测试**

Run: `cargo test storage::tests vision::storage_tests`

Expected: PASS，旧历史告警仍可查询。

- [ ] **Step 6: 提交迁移层**

```bash
git add src-tauri/src/storage.rs src-tauri/src/vision/storage.rs src-tauri/src/vision/storage_tests.rs
git commit -m "feat: add vision runtime storage migration"
```

## 任务 4：实现模型 Registry、缓存和安装安全边界

**Files:**
- Create: `src-tauri/src/vision/registry.rs`
- Create: `src-tauri/src/vision/registry_tests.rs`
- Modify: `src-tauri/src/vision/storage.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写入签名、回滚和 LRU 失败测试**

```rust
#[test]
fn rejects_unsigned_remote_package_but_labels_local_import_unsigned() { /* assert source policy */ }

#[test]
fn eviction_never_removes_baseline_active_or_last_known_good() { /* assert protected */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::registry_tests`

Expected: FAIL。

- [ ] **Step 3: 实现 Registry**

实现签名 Catalog、根密钥/轮换/吊销、离线 Catalog 缓存、系统代理动态读取、GitHub Token、镜像重试与断点续传。默认缓存上限 2GB；baseline、Active、LKG 永不被 LRU 清理。

- [ ] **Step 4: 实现本地导入与官方 Catalog 差异**

本地 ZIP 允许高级用户导入但明确标识未签名；远程超管策略不可强制未签名包。安装完成先解压到 staging，校验后原子改名。

- [ ] **Step 5: 运行 Registry 测试**

Run: `cargo test vision::registry_tests`

Expected: PASS。

- [ ] **Step 6: 提交 Registry**

```bash
git add src-tauri/src/vision/registry.rs src-tauri/src/vision/registry_tests.rs src-tauri/src/vision/storage.rs src-tauri/src/lib.rs
git commit -m "feat: add signed vision model registry"
```

## 任务 5：实现 LatestFrameMailbox 与 Rust Worker

**Files:**
- Create: `src-tauri/src/vision/worker.rs`
- Create: `src-tauri/src/vision/worker_tests.rs`
- Modify: `src-tauri/src/vision/runtime.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写入最新帧和流切换失败测试**

```rust
#[test]
fn mailbox_replaces_stale_frame_and_counts_drop() { /* submit A then B */ }

#[test]
fn new_stream_discards_old_tracks_and_results() { /* switch stream generation */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::worker_tests`

Expected: FAIL。

- [ ] **Step 3: 实现 Raw Envelope 和单 Worker**

`submit_vision_frame_raw` 只做 envelope/尺寸/时间戳校验并替换邮箱最新帧；禁止 PNG/JPEG/Base64 和 `Vec<u8>` JSON 数组。Worker 在专用线程运行，只有采样、模型加载和 Benchmark 在后台；UI/Tauri handler 不阻塞推理。

- [ ] **Step 4: 实现流身份隔离与恢复**

流变更清空内存 Track、连续命中和 `track_id -> alert_id`；数据库级人员冷却、反馈与历史不删除。连续 3 次推理失败重建 Session，随后 LKG，再到 baseline，失败 Profile 隔离 10 分钟。

- [ ] **Step 5: 验证 Worker 行为**

Run: `cargo test vision::worker_tests; cargo check`

Expected: PASS。

- [ ] **Step 6: 提交 Worker**

```bash
git add src-tauri/src/vision/worker.rs src-tauri/src/vision/worker_tests.rs src-tauri/src/vision/runtime.rs src-tauri/src/lib.rs
git commit -m "feat: process vision frames in latest-frame worker"
```

## 任务 6：拆出追踪、匹配与融合算法

**Files:**
- Create: `src-tauri/src/vision/tracking.rs`
- Create: `src-tauri/src/vision/matching.rs`
- Create: `src-tauri/src/vision/alert.rs`
- Create: `src-tauri/src/vision/recognition_tests.rs`

- [ ] **Step 1: 写入 Track/Fusion/多样本匹配失败测试**

```rust
#[test]
fn same_face_and_body_track_emit_one_upgraded_alert() { /* assert one alert */ }

#[test]
fn prototype_and_top_k_agree_before_identity_is_confirmed() { /* assert confirmed */ }

#[test]
fn low_body_score_stays_local_only() { /* assert no LAN push */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::recognition_tests`

Expected: FAIL。

- [ ] **Step 3: 实现轻量 Tracking**

以 IoU、中心点和人体外观关联 Track，TTL 2.5 秒。只存内存短状态，不存轨迹；被跳过采样帧不累计 miss。

- [ ] **Step 4: 实现评分、融合和分级告警**

保留 raw similarity，按 EmbeddingSpace 标准化为 match score；使用质量加权 prototype + Top-K 一致性，不能取最高样本分。输出 `confirmed_face`、`confirmed_fusion`、`probable_body`、`unknown`；仅强人脸/融合自动推送，严格人体为“人体疑似”且低分本地显示。

- [ ] **Step 5: 验证算法测试**

Run: `cargo test vision::recognition_tests`

Expected: PASS。

- [ ] **Step 6: 提交识别算法**

```bash
git add src-tauri/src/vision/tracking.rs src-tauri/src/vision/matching.rs src-tauri/src/vision/alert.rs src-tauri/src/vision/recognition_tests.rs
git commit -m "feat: add tracked multimodal vision matching"
```

## 任务 7：实现 Profile 激活、重算与 Benchmark 生命周期

**Files:**
- Create: `src-tauri/src/vision/activation.rs`
- Create: `src-tauri/src/vision/activation_tests.rs`
- Modify: `src-tauri/src/vision/runtime.rs`
- Modify: `src-tauri/src/vision/storage.rs`

- [ ] **Step 1: 写入 Activation 事务失败测试**

```rust
#[test]
fn cancel_before_switch_cancels_only_its_child_jobs() { /* assert terminal jobs */ }

#[test]
fn failed_warmup_keeps_previous_profile_active() { /* assert LKG */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::activation_tests`

Expected: FAIL。

- [ ] **Step 3: 实现状态机与任务恢复**

实现 Validation -> Smoke -> Benchmark -> 可选确认 -> Rebuild -> Warmup -> Switching -> Active；`Switching` 是不可取消提交点。低优先级 Benchmark/重算必须可被实时识别暂停，重启从 cursor 恢复。

- [ ] **Step 4: 实现人工确认与原子提交命令**

`confirm_vision_profile_activation` 只能从 `AwaitingUserConfirmation` 转入重算；取消用一个 `BEGIN IMMEDIATE` 同时标记 Activation 与其子任务。切换时原子交换运行时 Arc、FeatureStore 快照和 DB active/LKG 标志。

- [ ] **Step 5: 验证状态机**

Run: `cargo test vision::activation_tests`

Expected: PASS。

- [ ] **Step 6: 提交 Activation**

```bash
git add src-tauri/src/vision/activation.rs src-tauri/src/vision/activation_tests.rs src-tauri/src/vision/runtime.rs src-tauri/src/vision/storage.rs
git commit -m "feat: add recoverable vision profile activation"
```

## 任务 8：迁移人员库、参考图和隐私导入导出

**Files:**
- Modify: `src-tauri/src/vision/storage.rs`
- Modify: `src-tauri/src/vision/matching.rs`
- Create: `src-tauri/src/vision/people_tests.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写入参考图约束失败测试**

```rust
#[test]
fn rejects_multi_person_reference_image() { /* detected_subject_count > 1 */ }

#[test]
fn deleting_person_removes_photos_and_embeddings_but_keeps_alert_name_snapshot() { /* assert */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::people_tests`

Expected: FAIL。

- [ ] **Step 3: 实现参考图和特征空间迁移**

支持 3 至 30 张参考图，自动标记 face/body/both，质量严重不合格拒绝、边缘质量低权重保留。普通服装人体样本 24 小时满权、后续衰减、7 天过期；保留旧 EmbeddingSpace，不能混算。

- [ ] **Step 4: 实现安全导入导出与本地快照**

人员库导出使用 Argon2id + AES-GCM；原始参考图仅保存应用私有目录。告警截图仅本地保留 7 天/LRU，默认不跨设备发送。

- [ ] **Step 5: 验证人员库测试**

Run: `cargo test vision::people_tests`

Expected: PASS。

- [ ] **Step 6: 提交人员库迁移**

```bash
git add src-tauri/src/vision/storage.rs src-tauri/src/vision/matching.rs src-tauri/src/vision/people_tests.rs src-tauri/src/lib.rs
git commit -m "feat: migrate vision people library"
```

## 任务 9：迁移局域网策略与远程命令去重

**Files:**
- Create: `src-tauri/src/vision/protocol.rs`
- Create: `src-tauri/src/vision/protocol_tests.rs`
- Modify: `src-tauri/src/protocol.rs`
- Modify: `src-tauri/src/network.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写入协议兼容与幂等失败测试**

```rust
#[test]
fn duplicate_command_returns_stored_result_without_second_job() { /* assert one job */ }

#[test]
fn old_face_monitor_policy_only_updates_legacy_fields() { /* assert partial conversion */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::protocol_tests`

Expected: FAIL。

- [ ] **Step 3: 实现版本化 VisionPolicyFrame**

包含命令 ID、nonce、签发/过期时间、profile 版本、阈值/开关/锁定字段和 revision。新端接收旧 `FaceMonitorPolicyFrame` 时只生成存在字段的局部更新；旧端忽略未知 Frame，不断开 TCP。

- [ ] **Step 4: 实现接收事务和异步调度**

同一 `BEGIN IMMEDIATE` 中校验过期、插入命令收据、比较最高 revision、持久化执行意图；提交后才下载/安装/Benchmark/重算。不得把硬件或模型状态写入 mDNS/普通在线广播。

- [ ] **Step 5: 验证协议测试和现有网络测试**

Run: `cargo test vision::protocol_tests network::tests`

Expected: PASS。

- [ ] **Step 6: 提交 LAN 策略层**

```bash
git add src-tauri/src/vision/protocol.rs src-tauri/src/vision/protocol_tests.rs src-tauri/src/protocol.rs src-tauri/src/network.rs src-tauri/src/lib.rs
git commit -m "feat: add idempotent vision policy protocol"
```

## 任务 10：替换前端帧采样与 Tauri API 契约

**Files:**
- Create: `src/types/vision.ts`
- Create: `src/services/visionFrameTransport.ts`
- Modify: `src/services/cameraMediaCoordinator.ts`
- Modify: `src/services/tauri-api.ts`
- Modify: `src/types/face-monitor.ts`
- Create: `scripts/test-vision-frame-transport.mjs`
- Create: `scripts/test-vision-runtime.mjs`

- [ ] **Step 1: 写入 Envelope 与切流失败测试**

```js
assert.equal(readEnvelope(encodeFrame(frame)).streamGeneration, 2);
assert.notEqual(first.streamId, rebuilt.streamId);
```

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-vision-frame-transport.mjs`

Expected: FAIL。

- [ ] **Step 3: 实现前端 Raw Frame 传输**

每次重新取得视频流生成新 UUID 和 generation；采样优先 `requestVideoFrameCallback`，降级才使用 timer。图像只从 Canvas 读取一次为 RGBA，禁止编码 JPEG/Base64；`cameraMediaCoordinator` 继续负责 call/preview/monitor Lease。

- [ ] **Step 4: 接入新 API 与兼容映射**

新增 Runtime/Profile/Activation/People 命令封装；旧 `submitFaceMonitorFrame` 在过渡期只调用新的 raw API 适配器，确保现有 `App.vue` 尚未迁移时告警不丢。

- [ ] **Step 5: 验证前端契约和构建**

Run: `node scripts/test-vision-frame-transport.mjs; node scripts/test-vision-runtime.mjs; npm run build`

Expected: PASS。

- [ ] **Step 6: 提交前端采集层**

```bash
git add src/types src/services scripts package.json
git commit -m "feat: stream raw frames to vision runtime"
```

## 任务 11：接入视觉识别工作区与本地设置

**Files:**
- Create: `src/components/VisionModelCenter.vue`
- Create: `src/components/VisionPeoplePanel.vue`
- Create: `src/components/VisionRuntimeStatus.vue`
- Modify: `src/App.vue`
- Modify: `src/i18n.ts`
- Create: `scripts/test-vision-ui.mjs`

- [ ] **Step 1: 写入 UI 契约失败测试**

```js
assert.match(appSource, /VisionModelCenter/);
assert.match(i18nSource, /vision\.profile\.balanced/);
```

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-vision-ui.mjs`

Expected: FAIL。

- [ ] **Step 3: 创建“视觉识别”主工作区**

左侧主导航新增视觉识别入口，不放到仅设置页。默认显示低资源/均衡 Profile，实验性高准确度明确标识；高级区才显示组件、EmbeddingSpace、Benchmark 与下载镜像细节。

- [ ] **Step 4: 接入人员库、运行状态与可控告警**

人员库显示样本数、质量、过期和重算状态；侧栏/托盘显示 Running、Paused、Starved、Failed 并提供暂停/恢复。离开设置页或最小化到托盘后监控持续运行。

- [ ] **Step 5: 完成中英文本地化**

所有新增错误码映射、Profile 层级、兼容原因、任务状态和隐私提示均提供中文/英文；不得把中文错误原文直接写入 Rust 状态。

- [ ] **Step 6: 验证 UI**

Run: `node scripts/test-vision-ui.mjs; npm run build`

Expected: PASS。

- [ ] **Step 7: 提交视觉工作区**

```bash
git add src/App.vue src/components src/i18n.ts scripts/test-vision-ui.mjs
git commit -m "feat: add vision recognition workspace"
```

## 任务 12：切换告警、桌宠、通话并完成兼容清理

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/face_monitor.rs`
- Modify: `src-tauri/src/storage.rs`
- Modify: `src/App.vue`
- Modify: `src/stores/desktopPet.ts`
- Modify: `src/utils/alertCredibility.ts`
- Create: `scripts/test-vision-compatibility.mjs`

- [ ] **Step 1: 写入端到端兼容失败测试**

```js
assert.equal(await sendRecognitionAlert("confirmed_fusion"), "sent");
assert.equal(await startVideoCallWithMonitoring(), "monitoring-continues");
```

- [ ] **Step 2: 运行失败测试**

Run: `node scripts/test-vision-compatibility.mjs`

Expected: FAIL。

- [ ] **Step 3: 将新告警映射到既有桌宠与排行榜**

手动/自动告警语义保持不变；视觉自动告警必须显示识别类型、置信度与个性化文案，可反馈真实/虚假。原有历史告警、信誉度、外部推送和桌宠待处理角标继续可用。

- [ ] **Step 4: 验证通话共享摄像头**

视频通话关闭“发送视频”时，只停止远端轨道，monitor Lease 仍在则本地检测不中断；通话期间按策略降低采样频率，不关闭运行时。通话结束显式关闭仅属于通话的克隆轨道和 `srcObject`。

- [ ] **Step 5: 去除旧同步路径的主动调用**

确认新服务已覆盖设置、人员、策略、告警和历史查询后，删除 `App.vue` 对 JPEG `submit_face_monitor_frame` 的主动调用；保留旧协议解析到一个兼容发布周期，禁止删除旧数据库历史读取。

- [ ] **Step 6: 执行完整验证**

Run: `cargo test; cargo check; npm run build; node scripts/test-vision-compatibility.mjs; node scripts/test-face-recognition-alert.mjs`

Expected: 全部 PASS；在普通工作笔记本上均衡 Profile 以 720p、3 FPS 运行，P95 单帧处理不超过 300ms，视觉附加内存目标不超过 300MB。

- [ ] **Step 7: 提交切换与兼容层**

```bash
git add src-tauri/src src/App.vue src/stores src/utils scripts
git commit -m "feat: switch alerts to pluggable vision runtime"
```

## 任务 13：性能、隐私和发布验收

**Files:**
- Modify: `src-tauri/src/vision/runtime.rs`
- Modify: `src-tauri/src/debug_log.rs`
- Modify: `src/App.vue`
- Modify: `docs/releases/vNEXT.md`

- [ ] **Step 1: 写入资源保护失败测试**

```rust
#[test]
fn overload_adapts_sampling_before_switching_model() { /* Normal -> Degraded */ }
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test vision::runtime_tests::overload_adapts_sampling_before_switching_model`

Expected: FAIL。

- [ ] **Step 3: 实现诊断与受控日志**

Debug 页展示活动 Profile、帧接收/丢弃、推理 P50/P95、内存估计、缓存用量、Worker 队列和降级原因。日志只记录哈希、尺寸、分数区间、错误码和耗时，不能记录原始图像、Embedding 或可识别人脸信息。

- [ ] **Step 4: 执行手动验收矩阵**

验证冷启动 UI 先显示、模型异步预热；低资源与均衡 Profile；模型失败 LKG/基线回滚；重启恢复；切换摄像头流；通话与预览并存；远程策略重复投递；人员删除；缓存清理；中英文切换。

- [ ] **Step 5: 发布前验证与提交**

Run: `cargo fmt --check; cargo test; cargo check; npm run build; git diff --check`

Expected: PASS。

```bash
git add src-tauri/src/vision src-tauri/src/debug_log.rs src/App.vue docs/releases/vNEXT.md
git commit -m "perf: harden pluggable vision runtime"
```

## 发布与回滚规则

- 数据库 V5 迁移前必须有在线备份；迁移失败继续使用旧 schema，不能半迁移启动。
- 发布先灰度到一台 Windows 普通工作笔记本，验证 P95 和内存目标，再构建正式安装包/绿色版。
- 新 Profile 或新运行时失败时自动回退到 LKG，LKG 不可用再回退内置 baseline；运行时回退不删除参考图和旧 Embedding。
- 旧 `FaceMonitorPolicyFrame`、旧人员/告警数据至少保留一个兼容发布周期；根据遥测/Debug 日志确认无旧调用后再安排单独清理任务。
