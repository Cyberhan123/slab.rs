from enum import Enum


class AgentToolChoiceInputType3Type(str, Enum):
    TOOL = "tool"

    def __str__(self) -> str:
        return str(self.value)
