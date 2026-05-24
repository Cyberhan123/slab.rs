from enum import Enum


class ModelConfigOriginResponse(str, Enum):
    DERIVED = "derived"
    PACK_MANIFEST = "pack_manifest"
    PMID_FALLBACK = "pmid_fallback"
    SELECTED_BACKEND_CONFIG = "selected_backend_config"
    SELECTED_PRESET = "selected_preset"
    SELECTED_VARIANT = "selected_variant"

    def __str__(self) -> str:
        return str(self.value)
