# 摄像头人脸识别与自动告警 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 LanChat 的摄像头监控把检测到的人脸与本机录入的人员参考照片做 SFace 特征比对，命中后自动产生具名告警，并驱动桌宠 Alert 与外部群推送，且与狼来了普通告警区分；同时**彻底取消匿名"检测到人脸"告警**，不再产生相关事件与数据。

**Architecture:** 全部识别逻辑在 Rust 后端完成，复用现有 ort 推理与模型清单校验链路：YuNet 输出从"取最强分数"升级为多锚框解码 + NMS；新增 SFace 识别模型做 128 维特征提取；特征随人员记录存入 SQLite；命中走现有 `accept_match` 门限生成 `camera_face` 具名告警。前端只补类型字段、状态展示与桌宠联动。

**Tech Stack:** Tauri 2 / Rust（ort 2.0.0-rc.12、ndarray、image、rusqlite）/ Vue 3 + TypeScript / OpenCV Zoo YuNet + SFace（MIT ONNX）

**规格文档:** `docs/superpowers/specs/2026-08-13-camera-face-recognition-alert-design.md`

## 关键背景（实现者必读）

- 现有运行时：`src-tauri/src/face_monitor.rs`（YuNet 存在检测、`accept_match` 门限、清单校验）。
- 命令与告警发布：`src-tauri/src/lib.rs` 中 `submit_face_monitor_frame`（约 L585）、`publish_camera_face_presence`（约 L690，匿名告警已取消，在 Task 6 删除）、`create_local_face_person`（约 L627）、`send_external_push_alert`（约 L356，只接受 `QuickAlertFrame`，**不要改动它**）。
- 存储：`src-tauri/src/storage.rs` 的 `face_people`、`camera_face_alerts` 表与 `ensure_column` 迁移机制。
- 前端：`src/App.vue`（约 7992 行，`initializeFaceMonitor` ≈ L5407、`syncDesktopPetRuntime` ≈ L4343、`upsertCameraFaceAlert` ≈ L5334）、`src/types/face-monitor.ts`、`src/services/tauri-api.ts`。
- YuNet ONNX 输出（已用 onnx 元数据核实）：`cls_8/cls_16/cls_32`、`obj_8/obj_16/obj_32`、`bbox_8/bbox_16/bbox_32`、`kps_8/kps_16/kps_32`，共 12 个输出；输入 `input`，640×640。
- **YuNet 解码算法**（来自 OpenCV `face_detect.cpp`，输入 640×640 时 padW=padH=640）：对每个 stride s ∈ [8,16,32]，cols=rows=640/s，idx=r*cols+c；score=sqrt(clamp01(cls)*clamp01(obj))；cx=(c+bbox[idx*4+0])*s，cy=(r+bbox[idx*4+1])*s，w=exp(bbox[idx*4+2])*s，h=exp(bbox[idx*4+3])*s；5 个关键点 `(kps[idx*10+2n]+c)*s, (kps[idx*10+2n+1]+r)*s`，顺序为右眼、左眼、鼻尖、右嘴角、左嘴角。随后按分数降序做 NMS（IoU 阈值 0.35，topK=5）。
- **SFace 预处理**：112×112、BGR、像素 0..255、不缩放不减均值（与 OpenCV Zoo demo 的 `blobFromImage(img, 1.0, (112,112))` 一致）；输出 128 维，L2 归一化后余弦比对；匹配判定用余弦相似度。
- **人脸对齐模板**（arcface 112×112，顺序：左眼、右眼、鼻尖、左嘴角、右嘴角）：`(38.2946,51.6963),(73.5318,51.5014),(56.0252,71.7366),(41.5493,92.3655),(70.7299,92.2041)`。注意 YuNet 的"右眼"是被识者右眼、位于画面左侧，对应模板左眼坐标。
- Rust 测试命令：`cargo test --manifest-path src-tauri/Cargo.toml <过滤名>`；前端脚本测试：`node scripts/<name>.mjs`；类型检查：`npx vue-tsc --noEmit`。
- 提交信息用中文，遵循仓库现有风格（`feat:`/`fix:`/`docs:` 前缀可参照最近提交）。

