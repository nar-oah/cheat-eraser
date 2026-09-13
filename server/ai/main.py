from celery import Celery
from ai import Ai
from cheat_eraser_contracts.answer import Answer, AnswerResponse

celery_app = Celery(
    "cheat-eraser-ai",
    broker="redis://redis:6379/0",
    backend="redis://redis:6379/1",
)
ai = Ai()


@celery_app.task(name="cheat_eraser.ai.answer")
def add_answer(images: list[bytes]) -> dict[str, object] | None:
    ai.get_answer(images)
    return (
        AnswerResponse.get_response(ai.answer).model_dump(mode="json")
        if isinstance(ai.answer, Answer)
        else None
    )


@celery_app.task(name="cheat_eraser.ai.formula")
def get_formula(position: list[int]) -> bytes | None:
    return ai.get_formula((position[0], position[1]))
