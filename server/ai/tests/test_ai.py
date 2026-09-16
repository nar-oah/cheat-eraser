import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parents[1]))

import ai as ai_module
from cheat_eraser_contracts.answer import Answer


class FakeModels:
    def generate_content(self, **kwargs):
        return SimpleNamespace(text=None)


class FakeClient:
    models = FakeModels()


class FakePart:
    @staticmethod
    def from_bytes(**kwargs):
        return kwargs


def test_new_answer_request_clears_previous_answer(monkeypatch):
    value = ai_module.Ai()
    value.answer = Answer.model_validate(
        {
            "single_choice": ["A"],
            "multiple_choice": [],
            "binary_choice": [],
            "non_choice": [],
        }
    )
    monkeypatch.setattr(ai_module.genai, "Client", FakeClient)
    monkeypatch.setattr(ai_module.types, "Part", FakePart)

    value.get_answer([b"new paper"])

    assert value.answer is None


def test_formula_out_of_range_returns_none():
    value = ai_module.Ai()
    value.answer = Answer.model_validate(
        {
            "single_choice": [],
            "multiple_choice": [],
            "binary_choice": [],
            "non_choice": [{"answer": "text", "english": [], "math": []}],
        }
    )

    assert value.get_formula((0, 0)) is None
    assert value.get_formula((-1, 0)) is None
