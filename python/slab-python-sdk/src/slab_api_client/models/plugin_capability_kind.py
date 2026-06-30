from enum import Enum


class PluginCapabilityKind(str, Enum):
    A2U_SURFACE = "a2u_surface"
    TOOL = "tool"
    WORKFLOW = "workflow"

    def __str__(self) -> str:
        return str(self.value)
