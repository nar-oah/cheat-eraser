from dataclasses import dataclass, field
from typing import Dict, List, Optional
import numpy as np
import cv2
from ocr import get_info, mod_size, del_header, get_missing


@dataclass
class PaperInfo:
    variance: float = float("-inf")
    paper: Optional[np.ndarray] = None
    info: Dict[str, List[int]] = field(default_factory=dict)


class TestPaper:
    def __init__(self, threshold: float) -> None:
        self.threshold: float = threshold
        self.papers: Dict[int, PaperInfo] = {}

    def _get_ndarray(self, image: bytes) -> Optional[np.ndarray]:
        buf: np.ndarray = np.frombuffer(image, np.uint8)
        return cv2.imdecode(buf, cv2.IMREAD_COLOR)

    def _get_variance(self, paper: np.ndarray) -> float:
        gray = cv2.cvtColor(paper, cv2.COLOR_BGR2GRAY)
        return cv2.Laplacian(gray, cv2.CV_64F).var()

    def add_paper(self, image: bytes) -> None:
        if (paper := self._get_ndarray(image)) is None:
            return None
        variance: float = self._get_variance(paper)
        if variance < self.threshold:
            return None
        if (res := mod_size(paper)) is None:
            return None
        paper, page = res
        value = self.papers.get(page, PaperInfo())
        if (paper := del_header(paper)) is None:
            return None
        if (info := get_info(paper)) is None:
            return None
        self.papers[page] = max(
            value, PaperInfo(variance, paper, info), key=lambda x: x.variance
        )

    def get_papers(self) -> List[PaperInfo]:
        return [paper for _, paper in sorted(self.papers.items())]

    def get_missing(self) -> Dict[str, List[int]]:
        res: Dict[str, List[int]] = {}
        papers = self.get_papers()
        for paper in papers:
            missing = paper.info.copy()
            last_key = next(reversed(res), None)
            last_value = missing.pop("unknown", [])
            if last_key:
                res.setdefault(last_key, []).extend(last_value)
            res.update(missing)
        return get_missing(res)


if __name__ == "__main__":
    import os

    exam = TestPaper(threshold=100.0)
    source_path = os.path.join(os.getcwd(), "saved_images")
    for filename in os.listdir(source_path):
        print(f"Processing: {filename}")
        file_path = os.path.join(source_path, filename)
        with open(file_path, "rb") as f:
            image_bytes = f.read()
        exam.add_paper(image_bytes)
    print(f"get_missing result: {exam.get_missing()}")

    target_path = os.path.join(os.getcwd(), "output")
    papers: List[np.ndarray] = [
        paper.paper for paper in exam.get_papers() if paper.paper is not None
    ]
    for idx, img in enumerate(papers):
        filename = f"filter{idx + 1}.png"
        file_path = os.path.join(target_path, filename)
        cv2.imwrite(file_path, img)
        print(f"Saved: {file_path}")
