FROM ghcr.io/astral-sh/uv:python3.14-bookworm-slim AS builder

WORKDIR /app
ENV UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_HTTP_TIMEOUT=120

COPY . .
RUN uv sync --locked --no-dev --no-editable --package cheat-eraser-ai

FROM ghcr.io/astral-sh/uv:python3.14-bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends libcairo2 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/.venv /app/.venv
COPY --from=builder /app/ai /app/ai

ENV PATH="/app/.venv/bin:$PATH"
WORKDIR /app/ai

CMD ["celery", "-A", "main:celery_app", "worker", "-Q", "cheat-eraser-ai", "--pool=threads", "--concurrency=4", "--loglevel=info"]
