# Viewer

`viewer` 是 cheat-eraser 项目的 ESP32-S3 墨水屏查看端。固件启动后会连接 Wi-Fi，从后端接口拉取页码、缺失题目和答案数据，并把内容渲染到 200x200 的 Waveshare 1.54 寸 V2 墨水屏上。

## 功能

- 显示已识别页码、缺失题目、单选题、多选题、判断题和非选择题答案。
- 使用四个 3x3 棋盘区域进行内容显示和题号定位。
- 左键短按切换上一页，右键短按切换下一页；长按可进入 deep sleep、手动刷新或重置服务端。
- 非选择题支持中文文本、英文提示文本和公式 BMP 图片。
- 60 秒无操作后让墨水屏休眠，刷新时自动唤醒。

## 硬件

当前代码面向 ESP32-S3，并启用了 PSRAM 配置。显示屏驱动使用 `epd-waveshare` 的 `epd1in54_v2`。

### 墨水屏引脚

| 功能 | GPIO |
| --- | --- |
| SCK | GPIO7 |
| MOSI | GPIO9 |
| CS | GPIO1 |
| DC | GPIO2 |
| RST | GPIO3 |
| BUSY | GPIO4 |

### 按键引脚

| 功能 | GPIO | 说明 |
| --- | --- | --- |
| 左键 | GPIO6 | 短按切换到上一个画面；长按进入 deep sleep，不重置服务端 |
| 右键 | GPIO5 | 短按切换到下一个画面；长按手动刷新当前画面 |
| 左键 + 右键 | GPIO6 + GPIO5 | 双键长按后重置服务端，不进入 deep sleep |

按键使用上拉输入，按下时应接地。进入 deep sleep 后，按下任一按键都可唤醒 viewer；唤醒后需先松开按键，固件才会继续处理新的按键操作。

## 后端接口

默认后端地址定义在 `src/api.rs`：

```rust
const URL: &str = "https://aws.naroah.top/cheat/";
```

固件会访问以下接口：

| 接口 | 方法 | 返回/用途 |
| --- | --- | --- |
| `/pages` | GET | 已识别页码数组，例如 `[1, 2, 4]` |
| `/missing` | GET | 缺失题目映射，例如 `{"单": [3, 8], "多": [2]}` |
| `/answer` | GET | 答案数据 |
| `/formula` | POST | 请求体为 `[题号, 公式序号]`，返回 BMP 图片字节 |
| `/reset` | POST | 重置后端状态 |

`/answer` 的 JSON 结构对应 `src/api.rs` 中的 `Answer`：

```json
{
  "single_choice": ["A", "B"],
  "multiple_choice": ["AC", "BD"],
  "binary_choice": [true, false],
  "non_choice": [
    {
      "answer": ["中文答案$后续文本"],
      "english": ["keyword"],
      "math": 1
    }
  ]
}
```

非选择题答案中的 `$` 表示公式占位。公式图片由 `/formula` 按需获取。

## 配置

烧录前通常需要检查两处硬编码配置：

- `src/wifi.rs`：Wi-Fi SSID 和密码。
- `src/api.rs`：后端 API 地址。

ESP-IDF 相关配置在：

- `.cargo/config.toml`：目标平台、runner 与 ESP-IDF `v5.5.3`；工具安装在当前 workspace 的 `.embuild/` 中。
- `rust-toolchain.toml`：使用 `esp` Rust toolchain。
- `sdkconfig.defaults`：ESP32-S3 PSRAM、主任务栈等配置。

## 开发环境

需要安装 ESP Rust 工具链和烧录工具：

```sh
cargo install espup
espup install
cargo install espflash
```

如果 `espup` 生成了环境脚本，请在当前 shell 中加载：

```sh
source ~/export-esp.sh
```

## 构建与烧录

仅构建：

```sh
cargo build --release
```

烧录并打开串口监视器：

```sh
cargo run --release
```

`.cargo/config.toml` 已配置：

```toml
target = "xtensa-esp32s3-espidf"
runner = "espflash flash --monitor"
```

因此 `cargo run` 会调用 `espflash` 完成烧录和监视。

## 显示流程

画面按以下顺序浏览：

1. 页码识别状态。
2. 缺失题目。
3. 单选题答案。
4. 多选题答案。
5. 判断题答案。
6. 非选择题答案。

布局规则详见 `Layout.md`。主要约定如下：

- 右下角棋盘用于当前页面或题号定位。
- 单选、判断每页最多显示 27 项。
- 多选和缺失题每页最多显示 3 项。
- 非选择题文本每帧最多显示 27 个字符，英文文本显示在中间区域，公式以 BMP 图片显示。

## 项目结构

```text
.
├── src/
│   ├── api.rs       # 后端 HTTP 接口
│   ├── button.rs    # 双按键输入和事件识别
│   ├── display.rs   # 墨水屏初始化、绘制和刷新
│   ├── layout.rs    # 页面状态、布局和渲染逻辑
│   ├── main.rs      # 程序入口
│   └── wifi.rs      # Wi-Fi 连接
├── .cargo/config.toml
├── Cargo.toml
├── build.rs
├── rust-toolchain.toml
└── sdkconfig.defaults
```

## 常见问题

### 找不到 ESP toolchain

确认已经执行 `espup install`，并在当前 shell 中加载了 `~/export-esp.sh`。

### 烧录失败或找不到串口

确认开发板已连接，并检查 `espflash` 是否能识别设备：

```sh
espflash board-info
```

### 启动后一直没有内容

检查串口日志中的 Wi-Fi 和 HTTP 错误。当前 Wi-Fi 与后端地址都在代码中硬编码，网络环境变化时需要同步修改。
