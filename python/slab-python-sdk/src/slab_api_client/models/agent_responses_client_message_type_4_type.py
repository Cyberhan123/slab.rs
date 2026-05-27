from enum import Enum


class AgentResponsesClientMessageType4Type(str, Enum):
    AGENT_INTERRUPT = "agent.interrupt"

    def __str__(self) -> str:
        return str(self.value)
