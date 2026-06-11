from enum import Enum


class AgentToolChoiceInputType1Type(str, Enum):
    NONE = "none"

    def __str__(self) -> str:
        return str(self.value)