---

### Task 1: 下载并登记 SFace 识别模型资源

**Files:**
- Create: `src-tauri/resources/object-models/face-recognizer.onnx`
- Modify: `src-tauri/resources/object-models/manifest.json`
- Modify: `src-tauri/resources/object-models/README.md`

- [ ] **Step 1: 下载 SFace ONNX**

```powershell
Invoke-WebRequest -Uri "https://github.com/opencv/opencv_zoo/raw/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx" -OutFile "src-tauri\resources\object-models\face-recognizer.onnx"
```

下载失败时改用镜像 `https://raw.githubusercontent.com/opencv/opencv_zoo/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx`。预期大小约 49MB。

- [ ] **Step 2: 计算 SHA-256 并升级 manifest 到 schemaVersion 2**

```powershell
(Get-FileHash "src-tauri\resources\object-models\face-recognizer.onnx" -Algorithm SHA256).Hash.ToLower()
```

把 `manifest.json` 改为（detector 条目保持原值不动，`modelVersion` 换新版本号，sha256 填实际值）：

```json
{
  "schemaVersion": 2,
  "modelVersion": "opencv-zoo-yunet-sface-2026.08",
  "detector": {
    "file": "presence-detector.onnx",
    "sha256": "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4"
  },
  "recognizer": {
    "file": "face-recognizer.onnx",
    "sha256": "<上一步计算的哈希>"
  }
}
```

- [ ] **Step 3: 更新 README**

在 `README.md` 模型列表追加：`face-recognizer.onnx`：OpenCV Zoo SFace 人脸识别模型，MIT，来源 https://github.com/opencv/opencv_zoo/tree/main/models/face_recognition_sface 。并把"此模型只判断当前画面中是否检测到人脸，不与本地照片进行相似度或身份比对"一句改为说明：存在检测仍不比对身份；识别模型仅与本机录入人员的本地特征比对，特征不出本机。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/resources/object-models
git commit -m "feat: 内置 SFace 人脸识别模型资源并升级模型清单"
```

---

### Task 2: manifest v2 解析与识别模型会话加载

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`
- Test: `src-tauri/src/face_monitor.rs`（`mod tests`）

- [ ] **Step 1: 写失败测试**

在 `mod tests` 追加：

```rust
#[test]
fn model_manifest_v2_includes_recognizer_asset() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("detector.onnx"), b"detector").unwrap();
    fs::write(temp.path().join("recognizer.onnx"), b"recognizer").unwrap();
    let detector = hex::encode(Sha256::digest(b"detector"));
    let recognizer = hex::encode(Sha256::digest(b"recognizer"));
    fs::write(temp.path().join("manifest.json"), format!(
        r#"{{"schemaVersion":2,"modelVersion":"test-2","detector":{{"file":"detector.onnx","sha256":"{detector}"}},"recognizer":{{"file":"recognizer.onnx","sha256":"{recognizer}"}}}}"#
    )).unwrap();
    let state = model_state_from_dir(temp.path()).unwrap();
    assert!(state.ready);
    assert!(state.recognizer_path.is_some());
}

#[test]
fn model_manifest_v1_stays_presence_only() {
    // 复用现有 v1 tempdir 测试结构，断言 state.recognizer_path.is_none()
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml model_manifest`
Expected: 编译失败（`recognizer_path` 不存在）或断言失败。

- [ ] **Step 3: 实现**

