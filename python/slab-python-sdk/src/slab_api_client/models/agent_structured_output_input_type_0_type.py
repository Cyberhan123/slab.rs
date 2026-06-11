from enum import Enum


class AgentStructuredOutputInputType0Type(str, Enum):
    JSON_OBJECT = "json_object"

    def __str__(self) -> str:
        return str(self.value)
