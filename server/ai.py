from google import genai
from google.genai import types
from typing import List, Optional, Tuple
from pydantic import BaseModel, Field
import ziamath as zm
from PIL import Image
import cairosvg
import io


class WordBase(BaseModel):
    answer: str = Field(
        title="答案",
        description="""
    仅填写中文和占位符
    占位符格式：
    - 所有数学公式按照顺序用$(数字)占位，具体的数学公式用latex的形式输出在math项。
    - 所有英文单词和句子按照顺序用$[数字]占位，具体的英文单词或句子输出在english项。
    """,
    )
    english: List[str] = Field(
        title="英语单词或语句",
        description="仅在后面的内容不是英语时拆分，如果一句话都是英文则存储整句",
    )


class WordProblems(WordBase):
    math: List[str] = Field(title="数学公式", description="输出为latex")


class WordResponse(WordBase):
    math: int

    @classmethod
    def get_response(cls, obj: WordProblems):
        data = obj.model_dump()
        data["math"] = len(obj.math)
        return cls(**data)


class AnswerBase(BaseModel):
    single_choice: List[str] = Field(
        title="单选题", description='每题输出一个最佳选项字母（如 "A"）'
    )
    multiple_choice: List[str] = Field(
        title="多选题", description='每题输出多个最佳选项字母（如 "ABC"）'
    )
    binary_choice: List[bool] = Field(title="判断题", description="每题输出判断结果")


class Answer(AnswerBase):
    non_choice: List[WordProblems] = Field(
        title="非选择题", description="每题输出简洁的标准答案"
    )


class AnswerResponse(AnswerBase):
    non_choice: List[WordResponse]

    @classmethod
    def get_response(cls, obj: Answer):
        data = obj.model_dump()
        words = [WordResponse.get_response(word) for word in obj.non_choice]
        data["non_choice"] = words
        return cls(**data)


class Ai:
    def __init__(self) -> None:
        self.answer: Optional[Answer] = None

    def get_answer(self, images: List[bytes]) -> None:
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