`face_monitor.rs` 修改要点：
1. `FaceModelManifest` 增加 `#[serde(default)] recognizer: Option<FaceModelAsset>`；`schema_version` 校验改为 `![1, 2].contains(&manifest.schema_version)` 时报错。
2. `FaceModelState` 增加 `recognizer_path: Option<PathBuf>`；`model_state_from_dir` 中当 `recognizer` 存在时 `validate_model_asset(dir, "识别", &asset)`。
3. `FaceMonitorRuntime` 增加 `recognizer: Option<Mutex<ort::session::Session>>`，构造时按 `recognizer_path` 加载（失败时记入 `model_state.error`，检测器仍可用）。
4. `FaceMonitorStatus` 增加 `pub recognizer_ready: bool`（serde camelCase 自动变为 `recognizerReady`），`status()` 返回 `cfg!(target_os = "windows") && self.recognizer.is_some()`。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: 全部 PASS（含原有 v1 清单、bundled 检测器测试）。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/face_monitor.rs
git commit -m "feat: 模型清单支持识别模型并加载 SFace 会话"
```

---

### Task 3: YuNet 多人脸锚框解码与 NMS

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`
- Test: `src-tauri/src/face_monitor.rs`（`mod tests`）

- [ ] **Step 1: 写失败测试**

解码逻辑做成纯函数，方便用合成数据测试：

```rust
#[derive(Debug, Clone)]
pub struct DetectedFace {
    pub x1: f32, pub y1: f32, pub w: f32, pub h: f32,
    pub landmarks: [(f32, f32); 5], // 右眼 左眼 鼻尖 右嘴角 左嘴角
    pub score: f32,
}

// 输入：stride→(cls, obj, bbox, kps) 切片，输入边长 size
fn decode_faces(strides: [(f32, &[f32], &[f32], &[f32], &[f32]); 3], size: f32, min_score: f32) -> Vec<DetectedFace>
fn nms_faces(faces: Vec<DetectedFace>, iou_threshold: f32, top_k: usize) -> Vec<DetectedFace>
```

测试用例（合成张量，stride 用 640 的小网格即可，把 size 设为 cols*stride）：
1. 单个锚框 score=0.9 → 解码出 1 张脸，bbox/关键点数值按公式手算比对；
2. 低分锚框（<0.6）被过滤；
3. 两个重叠框（IoU>0.35）NMS 后只剩高分框；两个分离框都保留；
4. top_k 生效。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml decode_faces nms_faces`（分别跑）
Expected: 编译失败或 FAIL。

- [ ] **Step 3: 实现解码与 NMS**

按"关键背景"中的公式实现：分数 `sqrt(clamp01(cls)*clamp01(obj))`；bbox `cx=(c+b0)*s, cy=(r+b1)*s, w=exp(b2)*s, h=exp(b3)*s`，x1=cx-w/2, y1=cy-h/2；关键点 `(kps+坐标)*s`。NMS 用标准贪心：按分数降序，IoU（交集/并集）>0.35 抑制，保留至多 top_k=5。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: 全部 PASS。

- [ ] **Step 5: 改造 detect_presence 使用解码结果**

把 `detect_presence` 从"取最强分数"改为：按输出名（`session` 的 outputs 元数据）定位 `cls_8..kps_32` 12 个张量（不要依赖固定下标；找不到名字时回退按下标 0..=5 取 cls/obj 做存在判断以保持兼容），调用 `decode_faces(..., 0.60)` + `nms_faces`；返回值升级为：

```rust
pub struct PresenceDetection { pub confidence: u8, pub detected_faces: u8, pub faces: Vec<DetectedFace> }
```

`faces` 为解码+NMS 结果（坐标在 640×640 输入空间），`detected_faces = faces.len().min(255)`，`confidence` 取最高分 ×100。现有两个调用点和测试保持通过。

- [ ] **Step 6: 运行全部检测测试并运行 bundled 模型冒烟**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: PASS（`bundled_onnx_detector_can_be_opened_by_onnx_runtime` 仍过）。

- [ ] **Step 7: 提交**

```powershell
git add src-tauri/src/face_monitor.rs
git commit -m "feat: YuNet 多人脸锚框解码与 NMS"
```

---

### Task 4: 人脸对齐与 SFace 特征提取

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`
- Test: `src-tauri/src/face_monitor.rs`（`mod tests`）

- [ ] **Step 1: 写失败测试**

```rust
fn align_face_112(image: &image::RgbImage, landmarks: [(f32, f32); 5]) -> image::RgbImage
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32
```

