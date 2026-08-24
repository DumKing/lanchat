# LanChat 多模型视觉识别平台实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把当前单一 ONNX 基线模型链路升级为可安装、可切换、可回滚的多模型视觉识别平台，支持 SFace、InsightFace/ArcFace、OSNet、OMZ，并为 FastReID 留出同一接入面。

**Architecture:** 一份 Profile 是一套完整的“人体检测 + 人脸引擎 + 人体 ReID 引擎 + 推理后端 + 融合策略”组合，而不是单个模型文件或性能档位。摄像头帧先经人体检测得到 person crop，人脸与人体两条支路分别生成版本隔离的 Embedding，最后由 IdentityFusion 输出身份判断；切换 Profile 时先构建候选运行时与特征库，完成验证后原子切换。

**Tech Stack:** Rust 2021、Tauri 2、Tokio、rusqlite、ort/ONNX Runtime、OpenVINO Runtime、Ed25519、ZIP、Vue 3、TypeScript、Naive UI。

---

## 1. 问题与边界

### 1.1 当前问题

`scripts/build-vision-model-packages.mjs` 当前把 `src-tauri/resources/object-models` 原样复制给 `office-light`、`office-balanced`、`office-sensitive`，只改 Profile 名称、版本和推荐参数。因此三个 ZIP 的模型权重完全相同，用户切换后不会获得不同的识别能力。

### 1.2 本次必须达成

- Catalog 中的每个可下载项必须对应真实不同的模型组合；不得再以同一组权重复制改名。
- 支持以下模型家族的统一接入：
  - 人脸：OpenCV SFace、InsightFace ArcFace。
  - 人体外观：OSNet、Open Model Zoo（OMZ）Person ReID。
  - 预留：FastReID。
- 支持两个真实推理后端：ONNX Runtime 与 OpenVINO Runtime。后端是运行方式，不等于模型家族。
- 人脸向量、人体向量按模型空间隔离存储；不同模型、不同维度、不同预处理方式的向量绝不直接比较。
- 普通办公笔记本可稳定运行。默认只安装轻量基线，其他模型按需下载。

### 1.3 非目标

- 不把 InsightFace 的受限权重直接放入官方安装包或官方 Catalog；只有通过许可证校验后的权重可作为用户本地导入项。
- 不在本次把 FastReID 作为官方可下载模型发布；先实现标准 Adapter 契约和本地导入路径，后续在许可证与模型验收齐全后再加入官方 Catalog。
- 不在前端实现推理；所有模型解析、校验、会话构建和特征计算都在 Rust 侧完成。

## 2. 目标运行链路

```text
Camera frame
  -> Person Detector
  -> person crop + tracker
  -> FaceEngine (SFace / ArcFace) -> Face Embedding
  -> PersonReIdEngine (OSNet / OMZ / FastReID) -> Body Embedding
  -> IdentityFusion
  -> ConfirmedFace / ConfirmedFusion / ProbableBody / Unknown
```

### 2.1 运行顺序

1. `PersonDetector` 处理原始帧，输出合格人体框和稳定 `track_id`。
2. 每个 `person crop` 先做质量门控：最小尺寸、模糊度、遮挡比例、可见人体比例。
3. `FaceEngine` 在 person crop 内寻找人脸；无脸或质量不足时返回空证据，不阻断人体链路。
4. `PersonReIdEngine` 对 person crop 输出人体向量。
5. `IdentityFusion` 在各自的 FeatureStore 中寻找候选，再按质量、校准分数、Top1/Top2 间隔和短时连续命中规则决定身份。

### 2.2 融合规则

- Face 与 Body 先分别检索候选人和置信度；只在候选 `person_id` 相同或时间轨迹一致时合并。
- 人脸质量达标时，默认权重 `face=0.70`、`body=0.30`；人脸缺失时只允许输出 `ProbableBody`，不得冒充确认身份。
- 每个模型空间维护独立的相似度校准曲线、阈值、Top1/Top2 Margin 和最少连续命中数。
- 同一轨迹使用滑动窗口平滑，默认取最近 5 个有效证据；窗口中至少 3 个一致候选才能输出 `ConfirmedFusion`。
- 所有原始分数、质量分、候选间隔、融合权重随告警审计保存，便于调参和追溯。

## 3. 模型与发布矩阵

