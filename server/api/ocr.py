from typing import Any, Dict, List, Optional, Tuple
import numpy as np
from numpy._typing import NDArray
from rapidocr_onnxruntime import RapidOCR
import cv2
import re

ocr_engine = RapidOCR()


def get_ocr(paper: np.ndarray) -> Optional[List[List[Any | str]]]:
    result, _ = ocr_engine(paper, use_det=True, use_cls=False, use_rec=True)
    return result if result else None


def get_info(paper: np.ndarray) -> Optional[Dict[str, List[int]]]:
    if (res := get_ocr(paper)) is None:
        return None
    sorted_result = sorted(res, key=lambda x: x[0][0][1])
    info_dict: Dict[str, List[int]] = {}
    current_section: str = "unknown"
    pattern_big = re.compile(r"^([一二三四五六七八九十]+)\s*[、.．\s]")
    pattern_small = re.compile(r"^(\d+)[.、．](?!\d)")
    for line in sorted_result:
        text = str(line[1])
        big_match = pattern_big.match(text)
        if big_match:
            current_section = big_match.group(1)
            info_dict.setdefault(current_section, [])
            continue
        small_match = pattern_small.match(text)
        if small_match:
            num = int(small_match.group(1))
            info_dict.setdefault(current_section, [])
            info_dict[current_section].append(num)
    return info_dict if info_dict else None


def get_missing(info: Dict[str, List[int]]) -> Dict[str, List[int]]:
    missing_dict: Dict[str, List[int]] = {}
    for section, numbers in info.items():
        if len(numbers) != 0:
            start: int = numbers[0]
            end: int = numbers[-1]
            full_range: set[int] = set(range(start, end + 1))
            actual_numbers: set[int] = set(numbers)
            missing: List[int] = sorted(list(full_range - actual_numbers))
            missing_dict.setdefault(section, missing)
    return missing_dict


def mod_size(paper: np.ndarray) -> Optional[Tuple[np.ndarray, int]]:
    if (result := get_ocr(paper)) is None:
        return None
    (_, origin) = paper.shape[:2]
    pat_footer = re.compile(r"第\s*(\d+)\s*页|共\s*\d+\s*页")
    footers: List[Tuple[Tuple[np.float32, np.float32], int]] = []
    for line in result:
        text = str(line[1])
        match = pat_footer.search(text)
        if match:
            box: NDArray[np.float32] = np.array(line[0], dtype=np.float32)
            xs: NDArray[np.float32] = box[:, 0]
            value: Tuple[np.float32, np.float32] = np.mean(xs), max(xs) - min(xs)
            footers.append((value, int(match.group(1))))
    if not footers:
        return None
    left_footer: Tuple[Tuple[np.float32, np.float32], int] = min(footers)
    (center, width), page = left_footer
    offset: np.float32 = (width * 7) / 2
    start: int = int(max(0, center - offset))
    end: int = int(min(origin, center + offset))
    return (paper[:, start:end], page)


def del_header(paper: np.ndarray) -> Optional[np.ndarray]:
    rot = cv2.rotate(paper, cv2.ROTATE_90_CLOCKWISE)
    if (result := get_ocr(rot)) is None:
        return None
    (height, _) = paper.shape[:2]
    pat_sealed = re.compile(r"密\s*[-._~]*\s*封\s*[-._~]*\s*线")
    for line in result:
        text = str(line[1])
        if pat_sealed.search(text):
            box: NDArray[np.float32] = np.array(line[0], dtype=np.float32)
            ys: NDArray[np.float32] = box[:, 1]
            center: np.float32 = np.mean(ys)
            top: int = int(max(0, int(min(ys)) - 50))
            bottom: int = int(min(height, int(max(ys)) + 50))
            is_half: np.bool = center < (height / 2)
            rot: np.ndarray = rot[bottom:, :] if is_half else rot[:top, :]
            break
    final_img: np.ndarray = cv2.rotate(rot, cv2.ROTATE_90_COUNTERCLOCKWISE)
    return final_img