测试：
1. `align_face_112` 用任意 5 个非退化点（例如把模板点放大 2 倍平移）→ 输出尺寸恒为 112×112；
2. `cosine_similarity` 相同向量=1.0，正交向量≈0，方向相反≈-1（误差 1e-4）；
3. L2 归一化后范数≈1（测试 `normalize_embedding`）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml align_face cosine normalize_embedding`
Expected: FAIL。

- [ ] **Step 3: 实现**

1. **相似变换（最小二乘闭式解）**：设变换 `x' = a*x - b*y + tx, y' = b*x + a*y + ty`，令 `d = Σ(x²+y²)`（相对均值的中心化坐标）：
   `a = Σ(dx*dx' + dy*dy') / d`，`b = Σ(dy*dx' - dx*dy') / d`，
   `tx = mx' - a*mx + b*my`，`ty = my' - b*mx - a*my`。
   源点为 YuNet 关键点（原图坐标空间），目标点为 arcface 模板；配对：YuNet 右眼↔模板左眼(38.2946,51.6963)、YuNet 左眼↔模板右眼(73.5318,51.5014)、鼻尖↔(56.0252,71.7366)、YuNet 右嘴角↔模板右嘴(70.7299,92.2041)、YuNet 左嘴角↔模板左嘴(41.5493,92.3655)。
2. **反向采样**：对输出 112×112 每像素求逆变换 `inv = 1/(a²+b²)`：`x = (a*(u-tx) + b*(v-ty))*inv`，`y = (a*(v-ty) - b*(u-tx))*inv`，双线性采样，越界取黑。
3. **`extract_embedding`**（`FaceMonitorRuntime` 方法，供录入与识别共用）：

```rust
pub fn extract_embedding(&self, image: &image::RgbImage, landmarks: [(f32, f32); 5]) -> Result<[f32; 128], String> {
    // recognizer 会话不存在 → Err("识别模型未就绪")
    // aligned = align_face_112(...)；构造 NCHW 1×3×112×112，BGR 顺序，像素 0..255 f32
    // 推理后取 128 维输出，L2 归一化；维数不为 128 时报错
}
```

4. **照片级入口**：`pub fn embedding_from_photo_bytes(&self, bytes: &[u8]) -> Result<[f32; 128], String>`：解码图片→resize 到 640×640 跑检测解码→取最高分脸→把关键点从 640 空间换算回原图坐标（乘 orig/640）→`extract_embedding`；无脸时 `Err("参考照片中未检测到人脸")`。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: 全部 PASS。

- [ ] **Step 5: 用真实模型做冒烟验证（人工核对）**

临时在 tests 里加（或用 `cargo test -- --nocapture` 打印）：加载仓库内两个模型，用 `image` 生成纯噪声图跑 `embedding_from_photo_bytes` 应返回"未检测到人脸"错误；确认 SFace 会话可打开、输出 128 维。验证后删掉临时打印代码。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/face_monitor.rs
git commit -m "feat: 人脸关键点对齐与 SFace 特征提取"
```

---

### Task 5: SQLite 特征列迁移与读写

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Test: `src-tauri/src/storage.rs`（若无 tests mod 则新建；参考其他模块测试风格）

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn face_people_store_embedding_and_model_version() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(temp.path().join("lanchat.db")).unwrap();
    let mut frame = /* 构造 FacePersonPolicyFrame，person_id "p1" */;
    storage.upsert_face_person(&frame).unwrap();
    storage.update_face_person_embedding("p1", Some(vec![1u8, 2, 3]), Some("v1")).unwrap();
    let person = storage.list_face_people().unwrap().pop().unwrap();
    assert_eq!(person.embedding.as_deref(), Some([1u8, 2, 3].as_slice()));
    assert_eq!(person.embedding_model_version.as_deref(), Some("v1"));
}
```

（`Storage::open` 的实际构造方法名以现有代码为准，先读 `storage.rs` 确认。）

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_people_store_embedding`
Expected: 编译失败（字段/方法不存在）。

- [ ] **Step 3: 实现**

