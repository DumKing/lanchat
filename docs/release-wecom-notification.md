# GitHub Release 企业微信通知

发布流水线在 GitHub Release 创建成功后，可以向企业微信群机器人发送 Markdown 通知。

## 仓库配置

在 GitHub 仓库的 `Settings > Secrets and variables > Actions` 中配置：

- Variable `WECHAT_RELEASE_NOTIFY_ENABLED`：设置为 `true` 时开启通知；其它值或未配置时不发送。
- Secret `WECHAT_RELEASE_BOT_WEBHOOK`：企业微信群机器人的完整 Webhook 地址。该地址包含访问密钥，不能保存为普通 Variable，也不要提交到仓库。

配置后，通过 `v*` 标签或手动运行 `Release Packages` 流水线发布版本。通知内容使用企业微信 Markdown 报文，包含版本号、仓库、构建产物和 Release 下载链接。
