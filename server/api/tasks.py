import logging

from celery import Celery
from celery.result import AsyncResult
from pydantic import ValidationError
from os import getenv
from cheat_eraser_contracts.answer import AnswerResponse, AnswerResult

QUEUE = "cheat-eraser-ai"
celery_app = Celery(
    "cheat-eraser-api",
    broker=getenv("CELERY_BROKER_URL", "redis://redis:6379/0"),
    backend=getenv("CELERY_RESULT_BACKEND", "redis://redis:6379/1"),
)
logger = logging.getLogger(__name__)


def add_answer(images: list[bytes]) -> AsyncResult:
    return celery_app.send_task(
        "cheat_eraser.ai.answer",
        args=[images],
        queue=QUEUE,
    )


def get_answer(task: AsyncResult | None) -> AnswerResult:
    if task is None or not task.ready():
        return AnswerResult(status="pending")
    if task.failed():
        logger.error(
            "AI answer task %s failed: %r\n%s",
            task.id,
            task.result,
            task.traceback or "",
        )
        return AnswerResult(status="error", error="AI answer task failed")
    result = task.get(propagate=False)
    if not isinstance(result, dict):
        logger.error("AI answer task %s returned invalid result: %r", task.id, result)
        return AnswerResult(status="error", error="AI answer task returned no answer")
    try:
        answer = AnswerResponse.model_validate(result)
    except ValidationError as error:
        logger.error("AI answer task %s returned invalid answer: %s", task.id, error)
        return AnswerResult(
            status="error", error="AI answer task returned invalid answer"
        )
    return AnswerResult(status="ready", answer=answer)


def get_formula(position: tuple[int, int]) -> bytes | None:
    task = celery_app.send_task(
        "cheat_eraser.ai.formula",
        args=[position],
        queue=QUEUE,
    )
    value = task.get(timeout=30)
    task.forget()
    return value if isinstance(value, bytes) else None
