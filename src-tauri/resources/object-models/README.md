# 本机画面出现检测模型资源

此目录只存放本机离线检测模型，不会通过 LanChat 的局域网消息或文件服务传播。

默认随 Windows 安装包提供：

- `presence-detector.onnx`：OpenCV Zoo YuNet 人脸检测模型，MIT。
- `face-recognizer.onnx`：OpenCV Zoo SFace 人脸识别模型，MIT。

相关模型来源和许可证：

- https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet
- https://github.com/opencv/opencv_zoo/tree/main/models/face_recognition_sface

`manifest.json` 的 `modelVersion` 必须是非空版本号；模型须填写 SHA-256。应用启动时会校验文件与摘要，校验失败时摄像头自动告警保持不可用，并在设置页显示原因。

检测模型只定位画面中的人脸；识别模型仅与本机录入人员的本地特征向量比对，特征数据不出本机。摄像头采样帧只进入内存推理流程，并在本轮推理结束后释放。
