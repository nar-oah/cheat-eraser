from datetime import datetime
from pathlib import Path
from typing import Dict

import cv2

from pretreatment import PaperInfo, TestPaper


def save_exam(exam: TestPaper) -> None:
    target = Path("backup") / datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    target.mkdir(parents=True, exist_ok=True)
    _save_images(target, exam.papers)
    _save_info(target, exam.papers)


def _save_images(target: Path, papers: Dict[int, PaperInfo]) -> None:
    for key, paper in sorted(papers.items()):
        if paper.paper is not None:
            cv2.imwrite(str(target / f"{key}.png"), paper.paper)


def _save_info(target: Path, papers: Dict[int, PaperInfo]) -> None:
    with (target / "info").open("w", encoding="utf-8") as file:
        for key, paper in sorted(papers.items()):
            file.write(f"key: {key}\n")
            file.write(f"variance: {paper.variance}\n")
            file.write(f"info: {paper.info}\n")
