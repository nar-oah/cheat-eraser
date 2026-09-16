import cv2
import json
import logging
from os import getenv
from celery.result import AsyncResult
from typing import Dict, List, Optional, Tuple
from fastapi import FastAPI, File, HTTPException, Response, UploadFile
from cheat_eraser_contracts.answer import AnswerResponse, AnswerResult
from pretreatment import PaperResult, TestPaper
from exam_backup import save_exam
from tasks import add_answer, get_answer, get_formula


app = FastAPI()
logger = logging.getLogger(__name__)
EXPECTED_RANGES: Dict[str, Tuple[int, int]] = {
    section: (int(bounds[0]), int(bounds[1]))
    for section, bounds in json.loads(
        getenv("EXPECTED_QUESTION_RANGES", "{}") or "{}"
    ).items()
}
exam = TestPaper(100.0, EXPECTED_RANGES)
answer_task: AsyncResult | None = None
answer_cache: AnswerResponse | None = None
answer_error: str | None = None


def mod_answer(result: AnswerResult) -> None:
    global answer_cache, answer_error
    if result.status == "ready":
        answer_cache = result.answer
        answer_error = None
    elif result.status == "error":
        answer_cache = None
        answer_error = result.error


@app.post("/pre-check", response_model=PaperResult)
async def pre_image(file: UploadFile = File(...)) -> PaperResult:
    return exam.add_paper(await file.read())


@app.get("/pages")
async def get_pages() -> List[int]:
    return list(exam.papers.keys())


@app.get("/missing")
async def get_missing() -> Dict[str, List[int]]:
    return exam.get_missing()


@app.post("/upload")
async def add_image() -> None:
    global answer_task, answer_cache, answer_error
    answer_task = None
    answer_cache = None
    answer_error = None
    papers: List[bytes] = [
        res[1].tobytes()
        for _, paper in sorted(exam.papers.items())
        if paper.paper is not None
        if (res := cv2.imencode(".jpg", paper.paper))[0]
    ]
    try:
        answer_task = add_answer(papers)
    except Exception as error:
        logger.exception("Failed to dispatch AI answer task")
        answer_error = "AI answer task could not be started"
        raise HTTPException(status_code=503, detail=answer_error) from error


@app.get("/answer", response_model=AnswerResult)
async def get_result() -> AnswerResult:
    if answer_task is None and answer_error is not None:
        return AnswerResult(status="error", error=answer_error)
    result = get_answer(answer_task)
    mod_answer(result)
    return AnswerResult(
        status=result.status,
        answer=answer_cache if result.status == "ready" else None,
        error=answer_error if result.status == "error" else None,
    )


@app.post("/formula")
async def get_formula_image(position: Tuple[int, int]) -> Response:
    value = get_formula(position)
    return (
        Response(content=value, media_type="image/bmp")
        if value is not None
        else Response(status_code=204)
    )


@app.post("/reset")
async def reset() -> None:
    global exam, answer_task, answer_cache, answer_error
    save_exam(exam)
    exam = TestPaper(100.0, EXPECTED_RANGES)
    answer_task = None
    answer_cache = None
    answer_error = None


if __name__ == "__main__":
    import uvicorn

    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=False)
