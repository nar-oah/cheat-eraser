from celery import Celery
from celery.result import AsyncResult
from os import getenv

QUEUE = "cheat-eraser-ai"
celery_app = Celery(
    "cheat-eraser-api",
    broker=getenv("CELERY_BROKER_URL", "redis://redis:6379/0"),
    backend=getenv("CELERY_RESULT_BACKEND", "redis://redis:6379/1"),
)


def add_answer(images: list[bytes]) -> AsyncResult:
    return celery_app.send_task(
        "cheat_eraser.ai.answer",
        args=[images],
        queue=QUEUE,
    )


def get_answer(task: AsyncResult | None) -> dict[str, object] | None:
    return (
        task.get(propagate=False)
        if isinstance(task, AsyncResult) and task.ready()
        else None
    )


def get_formula(position: tuple[int, int]) -> bytes | None:
    task = celery_app.send_task(
        "cheat_eraser.ai.formula",
        args=[position],
        queue=QUEUE,
    )
    value = task.get(timeout=30)
    task.forget()
    return value if isinstance(value, bytes) else None
