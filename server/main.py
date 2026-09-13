import cv2
from typing import Dict, List, Optional, Tuple
from fastapi import FastAPI, UploadFile, File, BackgroundTasks
from pretreatment import TestPaper
from ai import AnswerResponse, Ai
from exam_backup import save_exam


app = FastAPI()
exam = TestPaper(100.0)
ai = Ai()


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
async def add_image(tasks: BackgroundTasks) -> None:
    papers: List[bytes] = [
        res[1].tobytes()
        for _, paper in sorted(exam.papers.items())
        if paper.paper is not None
        if (res := cv2.imencode(".jpg", paper.paper))[0]
    ]
    tasks.add_task(ai.get_answer, papers)


@app.get("/answer")
async def get_result() -> Optional[AnswerResponse]:
    if ai.answer:
        return AnswerResponse.get_response(ai.answer)
    return None


@app.post("/formula")
async def get_formula(position: Tuple[int, int]) -> Optional[bytes]:
    return ai.get_formula(position)


@app.post("/reset")
async def reset() -> None:
    global exam
    save_exam(exam)
    exam = TestPaper(100.0)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
