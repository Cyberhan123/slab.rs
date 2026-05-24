from enum import Enum


class PluginCapabilityKind(str, Enum):
    TOOL = "tool"
    WORKFLOW = "workflow"

    def __str__(self) -> str:
        return str(self.value)
