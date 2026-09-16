import asyncio
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parents[1]))

import main as api
import ocr
from cheat_eraser_contracts.answer import AnswerResponse
from pretreatment import PaperInfo, TestPaper as Exam


ANSWER = {
    "single_choice": ["A"],
    "multiple_choice": ["AB"],
    "binary_choice": [True],
    "non_choice": [{"answer": "text", "english": [], "math": 0}],
}


class FakeTask:
    id = "test-task"
    traceback = "worker traceback"

    def __init__(self, ready=True, failed=False, result=None):
        self._ready = ready
        self._failed = failed
        self.result = result

    def ready(self):
        return self._ready

    def failed(self):
        return self._failed

    def get(self, propagate=False):
        return self.result


def get_answer_result(task):
    api.answer_task = task
    api.answer_cache = None
    api.answer_error = None
    return asyncio.run(api.get_result())


def test_reset_clears_answer_task_and_cache(monkeypatch):
    monkeypatch.setattr(api, "save_exam", lambda value: None)
    monkeypatch.setattr(api, "EXPECTED_RANGES", {"一": (1, 5)})
    api.exam = Exam(100.0)
    api.answer_task = FakeTask()
    api.answer_cache = AnswerResponse.model_validate(ANSWER)
    api.answer_error = "old error"

    asyncio.run(api.reset())

    assert api.exam.papers == {}
    assert api.exam.expected_ranges == {"一": (1, 5)}
    assert api.answer_task is None
    assert api.answer_cache is None
    assert api.answer_error is None


def test_upload_clears_old_answer_cache_and_starts_pending(monkeypatch):
    task = FakeTask(ready=False)
    monkeypatch.setattr(api, "add_answer", lambda papers: task)
    api.exam = Exam(100.0)
    api.answer_cache = AnswerResponse.model_validate(ANSWER)
    api.answer_error = "old error"

    asyncio.run(api.add_image())

    assert api.answer_task is task
    assert api.answer_cache is None
    assert api.answer_error is None


def test_total_pages_footer_is_rejected_without_exception(monkeypatch):
    result = [[[[10, 90], [20, 90], [20, 95], [10, 95]], "共 12 页", 0.99]]
    monkeypatch.setattr(ocr, "get_ocr", lambda paper: result)

    assert ocr.mod_size(np.zeros((100, 100, 3), dtype=np.uint8)) is None


def test_formula_returns_raw_bmp_and_no_content(monkeypatch):
    monkeypatch.setattr(api, "get_formula", lambda position: b"BMpayload")
    response = asyncio.run(api.get_formula_image((0, 0)))
    assert response.status_code == 200
    assert response.media_type == "image/bmp"
    assert response.body == b"BMpayload"

    monkeypatch.setattr(api, "get_formula", lambda position: None)
    response = asyncio.run(api.get_formula_image((0, 0)))
    assert response.status_code == 204
    assert response.body == b""


def test_answer_pending_ready_and_error_states(caplog):
    pending = get_answer_result(FakeTask(ready=False))
    ready = get_answer_result(FakeTask(result=ANSWER))
    error = get_answer_result(FakeTask(failed=True, result=RuntimeError("boom")))

    assert pending.model_dump(mode="json") == {
        "status": "pending",
        "answer": None,
        "error": None,
    }
    assert ready.status == "ready"
    assert ready.answer == AnswerResponse.model_validate(ANSWER)
    assert ready.error is None
    assert error.status == "error"
    assert error.answer is None
    assert error.error == "AI answer task failed"
    assert "boom" in caplog.text
    assert "worker traceback" in caplog.text


def test_expected_ranges_detect_leading_and_trailing_missing_questions():
    assert ocr.get_missing({"一": [2, 3, 4]}, {"一": (1, 5)}) == {"一": [1, 5]}
    assert ocr.get_missing({}, {"二": (6, 8)}) == {"二": [6, 7, 8]}


def test_missing_merges_pages_without_mutating_paper_info():
    exam = Exam(100.0, {"一": (1, 5)})
    exam.papers = {
        1: PaperInfo(info={"一": [2]}),
        2: PaperInfo(info={"unknown": [3], "一": [4]}),
    }

    assert exam.get_missing() == {"一": [1, 5]}
    assert exam.get_missing() == {"一": [1, 5]}
    assert exam.papers[1].info == {"一": [2]}
    assert exam.papers[2].info == {"unknown": [3], "一": [4]}
