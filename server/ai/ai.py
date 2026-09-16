from google import genai
from google.genai import types
from typing import List, Optional, Tuple
from cheat_eraser_contracts.answer import Answer
import ziamath as zm
from PIL import Image
import cairosvg
import io


class Ai:
    def __init__(self) -> None:
        self.answer: Optional[Answer] = None

    def get_answer(self, images: List[bytes]) -> None:
        self.answer = None
        client = genai.Client()
        response = client.models.generate_content(
            model="gemini-3.5-flash",
            contents=[
                [
                    types.Part.from_bytes(data=image, mime_type="image/jpeg")
                    for image in images
                ],
                "我将上传多张卷子照片，按照大题类型分类后，按照小题顺序输出答案到对应类别",
            ],
            config={
                "response_mime_type": "application/json",
                "response_json_schema": Answer.model_json_schema(),
            },
        )
        if response.text is not None:
            self.answer = Answer.model_validate_json(response.text)

    def get_formula(self, position: Tuple[int, int]) -> Optional[bytes]:
        if self.answer:
            latex = self.answer.non_choice[position[0]].math[position[1]]
            math = zm.zmath.Latex(latex, color="white", size=24)
            svg = math.svg()
            img = cairosvg.svg2png(
                bytestring=svg.encode("utf-8"), output_width=200, output_height=67
            )
            if img:
                img = Image.open(io.BytesIO(img)).convert("RGBA")
                bg = Image.new("RGBA", img.size, (0, 0, 0, 255))
                bg.paste(img, mask=img.split()[3])
                gray = bg.convert("L")
                bw = gray.convert("1")
                arr = io.BytesIO()
                bw.save(arr, format="BMP")
                return arr.getvalue()
        return None


if __name__ == "__main__":
    ai = Ai()

    with open("output/filter3.png", "rb") as f:
        image_bytes = f.read()
        ai.get_answer([image_bytes])
    formula = ai.get_formula((2, 2))
    if formula:
        with open("output/formula.bmp", "wb") as f:
            f.write(formula)