| Profile ID | 检测器 | FaceEngine | PersonReIdEngine | 后端 | 交付方式 | 默认 |
| --- | --- | --- | --- | --- | --- | --- |
| `baseline-sface-youtu` | YuNet + YOLOX INT8 | SFace | 当前 YoutuReID INT8 | ONNX Runtime CPU | 安装包内置 | 是 |
| `office-osnet-x025` | YuNet + 轻量 Person Detector | SFace | OSNet x0.25 | ONNX Runtime CPU | 官方签名下载 | 否 |
| `office-omz-retail-0288` / `office-omz-retail-0286` | OMZ person detector | OMZ retail-0095 | OMZ retail-0288 / 0286 | OpenVINO CPU | 官方签名下载 | 否 |
| `office-arcface-buffalo-sc` | YuNet + YOLOX | ArcFace Buffalo-SC | YoutuReID | ONNX Runtime | 官方签名下载，仅非商用学习/研究 | 是 |
| `custom-fastreid` | 包内声明 | 可选 | FastReID | ONNX Runtime 或 OpenVINO | 本地导入，后续启用 | 否 |

说明：官方条目的具体权重、来源、许可证、SHA-256、量化类型、内存与 CPU 预算必须经过发布前 License Gate 和 Smoke Test 后才写入 Catalog。Catalog 不能以“轻量/均衡/灵敏”这类没有模型实体含义的名称代替模型组合。

## 4. 模型包与 Catalog 规范

### 4.1 Manifest V4

保留 V3 只读兼容，新增 V4 作为唯一可安装格式。每个包至少包含：

```json
{
  "schemaVersion": 4,
  "package": { "id": "com.lanchat.vision.office-osnet-x025", "version": "1.0.0" },
  "profile": {
    "id": "office-osnet-x025",
    "displayName": "OSNet 办公均衡",
    "provider": "torchreid",
    "engine": "onnxruntime",
    "supportedBackends": ["cpu"],
    "license": { "spdx": "TO_BE_VERIFIED", "sourceUrl": "..." }
  },
  "pipeline": {
    "personDetector": "person-detector",
    "faceEngine": "sface",
    "personReIdEngine": "osnet-x025",
    "fusionPolicy": "quality-temporal-v1"
  },
  "components": []
}
```

每个组件必须声明 `family`、`adapterId`、`engine`、输入/输出、量化、模型文件摘要和 `embeddingSpaceId` 计算所需语义字段。安装时拒绝缺字段、重复 ID、未声明 Adapter、摘要不匹配和不受支持的后端。

### 4.2 防止“改名副本”

- 发布构建以 `scripts/vision-model-sources.json` 为唯一来源；每个 Profile 指向独立模型目录，不再复制内置目录。
- Catalog 写入每个组件 SHA-256；构建脚本校验除显式 `shared=true` 的检测器外，不同 Profile 的识别/ReID 组件不能拥有相同 SHA-256。
- 生成 `model-diff.json`，列出 Profile 间实际模型文件、模型家族、向量空间、尺寸与摘要差异；CI 将其作为 Release Artifact。
- 若两个 Profile 只有推荐参数不同，构建必须失败并提示“PROFILE_HAS_NO_DISTINCT_MODEL_ASSETS”。

## 5. Rust 模块拆分

### 5.1 新增边界

```text
src-tauri/src/vision/
  backend/{mod.rs, ort.rs, openvino.rs}
  engines/
    face/{mod.rs, sface.rs, arcface.rs}
    person/{mod.rs, youtu.rs, osnet.rs, omz.rs, fastreid.rs}
  detector/{mod.rs, person.rs, face.rs}
  fusion/{mod.rs, calibration.rs, temporal.rs}
  profile/{manifest_v4.rs, compatibility.rs, activation.rs}
```

- `InferenceBackend`：加载会话、准备输入、执行推理、释放资源；`OrtBackend` 和 `OpenVinoBackend` 仅处理后端差异。
- `FaceEngine`：输入 person crop，输出 `FaceObservation` 与可选 Face Embedding。
- `PersonReIdEngine`：输入 person crop，输出 Body Embedding 与质量指标。
- `IdentityFusion`：不认识 ONNX/OpenVINO，不读取模型文件；只消费归一化识别证据。
- `FeatureStore`：键为 `person_id + embedding_space_id + modality`，彻底隔离不同模型产生的特征。

### 5.2 兼容层

现有 `face_monitor.rs` 保留为过渡 Facade：将旧的单链路调用转成 `VisionRuntime.submit_frame`。新模块稳定后再删除旧模型会话字段，避免一次性改动摄像头、告警、桌宠和外部推送。

