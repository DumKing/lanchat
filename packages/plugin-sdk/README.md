# LanChat 内置插件 SDK

此目录是 LanChat 插件公共契约的唯一来源。类型、运行时常量或客户端方法变化后，运行：

```bash
npm run public-api:generate --workspace @lanchat/plugin-sdk
```

生成器从编译后的声明和运行时导出创建规范化 `public-api-report.json`。报告变化必须随契约修改一同显式提交。

独立 `lanchat-plugin-sdk` 仓库是此契约的发布镜像。涉及两仓的变更必须先合并主程序，再让独立 SDK 基于主程序 `main` 的报告通过同步检查；主程序 CI 不依赖独立仓库。
