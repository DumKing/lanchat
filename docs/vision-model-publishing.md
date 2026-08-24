# LanChat 视觉模型发布规范

LanChat 的视觉模型由 GitHub Release 承载。客户端只接受由 Ed25519 根密钥签名的 `vision-catalog.json`，下载后还会校验 ZIP 摘要、Manifest V4、每个主模型和辅助文件摘要、适配器以及运行时后端。

## 模型组合

模型 Profile 是完整的识别组合，不是仅调整帧率和阈值的“档位”。当前官方目录定义以下组合：

| Profile | 人脸引擎 | 人体 ReID | 后端 | 分发方式 |
| --- | --- | --- | --- |
| `baseline` | SFace | YoutuReID | ONNX Runtime | 安装包内置 |
| `office-arcface-buffalo-sc` | ArcFace Buffalo-SC | YoutuReID | ONNX Runtime | 官方签名下载，仅非商用学习/研究 |
| `office-omz-retail-0288` | OMZ retail-0095 | OMZ retail-0288 | OpenVINO Runtime | 官方签名下载 |
| `office-omz-retail-0286` | OMZ retail-0095 | OMZ retail-0286 | OpenVINO Runtime | 官方签名下载 |

ArcFace 包使用 InsightFace `buffalo_sc` 的 `w600k_mbf.onnx`。InsightFace 的公开预训练权重仅限非商用研究用途；LanChat 因此只将它作为“学习/非商用”可选包发布，任何商业使用或二次分发都必须先取得相应授权。`custom-fastreid` 仍只允许按本地导入规范安装，直到许可证、来源和跨机验收完成。

## 资源目录

`scripts/vision-model-sources.json` 是发布模型资源的唯一入口。每个 Profile 必须拥有独立 `sourceDir`，且目录中至少包含：

```text
object-models/
  manifest.v4.json
  <主模型文件>
  <需要时的辅助文件，例如 OpenVINO .bin>
```

发布脚本不会把 `baseline` 权重复制成其他模型。若目录不存在、Manifest 中的识别/ReID 模型与来源声明不一致、哈希失配，或两个 Profile 的识别资产完全相同，构建会以 `PROFILE_HAS_NO_DISTINCT_MODEL_ASSETS` 失败。

OpenVINO Profile 的 XML 和同名 BIN 必须作为一个组件同时列入 Manifest；只包含 XML 的包会被拒绝。

## 许可证与来源门禁

发布前逐项确认：

1. 模型来源是官方仓库、官方模型库，或获得明确许可的内部资产。
2. `vision-model-profiles.json` 中的提供方、许可证和引擎与 Manifest 一致。
3. InsightFace `buffalo_sc` 只可用于非商用学习/研究发布；其他 InsightFace、FastReID 等受限或许可证未完成核验的权重不得进入官方下载目录。
4. OMZ IR 文件必须来自 Open Model Zoo 官方下载流程。发布流水线使用 `omz_downloader` 下载并生成 `0288`、`0286` 两套独立包。

## 签名与构建

在 GitHub Actions Secret 配置 `VISION_CATALOG_SIGNING_KEY`。值为 Ed25519 32 字节种子的 64 位十六进制；私钥不得提交到仓库、Release 附件或构建日志。客户端公钥固化在 Rust 代码中，轮换时必须先发布包含新公钥的客户端。

在安全构建机执行：

```powershell
node scripts/test-vision-package-distinctness.mjs
./scripts/package-vision-models.ps1 -Tag v0.6.1
```

构建产物位于 `release-assets/vision-models`，必须作为同一个 Release 附件上传：

- 各 Profile ZIP；
- 已签名 `vision-catalog.json`；
- `model-diff.json`，用于审计模型家族、摘要和向量空间差异。

若 OMZ 或 ArcFace 的真实资源尚未准备，构建会明确失败；不得用基线模型替代后发布。

## 客户端激活

客户端安装模型时先在 staging 目录验签、验 Manifest 和文件摘要。激活前还会检查本机后端：ONNX Profile 需要 ONNX Runtime，OpenVINO Profile 需要 OpenVINO Runtime 与 XML/BIN 成对资源。当前版本允许下载 OpenVINO 包并校验完整性；在完整 OpenVINO 推理执行器随客户端发布前，模型中心会明确禁止激活，而不会静默回退到其他模型。通过后模型将标记为“下次启用”；当前兼容运行时会在应用重启后加载新 Profile。

每个 Profile 的人脸与人体向量都以独立 `EmbeddingSpaceId` 存入本机 FeatureStore。切换模型只重建目标空间，不会覆盖或比较旧空间向量。