## 6. 数据迁移与 Profile 切换

1. V5 旧特征登记为 `opencv-sface-128-rgb-v1` 和 `youtu-reid-768-rgb-v1`，不迁移数值。
2. 下载包先在 staging 解压、验签、校验 manifest、文件摘要和 Adapter 可用性。
3. 以新模型构建 Candidate Runtime，执行 3 次预热和固定样本 Smoke Test。
4. 后台从人员照片与可用人体样本重算新空间 Embedding；旧 Runtime 继续服务。
5. 新空间特征达到可用状态后，短暂停止取帧，原子切换 Profile + FeatureStore revision。
6. 切换后 10 分钟监控失败率和 P95 延迟；异常时回滚到 Last Known Good，保留失败原因和审计记录。

人员资料界面需明确显示“人脸样本已生成 X/Y 个模型特征、人体样本已生成 X/Y 个模型特征”，不能把旧 SFace 特征错误标记为 ArcFace 或 OSNet 可用。

## 7. 模型中心 UI

模型中心不再展示伪档位，而是展示真实 Profile：

- 名称、提供方、FaceEngine、PersonReIdEngine、推理后端、量化类型。
- 下载大小、磁盘占用、预计 CPU/内存、最近 P95 推理耗时。
- 许可证、来源、官方签名状态；受限模型只显示“本地导入”。
- 安装、校验、预热、特征重建、切换、回滚的分阶段进度与失败原因。
- 选择某个 Profile 前显示它会新建哪些 EmbeddingSpace，以及哪些人员需要补录人体样本。

默认按硬件能力只推荐 `baseline-sface-youtu`；当 P95 延迟连续超预算时显示建议切回基线，不自动切换未验证模型。

## 8. 分阶段实施任务

### Task 1: 建立 V4 Manifest 与真实模型源配置

**Files:**
- Create: `scripts/vision-model-sources.json`
- Create: `src-tauri/src/vision/profile/manifest_v4.rs`
- Modify: `src-tauri/src/vision/manifest.rs`
- Modify: `scripts/build-vision-model-packages.mjs`
- Test: `src-tauri/src/vision/manifest_tests.rs`
- Test: `scripts/test-vision-package-distinctness.mjs`

- [ ] 先写 V4 缺字段、未知 Adapter、重复权重 Profile 的失败测试。
- [ ] 实现 Manifest V4 解析、V3 只读兼容与 V4 安装校验。
- [ ] 改造构建脚本为按独立源目录组包，并生成 `model-diff.json`。
- [ ] 运行 `cargo test vision::manifest_tests` 与 `node scripts/test-vision-package-distinctness.mjs`。
- [ ] 提交：`feat: define distinct vision model packages`。

### Task 2: 抽象后端与引擎 Adapter

**Files:**
- Create: `src-tauri/src/vision/backend/mod.rs`
- Create: `src-tauri/src/vision/backend/ort.rs`
- Create: `src-tauri/src/vision/backend/openvino.rs`
- Create: `src-tauri/src/vision/engines/face/{mod.rs,sface.rs,arcface.rs}`
- Create: `src-tauri/src/vision/engines/person/{mod.rs,youtu.rs,osnet.rs,omz.rs,fastreid.rs}`
- Modify: `src-tauri/src/vision/worker.rs`
- Test: `src-tauri/src/vision/worker_tests.rs`

- [ ] 为每个 Trait 写假实现，验证 Worker 可按 Adapter ID 装配正确引擎。
- [ ] 先迁移既有 SFace/YoutuReID 到 Adapter，保证基线行为不变。
- [ ] 实现 OSNet 与 OMZ Adapter；OpenVINO 后端只在运行库与模型包同时可用时报告可选。
- [ ] 为 ArcFace/FastReID 实现 Manifest 识别和本地导入验证，不把未通过许可证 Gate 的包加入官方 Catalog。
- [ ] 运行 `cargo test vision::worker_tests` 和各 Adapter 的 Golden Vector 测试。
- [ ] 提交：`feat: add pluggable face and person reid engines`。

### Task 3: 实现 IdentityFusion 与模型空间隔离

**Files:**
- Create: `src-tauri/src/vision/fusion/{mod.rs,calibration.rs,temporal.rs}`
- Modify: `src-tauri/src/vision/types.rs`
- Modify: `src-tauri/src/vision/storage.rs`
- Modify: `src-tauri/src/vision/matching.rs`
- Test: `src-tauri/src/vision/recognition_tests.rs`