1. `FacePersonRecord` 增加 `pub embedding: Option<Vec<u8>>`、`pub embedding_model_version: Option<String>`。
2. 建表块之后用现有 `ensure_column(&conn, "face_people", "embedding", "BLOB")` 和 `ensure_column(&conn, "face_people", "embedding_model_version", "TEXT")` 迁移。
3. `face_person_from_row`、`read_face_person`、`list_face_people` 的 SELECT 增加两列。
4. 新增 `update_face_person_embedding(&self, person_id: &str, embedding: Option<Vec<u8>>, version: Option<String>) -> Result<(), String>`。
5. embedding 不参与 `FacePersonPolicyFrame` 网络同步；`upsert_face_person` 收到远端帧时保留本地已有特征列不变（INSERT 时写 NULL，ON CONFLICT 时不覆盖这两列——把它们从 DO UPDATE 的 SET 列表中排除）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_people`
Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/src/storage.rs
git commit -m "feat: face_people 增加人脸特征列与迁移"
```

---

### Task 6: 移除匿名人脸出现告警链路

**Files:**
- Modify: `src-tauri/src/lib.rs`

需求变更：匿名 `camera_face_presence` 告警不再产生任何事件与数据。本任务先拆除旧链路，Task 7 再接入识别链路。

- [ ] **Step 1: 删除匿名告警发布**

1. 删除 `publish_camera_face_presence` 函数（约 L690）。
2. `submit_face_monitor_frame` 中删除对它的调用；检测命中后暂改为 `Ok(None)`（Task 7 会替换为识别逻辑）。
3. `storage.rs` 的 `upsert_camera_face_alert` 对 `camera_face_presence` 的兼容校验**保留**（历史广播帧与旧版本客户端兼容），无需改动。

- [ ] **Step 2: 编译与回归**

Run: `cargo check --manifest-path src-tauri/Cargo.toml` 与 `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: 无编译错误，测试 PASS。

- [ ] **Step 3: 提交**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat: 取消匿名人脸出现告警，不再产生相关事件与数据"
```

---

### Task 7: 识别匹配与具名告警发布

**Files:**
- Modify: `src-tauri/src/face_monitor.rs`（匹配编排）
- Modify: `src-tauri/src/lib.rs`（`submit_face_monitor_frame`、新增 `publish_camera_face_alert`）
- Test: `src-tauri/src/face_monitor.rs`

- [ ] **Step 1: 写失败测试（纯匹配逻辑）**

在 `face_monitor.rs` 增加纯函数并测试：

```rust
pub struct PersonTemplate { pub person_id: String, pub display_name: String, pub embedding: [f32; 128] }
pub struct FaceMatch { pub person_id: String, pub display_name: String, pub confidence: u8 }

fn best_match(embedding: &[f32; 128], people: &[PersonTemplate]) -> Option<FaceMatch>
```

测试：相同向量命中且 confidence=100；相似度 0.5 → confidence=50；负相似度 clamp 到 0；空人员列表返回 None。

- [ ] **Step 2: 运行确认失败后实现**

`best_match`：对每个人员算余弦相似度取最大者，`confidence = (sim * 100.0).round().clamp(0.0, 100.0) as u8`，返回最高分人员（不设内置下限，由 `accept_match` 的 `min_confidence` 把关）。

- [ ] **Step 3: 新增识别编排方法（含特征缓存）**

`face_monitor.rs`：

```rust
pub struct FaceRecognitionFrame {
    pub detection_confidence: u8,
    pub matches: Vec<FaceMatch>, // 每张人脸至多一个命中（去重同一 person 只留最高分）
}

pub fn recognize_frame(&self, bytes: &[u8], people: &[PersonTemplate]) -> Result<Option<FaceRecognitionFrame>, String>
```

内部：busy 门闩（同现有）→ `detect_presence`（已含 faces）→ 每张脸关键点换算回原图坐标→ `extract_embedding`（单人特征提取失败只跳过该脸，不整帧失败）→ `best_match`。特征缓存放在调用方（lib.rs，见 Step 4）。

