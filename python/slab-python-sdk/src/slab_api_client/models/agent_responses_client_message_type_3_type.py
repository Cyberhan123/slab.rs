from enum import Enum


class AgentResponsesClientMessageType3Type(str, Enum):
    AGENT_APPROVAL_RESOLVE = "agent.approval.resolve"

    def __str__(self) -> str:
        return str(self.value)
