from enum import Enum


class AgentResponsesClientMessageType5Type(str, Enum):
    AGENT_SHUTDOWN = "agent.shutdown"

    def __str__(self) -> str:
        return str(self.value)
