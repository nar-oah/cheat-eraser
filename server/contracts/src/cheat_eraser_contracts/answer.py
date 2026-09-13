from typing import Self
from pydantic import BaseModel, Field


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
    english: list[str] = Field(
        title="英语单词或语句",
        description="仅在后面的内容不是英语时拆分，如果一句话都是英文则存储整句",
    )


class WordProblems(WordBase):
    math: list[str] = Field(title="数学公式", description="输出为latex")


class WordResponse(WordBase):
    math: int

    @classmethod
    def get_response(cls, obj: WordProblems) -> Self:
        data = obj.model_dump()
        data["math"] = len(obj.math)
        return cls(**data)


class AnswerBase(BaseModel):
    single_choice: list[str] = Field(
        title="单选题", description='每题输出一个最佳选项字母（如 "A"）'
    )
    multiple_choice: list[str] = Field(
        title="多选题", description='每题输出多个最佳选项字母（如 "ABC"）'
    )
    binary_choice: list[bool] = Field(title="判断题", description="每题输出判断结果")


class Answer(AnswerBase):
    non_choice: list[WordProblems] = Field(
        title="非选择题", description="每题输出简洁的标准答案"
    )


class AnswerResponse(AnswerBase):
    non_choice: list[WordResponse]

    @classmethod
    def get_response(cls, obj: Answer) -> Self:
        data = obj.model_dump()
        data["non_choice"] = list(map(WordResponse.get_response, obj.non_choice))
        return cls(**data)
