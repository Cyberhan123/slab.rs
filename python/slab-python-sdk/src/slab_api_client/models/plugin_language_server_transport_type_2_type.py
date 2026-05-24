from enum import Enum


class PluginLanguageServerTransportType2Type(str, Enum):
    NODEPACKAGE = "nodePackage"

    def __str__(self) -> str:
        return str(self.value)
