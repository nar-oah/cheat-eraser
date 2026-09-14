# Cheat Eraser Scanner

ESP32-S3 摄像头扫描器固件。设备连接 Wi-Fi 后初始化摄像头，按下 GPIO2 上的按键会拍摄一张 JPEG 图片，并通过后台线程以 `multipart/form-data` 上传到服务端预检接口。

## 功能

- 基于 Rust + ESP-IDF 构建，目标芯片为 ESP32-S3。
- 使用 `espressif/esp32-camera` 组件采集 JPEG 图像。
- 按键触发拍照，GPIO2 低电平表示按下。
- 图片采集后通过独立上传线程发送，避免阻塞主循环。
- 启用 PSRAM，用于承载较大的相机帧缓冲。

## 硬件要求

- ESP32-S3 开发板，建议带 PSRAM。
- 兼容 ESP32 Camera 驱动的摄像头模组。
- 接到 GPIO2 的按键，当前代码使用内部上拉，因此按下时应拉低。
- 可访问配置 Wi-Fi 的网络环境。

当前摄像头引脚配置写在 [src/camera.rs](src/camera.rs)：

| 功能 | GPIO |
| --- | --- |
| XCLK | 10 |
| SCCB SDA | 40 |
| SCCB SCL | 39 |
| D0-D7 | 15, 17, 18, 16, 14, 12, 11, 48 |
| VSYNC | 38 |
| HREF | 47 |
| PCLK | 13 |
| PWDN / RESET | 未使用 |

## 项目结构

```text
.
├── src/
│   ├── main.rs      # 初始化 Wi-Fi、相机和按键主循环
│   ├── camera.rs    # ESP32 Camera 配置、取帧和释放
│   ├── wifi.rs      # Wi-Fi 连接配置
│   ├── upload.rs    # 图片上传 HTTP 客户端
│   └── bindings.h   # esp32-camera bindgen 入口
├── .cargo/config.toml
├── sdkconfig.defaults
├── rust-toolchain.toml
├── Cargo.toml
└── TODO.md
```

## 开发环境

需要准备：

- ESP Rust 工具链，项目使用 [rust-toolchain.toml](rust-toolchain.toml) 中的 `esp` channel。
- `ldproxy` 与 `espflash`。
- ESP-IDF `v5.5.3`；模板配置会将工具安装在当前 workspace 的 `.embuild/` 中。

目标平台配置位于 [.cargo/config.toml](.cargo/config.toml)，默认 target 为 `xtensa-esp32s3-espidf`。

## 配置

当前有两处运行时配置直接写在源码里：

- Wi-Fi SSID 和密码：[src/wifi.rs](src/wifi.rs)
- 图片上传接口：[src/upload.rs](src/upload.rs)

在部署到不同环境前，需要先修改这两个文件。建议后续把这些值改为构建参数、NVS 配置或其他不直接提交到仓库的配置来源。

PSRAM 和任务栈等 ESP-IDF 配置位于 [sdkconfig.defaults](sdkconfig.defaults)。当前相机帧使用 PSRAM，图像格式为 JPEG，分辨率为 UXGA，JPEG 质量参数为 `10`。

## 构建

Release 构建：

```bash
cargo build --release
```

Debug 构建：

```bash
cargo build
```

产物默认位于：

```text
target/xtensa-esp32s3-espidf/<debug|release>/scanner
```

## 烧录

Release 构建并烧录：

```bash
cargo run --release
```

Debug 构建并烧录：

```bash
cargo run
```

`.cargo/config.toml` 中的 runner 会调用 `espflash flash --monitor` 完成烧录并打开串口监控。

## 运行流程

1. 程序启动后初始化 ESP-IDF patch 和日志。
2. 连接 Wi-Fi。
3. 初始化摄像头。
4. 配置 GPIO2 为上拉输入。
5. 启动后台上传线程。
6. 主循环轮询按键状态。
7. 按键从未按下变为按下时拍摄 JPEG。
8. 图片数据复制到 `Vec<u8>` 并发送给上传线程。
9. 上传线程通过 HTTPS POST 上传图片并记录服务端响应。

## 常见问题

### Camera init failed

检查摄像头型号、排线方向、供电和 [src/camera.rs](src/camera.rs) 中的引脚定义是否匹配当前开发板。

### 拍照或上传时内存不足

当前使用 UXGA 分辨率和 PSRAM 帧缓冲。可以尝试降低 `frame_size`、调整 `jpeg_quality`，或检查 [sdkconfig.defaults](sdkconfig.defaults) 中的 PSRAM 配置是否生效。

### Wi-Fi connected 之后上传失败

检查 [src/upload.rs](src/upload.rs) 中的上传地址、服务端证书链、网络连通性和服务端是否接受 `file` 字段的 JPEG multipart 上传。

## 当前限制

- Wi-Fi 和上传地址仍是硬编码配置。
- 暂无自动化测试。
- `TODO.md` 中记录了后续方向：在不引起内存崩溃的前提下提升图片质量。
