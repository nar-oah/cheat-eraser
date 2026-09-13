# Cheat Eraser Server

试卷图像预处理、缺题检测和答案生成服务。服务基于 FastAPI，接收试卷照片后通过 OpenCV 与 RapidOCR 做页面裁剪、清晰度过滤和题号识别，再调用 Google Gemini 生成按题型分类的答案。

## 功能

- 上传试卷图片并按页码保留清晰度最高的版本。
- OCR 识别大题、小题题号，返回缺失题号。
- 将已收集的试卷页发送给 Gemini，生成选择题、判断题和非选择题答案。
- 非选择题答案中的数学公式使用占位符返回，并可按索引生成公式图片。
- 重置当前试卷状态时将已处理页面备份到 `backup/`。

## 项目结构

```text
.
├── main.py                # FastAPI 入口与接口定义
├── ai.py                  # Gemini 调用、答案结构和公式图片生成
├── ocr.py                 # OCR、页码识别、裁剪和缺题计算辅助逻辑
├── pretreatment.py        # 试卷图片预处理与页面状态管理
├── exam_backup.py         # reset 时保存当前试卷快照
├── requirements.txt       # Python 依赖
└── cheat-eraser.service   # systemd 服务示例
```

运行时目录：

- `.env`：环境变量文件，包含 API Key。
- `saved_images/`：本地调试图片目录。
- `output/`：本地调试输出目录。
- `backup/`：调用 `/reset` 后生成的试卷备份目录。

这些目录和敏感文件已在 `.gitignore` 中忽略。

## 环境要求

- Python 3.11
- 可访问 Google Gemini API 的 API Key

创建虚拟环境并安装依赖：

```bash
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

创建 `.env`：

```bash
GEMINI_API_KEY=your-api-key
```

`google-genai` 也支持 `GOOGLE_API_KEY`。如果两个变量都设置，SDK 会优先使用 `GOOGLE_API_KEY`。

## 本地启动

```bash
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000
```

也可以直接运行入口文件：

```bash
source venv/bin/activate
python main.py
```

服务启动后访问：

- API 根地址：`http://localhost:8000`
- Swagger 文档：`http://localhost:8000/docs`

## 接口流程

1. 上传试卷页：

```bash
curl -X POST http://localhost:8000/pre-check \
  -F "file=@/path/to/paper.jpg"
```

该接口会把图片处理任务放入后台队列。图片需要足够清晰，并且能通过 OCR 识别页码。

2. 查看已收集页码：

```bash
curl http://localhost:8000/pages
```

返回示例：

```json
[1, 2, 3]
```

3. 查看缺失题号：

```bash
curl http://localhost:8000/missing
```

返回示例：

```json
{
  "一": [3, 4],
  "二": []
}
```

4. 触发答案生成：

```bash
curl -X POST http://localhost:8000/upload
```

该接口同样在后台调用 Gemini。调用后可轮询 `/answer` 获取结果。

5. 获取答案：

```bash
curl http://localhost:8000/answer
```

返回结构：

```json
{
  "single_choice": ["A", "C"],
  "multiple_choice": ["AB", "BCD"],
  "binary_choice": [true, false],
  "non_choice": [
    {
      "answer": "答案文本，公式用$(0)占位，英文用$[0]占位",
      "english": ["example"],
      "math": 1
    }
  ]
}
```

6. 获取公式图片：

```bash
curl -X POST http://localhost:8000/formula \
  -H "Content-Type: application/json" \
  -d '[0, 0]'
```

请求体格式为 `[非选择题索引, 公式索引]`。接口返回对应公式图片字节或 `null`。

7. 重置当前试卷：

```bash
curl -X POST http://localhost:8000/reset
```

重置前会将当前处理出的试卷页面和识别信息保存到 `backup/<timestamp>/`。

## 本地调试脚本

处理 `saved_images/` 中的试卷图片并输出到 `output/`：

```bash
source venv/bin/activate
python pretreatment.py
```

使用 `output/filter3.png` 调试 Gemini 答案生成与公式渲染：

```bash
source venv/bin/activate
python ai.py
```

## systemd 部署

仓库提供了 `cheat-eraser.service` 示例。默认配置假设项目位于：

```text
/home/admin/cheat_eraser
```

部署前按实际服务器调整以下字段：

- `User`
- `Group`
- `WorkingDirectory`
- `EnvironmentFile`
- `ExecStart`

安装服务：

```bash
sudo cp cheat-eraser.service /etc/systemd/system/cheat-eraser.service
sudo systemctl daemon-reload
sudo systemctl enable cheat-eraser
sudo systemctl start cheat-eraser
```

查看状态和日志：

```bash
sudo systemctl status cheat-eraser
sudo journalctl -u cheat-eraser -f
```

## 注意事项

- `/pre-check` 和 `/upload` 都使用后台任务，接口返回成功不代表后台任务已经完成。
- 当前服务状态保存在进程内存中，重启服务会丢失未备份的当前试卷状态。
- `/reset` 会备份已处理页面，但不会备份原始上传文件。
- Gemini 调用依赖外部网络和 API Key，`/answer` 在生成完成前会返回 `null`。
