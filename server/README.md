# Cheat Eraser Server

试卷图像预处理、缺题检测和答案生成服务。后端使用 Python 3.14 与 uv workspace，HTTP API 和 Gemini 任务分别运行在独立容器中，Redis 负责 Celery 消息与任务结果。

## 结构

```text
server/
├── pyproject.toml
├── uv.lock
├── api/
│   ├── Dockerfile
│   ├── main.py
│   ├── tasks.py
│   ├── ocr.py
│   ├── pretreatment.py
│   └── exam_backup.py
├── ai/
│   ├── Dockerfile
│   ├── main.py
│   └── ai.py
└── contracts/
    └── src/cheat_eraser_contracts/
```

- `api` 保留试卷页面的进程内状态，提供原有 `/pre-check`、`/pages`、`/missing`、`/upload`、`/answer`、`/formula` 和 `/reset` 接口。
- `ai` 消费 `cheat-eraser-ai` 队列，执行 Gemini 请求与公式图片生成。
- `contracts` 保存两个服务共享的 Pydantic 响应模型。

API 必须保持单 worker、单副本，否则进程内的当前试卷会被拆散。AI 同样保持单进程、单并发且不横向扩容，因为当前答案保存在内存中。

## 安装

```bash
cd server
uv sync --all-packages --locked
```

创建仓库根目录的 `.env`：

```dotenv
GEMINI_API_KEY=your-api-key
EXPECTED_QUESTION_RANGES={"一":[1,10],"二":[11,20]}
```

也可使用 `GOOGLE_API_KEY`；两者同时存在时 Google SDK 优先读取 `GOOGLE_API_KEY`。
`EXPECTED_QUESTION_RANGES` 按大题名配置闭区间题号，用于检测每个大题首题、末题以及整段缺失；请在正式测试前替换为实际试卷范围，留空为 `{}` 时只检测已识别首尾之间的缺题。

## 本地启动

先启动 Redis：

```bash
docker run --rm --name cheat-eraser-redis -p 6379:6379 redis:8-alpine
```

启动 AI worker：

```bash
cd server/ai
CELERY_BROKER_URL=redis://127.0.0.1:6379/0 \
CELERY_RESULT_BACKEND=redis://127.0.0.1:6379/1 \
GEMINI_API_KEY=your-api-key \
uv run celery -A main:celery_app worker -Q cheat-eraser-ai \
  --pool=threads --concurrency=1 --loglevel=info
```

启动 API：

```bash
cd server/api
CELERY_BROKER_URL=redis://127.0.0.1:6379/0 \
CELERY_RESULT_BACKEND=redis://127.0.0.1:6379/1 \
uv run uvicorn main:app --host 0.0.0.0 --port 8000 --workers 1
```

Swagger 文档位于 `http://127.0.0.1:8000/docs`。

## 接口流程

1. `POST /pre-check` 上传字段名为 `file` 的试卷图片，并在当前请求内完成预处理/OCR；响应包含 `accepted`、`page`、`variance` 和 `reject_reason`。
2. `GET /pages` 与 `GET /missing` 轮询当前扫描状态。
3. `POST /upload` 清除旧答案缓存，并把当前已收集页面提交给 AI worker。
4. `GET /answer` 返回 `pending`、`ready` 或 `error` 状态；`ready` 时 `answer` 包含答案，`error` 时 `error` 包含失败原因。
5. `POST /formula` 以从 0 开始的 `[非选择题索引, 公式索引]` 请求公式图片；成功返回原始 `image/bmp`，无结果返回 204。
6. `POST /reset` 将当前页面写入 `backup/`，并清空试卷、AI 任务和答案状态。

`/answer` 响应示例：

```json
{
  "status": "ready",
  "answer": {
    "single_choice": ["A"],
    "multiple_choice": ["AC"],
    "binary_choice": [true],
    "non_choice": [
      {"answer": "文字答案$", "english": [], "math": 1}
    ]
  },
  "error": null
}
```

## Docker 与生产部署

两个应用镜像都以 `server/` 为构建上下文：

```bash
docker build -f server/api/Dockerfile -t cheat-eraser-api server
docker build -f server/ai/Dockerfile -t cheat-eraser-ai server
```

仓库根目录的 `compose.prod.yml` 会拉取：

- `ghcr.io/nar-oah/cheat-eraser/api:latest`
- `ghcr.io/nar-oah/cheat-eraser/ai:latest`
- `redis:8-alpine`

API 通过已有的外部 `traefik` 网络发布到 `https://aws.naroah.top/cheat`。部署主机需要预先创建该网络：

```bash
docker network create traefik
docker compose -f compose.prod.yml up -d --wait
```

`backup/` 使用名为 `cheat-eraser-backup` 的 Docker volume 持久化。旧 systemd 部署中的备份不会自动进入该 volume，切换前需按需要手工迁移。

## 自动部署

`.github/workflows/deploy.yml` 在 `main` 的后端相关变更上构建并推送 API、AI 镜像，然后通过 SSH 上传 Compose 文件并更新服务。版本标签 `v*.*.*` 还会发布对应的镜像标签，但不会触发服务器部署。

需要配置以下 GitHub Secrets：

- `SERVER_HOST`
- `SERVER_USER`
- `SERVER_SSH_KEY`

工作流使用 Actions 内建的 `GITHUB_TOKEN` 推送镜像。Compose 拉取沿用参考项目的约定：GHCR package 需公开，或部署机需提前完成 GHCR 登录。