- [ ] **Step 4: lib.rs 接入识别模式**

1. 新增辅助 `fn load_recognition_templates(state: &AppState) -> Result<Vec<PersonTemplate>, String>`：读 `list_face_people()`，过滤 `enabled && deleted_at.is_none() && photo_url 可读`；对每个人：若 `embedding` 存在且 `embedding_model_version == 当前 model_version` 直接用；版本不一致则用照片重新提取并 `update_face_person_embedding`；照片不可读或提取失败则 `update_face_person_embedding(id, None, None)` 并跳过。用 `Mutex<HashMap<String, (String version, [f32;128])>>` 挂在 `AppState`（或 runtime 内）做进程内缓存，避免每帧读库——仅当 DB 中特征缺失时落库。
2. `submit_face_monitor_frame` 改造：

```text
识别模型就绪 && templates 非空：
    recognize_frame → 对每个 match：
        accept_match(&match.person_id, match.confidence, policy...) 通过
        → publish_camera_face_alert(app, state, &match, policy.version).await（可多个）
    返回第一个生成的记录（前端预览用；无则 None）
否则（识别模型未就绪或无可用人员）：
    返回 Ok(None)，不产生任何告警事件与数据（匿名链路已在 Task 6 移除）
```

3. 新增 `publish_camera_face_alert`：复制 `publish_camera_face_presence` 结构，`CameraFaceAlertFrame` 填 `source_kind: "camera_face"`、`person_id/person_name` 为命中人员、`confidence` 为匹配置信度；落库、广播、`app.emit("camera_face_alert_received", &record)` 均复用。
4. 识别模型未就绪或无人员时不产生告警，错误原因通过 `status()` 暴露，由设置页展示。

- [ ] **Step 5: 运行测试并编译**

Run: `cargo test --manifest-path src-tauri/Cargo.toml face_monitor` 与 `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS / 无错误。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/face_monitor.rs src-tauri/src/lib.rs
git commit -m "feat: 摄像头人脸识别匹配与具名自动告警"
```

---

### Task 8: 人员录入时提取特征

**Files:**
- Modify: `src-tauri/src/lib.rs`（`create_local_face_person`）
- Test: `src-tauri/src/face_monitor.rs`（错误路径已在 Task 4 覆盖；此处做命令级验证）

- [ ] **Step 1: 修改 create_local_face_person**

在现有校验与 `upsert_face_person` 之后：

```rust
let embedding = state.face_monitor.embedding_from_photo_bytes(&bytes)?; // 无脸 → "参考照片中未检测到人脸"
state.storage.update_face_person_embedding(person_id, Some(embedding_bytes(&embedding)), Some(model_version))?;
```

`embedding_bytes`：`[f32;128]` → 小端字节 Vec<u8>（与 Task 5 读侧对应；提供成对的 `embedding_from_bytes` 反序列化辅助，维数不对时报错）。识别模型未就绪（`recognizer_ready == false`）时返回错误"识别模型未安装，暂时无法录入识别人员"。

- [ ] **Step 2: 编译与既有测试**

Run: `cargo check --manifest-path src-tauri/Cargo.toml` 与 `cargo test --manifest-path src-tauri/Cargo.toml face_monitor`
Expected: PASS。

- [ ] **Step 3: 提交**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat: 录入识别人员时提取并保存人脸特征"
```

---

### Task 9: 识别告警的外部推送（独立路径）

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 新增推送渲染与发送（不动 send_external_push_alert）**

```rust
fn render_camera_face_push_text(frame: &CameraFaceAlertFrame) -> String {
    let source_ip = frame.source_address.as_deref().unwrap_or("未知 IP");
    format!("[人脸识别告警] 检测到 {} · 置信度 {}% · 来源：{}（{}） · {}",
        frame.person_name, frame.confidence, frame.source_nickname, source_ip,
        format_alert_time(frame.created_at))
}