- [ ] 写测试：不同 `embedding_space_id` 不可比较；无脸只能 `ProbableBody`；连续命中后才可 `ConfirmedFusion`。
- [ ] 实现质量门控、Top1/Top2 Margin、模型级校准和五帧时序融合。
- [ ] 为每个识别结果保存证据快照，供告警详情与调试页读取。
- [ ] 运行 `cargo test vision::recognition_tests`。
- [ ] 提交：`feat: fuse face and body identity evidence`。

### Task 4: 安装、切换、迁移与回滚

**Files:**
- Modify: `src-tauri/src/vision/model_manager.rs`
- Modify: `src-tauri/src/vision/activation.rs`
- Modify: `src-tauri/src/vision/storage.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/vision/activation_tests.rs`
- Test: `src-tauri/src/vision/storage_tests.rs`

- [ ] 写测试：新 Profile 预热失败保留旧 Runtime；特征重建未完成不可切换；回滚只允许 Last Known Good。
- [ ] 实现候选 Runtime、后台 Embedding 重建、原子激活和失败审计。
- [ ] 对已安装模型做引用计数，清理缓存时不得删除活动或 LKG 模型。
- [ ] 运行 `cargo test vision::activation_tests vision::storage_tests`。
- [ ] 提交：`feat: activate vision profiles atomically`。

### Task 5: 模型中心与人员样本 UI

**Files:**
- Modify: `src/components/VisionModelCenter.vue`
- Modify: `src/components/VisionPeoplePanel.vue`
- Modify: `src/components/VisionRuntimeStatus.vue`
- Modify: `src/types/vision.ts`
- Modify: `src/services/tauri-api.ts`
- Test: `scripts/test-vision-ui.mjs`
- Test: `scripts/test-vision-runtime.mjs`

- [ ] 写 UI 契约测试：展示模型组合、许可证、后端、安装状态和特征迁移进度。
- [ ] 改为真实模型 Profile 列表，并移除“轻量/均衡/灵敏”伪差异文案。
- [ ] 人员页展示各模型空间的样本覆盖率与补录提示。
- [ ] 运行 `node scripts/test-vision-ui.mjs; node scripts/test-vision-runtime.mjs; npm run build`。
- [ ] 提交：`feat: show real vision model profiles`。

### Task 6: 官方模型发布与 CI

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/write-vision-catalog.mjs`
- Modify: `scripts/vision-model-profiles.json`
- Create: `scripts/test-vision-model-catalog.mjs`
- Create: `docs/vision-model-publishing.md`

- [ ] 写测试：Catalog 不得含受限许可证模型；Profile 需具备独特识别或 ReID 权重；所有 SHA 与 ZIP 内容一致。
- [ ] 发布实际独立的 Baseline、OSNet、OMZ 包；InsightFace/FastReID 只发布导入规范，不发布受限权重。
- [ ] 生成、签名、验证 Catalog，上传 ZIP、签名 Catalog 与 `model-diff.json`。
- [ ] 运行 `node scripts/test-vision-model-catalog.mjs` 与完整 Release dry-run。
- [ ] 提交：`feat: publish distinct signed vision models`。

## 9. 验收标准

- 模型中心中任意两个官方 Profile 的 FaceEngine 或 PersonReIdEngine 至少一个不同，且其对应识别/ReID 权重 SHA 不同。
- 选择 OSNet / OMZ 后，诊断页能明确显示实际 Backend、Adapter、模型版本和 EmbeddingSpace。
- 人脸不可见但人体质量达标时，可产生 `ProbableBody`；人脸与人体同时稳定时，产生 `ConfirmedFusion`。
- 切换模型后，旧空间特征不参与新模型检索；特征重建失败时自动回滚且旧告警不中断。
- 普通办公笔记本默认基线 Profile 在 720p、1 FPS 下 P95 推理不超过 500ms；超预算时降低采样并提示，不无限积压帧。
- CI 在发现同权重改名 Profile、许可证不合规、签名失效或模型 Smoke Test 失败时阻断发布。

## 10. 版本策略

当前 `v0.6.0` 已触发正式发布流水线，不应在发布中途替换其模型资产。本计划作为 `v0.6.1` 的后续功能分支实施：先完成真实模型包与运行时适配，再创建 PR 合并到受保护的 `main`，最后发布新的版本标签。
