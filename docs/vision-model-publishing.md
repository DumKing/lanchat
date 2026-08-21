# 视觉模型档位发布

LanChat 的视觉模型目录由 GitHub Release 承载。客户端只接受由 Ed25519 根密钥签名的 `vision-catalog.json`，并会逐项校验下载包、Manifest V3 与全部 ONNX 摘要。

## 首次配置

在仓库 Actions Secret 中设置 `VISION_CATALOG_SIGNING_KEY`：值是 32 字节 Ed25519 种子的 64 位小写十六进制。私钥不能提交到仓库、Release 附件或日志。

对应公钥已固化在 `src-tauri/src/vision/model_manager.rs`。如需轮换密钥，必须先发布包含新公钥的 LanChat 客户端，再切换 GitHub Secret。

## 发布内容

模型档位定义在 `scripts/vision-model-profiles.json`。当前有三套轻量档位，均复用内置的四个 ONNX 权重，只改变建议采样频率、识别阈值与连续命中次数：

- `office-light`：低资源巡检，1 帧/秒。
- `office-balanced`：均衡识别，2 帧/秒。
- `office-sensitive`：灵敏巡检，3 帧/秒。

完整发布工作流会自动生成三个 ZIP 和签名后的 `vision-catalog.json`，作为同一版本 GitHub Release 的附件。客户端在视觉识别页点击“检查模型”即可拉取目录，下载后选择“下次启用”，重启应用生效。

## 本地验证

在安全环境设置 `VISION_CATALOG_SIGNING_KEY` 后执行：

```powershell
./scripts/package-vision-models.ps1 -Tag v0.5.2
```

生成结果位于 `release-assets/vision-models`。该目录为发布中间产物，不应提交。
