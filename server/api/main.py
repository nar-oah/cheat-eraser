import cv2
from celery.result import AsyncResult
from typing import Dict, List, Optional, Tuple
from fastapi import BackgroundTasks, FastAPI, File, UploadFile
from cheat_eraser_contracts.answer import AnswerResponse
from pretreatment import TestPaper
from exam_backup import save_exam
from tasks import add_answer, get_answer, get_formula


app = FastAPI()
exam = TestPaper(100.0)
answer_task: AsyncResult | None = None
answer_cache: AnswerResponse | None = None


def mod_answer(result: dict[str, object] | None) -> None:
    global answer_cache
    answer_cache = (
        AnswerResponse.model_validate(result)
        if isinstance(result, dict)
        else answer_cache
    )


@app.post("/pre-check")
async def pre_image(tasks: BackgroundTasks, file: UploadFile = File(...)) -> None:
    tasks.add_task(exam.add_paper, await file.read())


@app.get("/pages")
async def get_pages() -> List[int]:
    return list(exam.papers.keys())


@app.get("/missing")
async def get_missing() -> Dict[str, List[int]]:
    return exam.get_missing()


@app.post("/upload")
async def add_image() -> None:
    global answer_task
    papers: List[bytes] = [
        res[1].tobytes()
        for _, paper in sorted(exam.papers.items())
        if paper.paper is not None
        if (res := cv2.imencode(".jpg", paper.paper))[0]
    ]
    mod_answer(get_answer(answer_task))
    answer_task = add_answer(papers)


@app.get("/answer")
async def get_result() -> Optional[AnswerResponse]:
    mod_answer(get_answer(answer_task))
    return answer_cache


@app.post("/formula")
async def get_formula_image(position: Tuple[int, int]) -> Optional[bytes]:
    return get_formula(position)


@app.post("/reset")
async def reset() -> None:
    global exam
    save_exam(exam)
    exam = TestPaper(100.0)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=False)