async fn send_camera_face_external_push(config: ExternalPushConfig, frame: CameraFaceAlertFrame) -> Result<(), String>
```

发送体与 `send_external_push_alert` 相同的 webhook 校验与钉钉/企业微信 JSON 结构（复制该段逻辑，正文换成 `render_camera_face_push_text`），**不走可信度阈值**。可把 webhook 校验与 HTTP 发送抽成小函数共享，避免整段复制。

- [ ] **Step 2: 在 publish_camera_face_alert 中触发**

```rust
let settings = state.desktop_pet.settings();
if settings.external_push_enabled {
    for config in settings.external_push_configs.into_iter().filter(|c| c.enabled && !c.webhook.trim().is_empty()) {
        let frame = frame.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = send_camera_face_external_push(config, frame).await { eprintln!("{error}"); }
        });
    }
}
```

只在产生端调用（`publish_camera_face_alert` 只在监控设备执行），接收端天然不推送。

- [ ] **Step 3: 渲染函数单测**

```rust
#[test]
fn camera_face_push_text_names_person_and_source() { /* 构造帧，断言文案包含 人员名、置信度、来源、时间 */ }
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml camera_face_push`
Expected: PASS。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat: 人脸识别告警独立外部群推送"
```

---

### Task 10: 前端类型、状态展示与人员特征状态

**Files:**
- Modify: `src/types/face-monitor.ts`
- Modify: `src/services/tauri-api.ts`（如有返回类型变化则无改动）
- Modify: `src/App.vue`（设置页状态区与人员列表）

- [ ] **Step 1: 类型补全**

`face-monitor.ts`：`FaceMonitorRuntimeStatus` 增加 `recognizerReady?: boolean`；`FacePersonPolicy` 增加 `embedding?: number[] | null` 与 `embeddingModelVersion?: string | null`（仅用于前端判定特征状态，Rust 记录已含两字段，serde camelCase 自动映射）。注意 `listFacePeople` 返回 embedding 字节数组可能较大但人员数量少，可接受；若想省流量可让后端把字段序列化为 `has_embedding: bool`——采用简单方案：`FacePersonRecord` 序列化时 `#[serde(serialize_with)]` 把 embedding 转成 `hasEmbedding: bool`（Rust 侧加字段 `has_embedding`，跳过原始字节），前端类型改为 `hasEmbedding?: boolean`。实现时以该方案为准，避免大字节过 IPC。

- [ ] **Step 2: 设置页展示**

在 `App.vue` 摄像头监控设置区（搜索 `摄像头人脸出现告警` 与 `faceMonitorRuntimeStatus` 相关模板）：
1. 状态区追加一行：识别模型未就绪时 `NAlert type="warning"` 显示 `faceMonitorRuntimeStatus.lastError`；无启用人员时前端根据 `facePeople` 过滤启用者数量，显示"尚未录入识别人员，摄像头监控不会产生告警"。
2. 人员列表每行显示特征徽标：`hasEmbedding` 为真 → `NTag type="success" 特征已提取`；否则 `NTag type="warning" 特征不可用`。

- [ ] **Step 3: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: 提交**

```powershell
git add src/types/face-monitor.ts src/App.vue src-tauri/src/storage.rs src-tauri/src/lib.rs
git commit -m "feat: 设置页展示识别模型与人员特征状态"
```

---

### Task 11: 桌宠介入识别告警（与狼来了区分）

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: 新增识别告警桌宠覆盖状态**

在 `syncDesktopPetRuntime` 附近新增：

```ts
const facePetAlert = ref<{ alertId: string; personName: string; confidence: number; sourceNickname: string; sourceAddress: string | null; createdAt: number; until: number } | null>(null);
const activeFacePetAlert = computed(() => (facePetAlert.value && facePetAlert.value.until > nowTick.value) ? facePetAlert.value : null);
```

`upsertCameraFaceAlert` 中：对所有 `CameraFaceAlert` 记录（匿名告警已取消，收到的即识别告警；历史旧记录 personName 为"检测到人脸"，同样可驱动桌宠无害）设置 `facePetAlert.value = { ..., until: Date.now() + 30_000 }`，随后 `nowTick.value = Date.now(); void syncDesktopPetRuntime();`（`nowTick` 已存在并有定时器驱动；若 30 秒后无定时器刷新，可在 `until` 到期时依靠下一次 nowTick 更新自然失效）。

