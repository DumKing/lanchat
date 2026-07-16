# 青蛙桌宠动作素材更新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 生成独立的绿色“生气”报警帧，并用三张同风格微动作替换待机循环中的开心、害羞和轻蹦迪。

**Architecture:** 保持现有 4 x 4 主动作图集不变，新图先进入候选素材目录并通过浏览器预览，再按固定索引重新组装正式图集。Rust 仅调整动作常量命名与待机时间轴，普通报警继续使用三帧循环，蹦迪图集保持独立。

**Tech Stack:** PNG alpha 图像、HTML/CSS/JavaScript 预览、Rust/egui、Node.js 验证脚本、Tauri 2。

---

### Task 1: 固定动作映射测试

**Files:**
- Modify: `scripts/test-native-frog-pet.mjs`
- Modify: `scripts/test-frog-pet-ui-native.mjs`

- [ ] **Step 1: 写入失败断言**

断言主图集仍为 4 x 4；普通报警索引为“惊讶、生气、告警”；待机时间轴包含“抬头、低头、轻晃”，且不再引用“开心、害羞、轻蹦迪”。

- [ ] **Step 2: 运行测试确认失败**

Run: `node scripts/test-native-frog-pet.mjs`

Expected: FAIL，指出新的姿态常量或时间轴尚不存在。

### Task 2: 生成并校验四张透明动作素材

**Files:**
- Create: `public/pet-assets/preview-candidates/33-angry-green.png`
- Create: `public/pet-assets/preview-candidates/34-idle-look-up.png`
- Create: `public/pet-assets/preview-candidates/35-idle-look-down.png`
- Create: `public/pet-assets/preview-candidates/36-idle-sway.png`

- [ ] **Step 1: 以当前待机青蛙为角色参考生成四张素材**

统一约束：正面、圆润比例、3D 软质材质、同一光照、主体居中、完整边缘、无文字和装饰。生气帧保持绿色，只强化眉眼、嘴角和前爪；三个待机帧只做小幅姿态变化。

- [ ] **Step 2: 去除纯色键背景并生成 alpha PNG**

使用 imagegen 内置流程生成纯洋红键背景，再用 `remove_chroma_key.py` 去背，避免绿色青蛙与键色冲突。

- [ ] **Step 3: 校验图像尺寸和透明通道**

确认四张图尺寸一致、四角 alpha 为 0、主体未被裁切、覆盖范围与当前待机帧接近。

### Task 3: 更新动作预览页

**Files:**
- Modify: `frog-pet-motion-preview.html`
- Modify: `frog-pet-image-preview.html`

- [ ] **Step 1: 替换待机和普通报警帧列表**

待机列表删除“开心、害羞、轻蹦迪”，插入“轻微抬头、低头观察、左右轻晃”；普通报警第二帧指向 `33-angry-green.png`。

- [ ] **Step 2: 在浏览器中验证动作切换**

打开 `file:///D:/lanchat/frog-pet-motion-preview.html`，确认待机动作幅度连续，报警三帧可明显区分，没有布局溢出或加载失败。

### Task 4: 重新组装正式图集并接入 Rust

**Files:**
- Modify: `public/pet-assets/frog-3d-actions-alpha.png`
- Modify: `src-tauri/src/native_frog_pet.rs`

- [ ] **Step 1: 按 4 x 4 固定顺序重新组装图集**

保留索引 0-13 的基础动作，将索引 6 替换为独立绿色生气帧，将索引 4、8、14 分别替换为抬头、低头、轻晃。

- [ ] **Step 2: 更新姿态常量**

将原 `POSE_HAPPY`、`POSE_SHY`、`POSE_PARTY` 改为 `POSE_LOOK_UP`、`POSE_LOOK_DOWN`、`POSE_SWAY`，普通报警仍使用 `[POSE_SURPRISE, POSE_ANGRY, POSE_ALERT]`。

- [ ] **Step 3: 更新待机时间轴**

三个微动作分散插入 24 秒待机循环，每个动作后回到 `POSE_IDLE`，不改变报警和蹦迪分支。

- [ ] **Step 4: 运行映射测试确认通过**

Run: `node scripts/test-native-frog-pet.mjs`

Expected: PASS。

Run: `node scripts/test-frog-pet-ui-native.mjs`

Expected: PASS。

### Task 5: 完整验证

**Files:**
- Verify: `src-tauri/src/native_frog_pet.rs`
- Verify: `frog-pet-motion-preview.html`

- [ ] **Step 1: Rust 编译检查**

Run: `cargo check`

Workdir: `src-tauri`

Expected: 编译通过。

- [ ] **Step 2: 前端构建**

Run: `npm run build`

Expected: TypeScript 与 Vite 构建通过；允许保留现有 chunk size 警告。

- [ ] **Step 3: 浏览器最终截图验证**

刷新动作预览页，检查三组动作均加载成功，待机循环不再出现开心、害羞和轻蹦迪，普通报警三张图不重复。

> 当前 `D:/lanchat/.git` 目录为空，无法执行计划中的分步提交；实现和验证仍在原目录完成。
