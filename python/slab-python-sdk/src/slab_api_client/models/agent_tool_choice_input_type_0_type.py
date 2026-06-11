from enum import Enum


class AgentToolChoiceInputType0Type(str, Enum):
    AUTO = "auto"

    def __str__(self) -> str:
        return str(self.value)