- [ ] **Step 2: 合入运行时快照**

修改 `syncDesktopPetRuntime`（App.vue ≈ L4343）：

```ts
const alert = activePetAlert.value;
const face = !alert ? activeFacePetAlert.value : null;
// latest_alert_id: alert?.alertId ?? face?.alertId ?? null
// latest_sender: alert?.senderNickname ?? (face ? "【人脸识别】" : null)
// latest_sender_address: ... ?? face?.sourceAddress ?? null
// latest_content: alert ? ... : (face ? `检测到 ${face.personName} · 置信度 ${face.confidence}%` : null)
// latest_created_at: ... ?? face?.createdAt ?? null
// flashing: !!alert || !!face
```

`feedbackable`、`pending_count`、`disco` 保持只由狼来了驱动（识别告警反馈在设置页完成，桌宠不显示反馈按钮）。

- [ ] **Step 3: 验证**

Run: `npx vue-tsc --noEmit`；再跑 `node scripts/test-desktop-pet-runtime.mjs` 确认未破坏现有断言。
Expected: 通过。

- [ ] **Step 4: 提交**

```powershell
git add src/App.vue
git commit -m "feat: 人脸识别告警驱动桌宠并与狼来了区分"
```

---

### Task 12: 前端测试脚本与全量回归

**Files:**
- Modify: `scripts/test-face-monitor-settings.mjs`、`scripts/test-face-monitor-media-coordinator.mjs`（按现状增补）
- 可能新增: `scripts/test-face-recognition-alert.mjs` 并登记到 `package.json`

- [ ] **Step 1: 增补脚本断言**

参照现有脚本模式（读源码文本断言关键行为）补充：
1. `lib.rs` 中 `publish_camera_face_presence` 已不存在（匿名告警链路已移除）；
2. `upsertCameraFaceAlert` 设置 `facePetAlert`；
3. `syncDesktopPetRuntime` 中识别覆盖文案包含 `【人脸识别】`；
4. Rust 侧 `render_camera_face_push_text` 存在且不改动 `send_external_push_alert` 签名（文本断言即可）。

- [ ] **Step 2: 全量回归**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
node scripts/test-face-monitor-settings.mjs
node scripts/test-face-monitor-media-coordinator.mjs
node scripts/test-face-recognition-alert.mjs
node scripts/test-desktop-pet-runtime.mjs
npx vue-tsc --noEmit
```

Expected: 全部 PASS。

- [ ] **Step 3: 手工集成验证清单（真实设备）**

1. 录入本人照片（上传与拍照各一次）→ 人员显示"特征已提取"；照片中无脸时提示"参考照片中未检测到人脸"。
2. 开启监控出镜 → 连续命中后产生具名告警"检测到 XX"，桌宠进入 Alert 显示【人脸识别】文案；外部推送（如已配置 webhook）收到 `[人脸识别告警]` 文案。
3. 陌生人出镜 → 无告警。
4. 删除全部人员 → 不再产生任何告警，设置页显示录入提示。
5. 临时将 manifest 中 recognizer 哈希改错并重启 → 状态区显示识别模型错误，不产生告警。

- [ ] **Step 4: 提交**

```powershell
git add scripts package.json
git commit -m "test: 人脸识别告警链路脚本验证与回归"
```

---

## 任务依赖关系

```text
Task 1（模型资源）
  └→ Task 2（清单/会话）→ Task 4（对齐+特征提取）→ Task 8（录入提取）
Task 3（多人脸解码）→ Task 7（识别编排+具名告警）→ Task 9（外部推送）
Task 5（特征列）→ Task 7 / Task 8
Task 6（移除匿名链路）→ Task 7
Task 10、11 → Task 12（回归）
```

Task 3 与 Task 5、Task 6 可并行；Task 2 完成后可并行启动 Task 3 与 Task 4。
