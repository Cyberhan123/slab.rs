from enum import Enum


class AgentResponsesClientMessageType1Type(str, Enum):
    AGENT_RESPONSE_CREATE = "agent.response.create"

    def __str__(self) -> str:
        return str(self.value)
