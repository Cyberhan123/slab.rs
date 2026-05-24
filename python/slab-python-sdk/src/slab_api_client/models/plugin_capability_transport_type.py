from enum import Enum


class PluginCapabilityTransportType(str, Enum):
    PLUGINCALL = "pluginCall"

    def __str__(self) -> str:
        return str(self.value)
