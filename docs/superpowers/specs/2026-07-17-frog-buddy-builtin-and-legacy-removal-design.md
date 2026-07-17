# Frog Buddy 内置与旧青蛙移除设计

## 目标

- 将 `D:\lcpic\frog-buddy` 作为第二个内置桌宠资源包随应用发布。
- 将 `frog-buddy` 设为首次启动、未选择桌宠或原选择失效时的默认桌宠。
- 保留用户已经选择且仍然有效的其他桌宠。
- 删除旧版固定青蛙图集、专属渲染回退、旧 WebView 桌宠、生成脚本和兼容接口。
- 保留呱呱告警、可信度、蹦迪、反馈、快捷键、拖动和缩放能力，并由当前选中的资源包展示。

## 架构

Rust 原生窗口继续负责透明置顶窗口、告警详情、状态机和鼠标交互，但运行时只从 `DesktopPetPackage` 读取 `Idle / Alert / Move / Interact / Life` 图片，不再编译期嵌入青蛙图集。运行时、状态、控制器、命令和事件统一改为 `desktop_pet` 命名。

内置资源由 `src-tauri/resources/desktop-pets/` 扫描并随 Tauri bundle 打包。注册器在保存的选择为空或不存在时选择 `frog-buddy`；有效的用户选择不被覆盖。

## 清理边界

删除以下旧实现：

- `public/pet-assets/` 下固定青蛙图集、GIF 和候选图。
- 旧青蛙 GIF/预览生成脚本与专属静态检查脚本。
- `native_frog_pet.rs`、`NativeFrogPet*`、`set_frog_pet_enabled`、`update_native_frog_pet`、`native_frog_action`。
- 已停用的 `frog-pet` WebView 分支和对应 CSS。

不删除以下业务能力：

- 局域网告警及其默认文案。
- 告警可信度、真实/虚假反馈和狼来了排行榜。
- 普通/蹦迪报警模式、停止快捷键和超管下发能力。

## 失败处理

- 新内置包不通过 Manifest V2 校验时构建验证失败。
- 当前选择失效时自动回退到 `frog-buddy`。
- 单帧解码失败时跳过绘制并记录日志，不再使用旧图集兜底。
- 资源包不可用时原生桌宠窗口保持透明，主应用继续运行。

## 验证

- Rust 单元测试覆盖默认选择、有效选择保留和无效选择回退。
- Node 静态测试覆盖新包已内置、旧嵌入图集和旧兼容 API 已移除。
- 运行桌宠资源包校验、前端构建、Rust 测试和 `cargo check`。
