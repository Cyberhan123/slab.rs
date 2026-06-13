from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.model_capability import ModelCapability
from ..models.model_kind import ModelKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.chat_model_capabilities import ChatModelCapabilities
    from ..models.model_runtime_state_response import ModelRuntimeStateResponse
    from ..models.model_spec_response import ModelSpecResponse
    from ..models.runtime_presets_response import RuntimePresetsResponse


T = TypeVar("T", bound="UnifiedModelResponse")


@_attrs_define
class UnifiedModelResponse:
    """Unified model response returned by `/v1/models`.

    Attributes:
        capabilities (list[ModelCapability]):
        created_at (str):
        display_name (str):
        id (str):
        kind (ModelKind):
        spec (ModelSpecResponse): Provider-specific model configuration (response).
        status (str): Status: `"ready"`, `"not_downloaded"`, `"downloading"`, `"error"`.
        updated_at (str):
        backend_id (None | str | Unset): Runtime backend identifier for local models, e.g. `"ggml.llama"`.
        chat_capabilities (ChatModelCapabilities | None | Unset):
        runtime_presets (None | RuntimePresetsResponse | Unset):
        runtime_state (ModelRuntimeStateResponse | None | Unset):
    """

    capabilities: list[ModelCapability]
    created_at: str
    display_name: str
    id: str
    kind: ModelKind
    spec: ModelSpecResponse
    status: str
    updated_at: str
    backend_id: None | str | Unset = UNSET
    chat_capabilities: ChatModelCapabilities | None | Unset = UNSET
    runtime_presets: None | RuntimePresetsResponse | Unset = UNSET
    runtime_state: ModelRuntimeStateResponse | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.chat_model_capabilities import ChatModelCapabilities
        from ..models.model_runtime_state_response import ModelRuntimeStateResponse
        from ..models.runtime_presets_response import RuntimePresetsResponse

        capabilities = []
        for capabilities_item_data in self.capabilities:
            capabilities_item = capabilities_item_data.value
            capabilities.append(capabilities_item)

        created_at = self.created_at

        display_name = self.display_name

        id = self.id

        kind = self.kind.value

        spec = self.spec.to_dict()

        status = self.status

        updated_at = self.updated_at

        backend_id: None | str | Unset
        if isinstance(self.backend_id, Unset):
            backend_id = UNSET
        else:
            backend_id = self.backend_id

        chat_capabilities: dict[str, Any] | None | Unset
        if isinstance(self.chat_capabilities, Unset):
            chat_capabilities = UNSET
        elif isinstance(self.chat_capabilities, ChatModelCapabilities):
            chat_capabilities = self.chat_capabilities.to_dict()
        else:
            chat_capabilities = self.chat_capabilities

        runtime_presets: dict[str, Any] | None | Unset
        if isinstance(self.runtime_presets, Unset):
            runtime_presets = UNSET
        elif isinstance(self.runtime_presets, RuntimePresetsResponse):
            runtime_presets = self.runtime_presets.to_dict()
        else:
            runtime_presets = self.runtime_presets

        runtime_state: dict[str, Any] | None | Unset
        if isinstance(self.runtime_state, Unset):
            runtime_state = UNSET
        elif isinstance(self.runtime_state, ModelRuntimeStateResponse):
            runtime_state = self.runtime_state.to_dict()
        else:
            runtime_state = self.runtime_state

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "capabilities": capabilities,
                "created_at": created_at,
                "display_name": display_name,
                "id": id,
                "kind": kind,
                "spec": spec,
                "status": status,
                "updated_at": updated_at,
            }
        )
        if backend_id is not UNSET:
            field_dict["backend_id"] = backend_id
        if chat_capabilities is not UNSET:
            field_dict["chat_capabilities"] = chat_capabilities
        if runtime_presets is not UNSET:
            field_dict["runtime_presets"] = runtime_presets
        if runtime_state is not UNSET:
            field_dict["runtime_state"] = runtime_state

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.chat_model_capabilities import ChatModelCapabilities
        from ..models.model_runtime_state_response import ModelRuntimeStateResponse
        from ..models.model_spec_response import ModelSpecResponse
        from ..models.runtime_presets_response import RuntimePresetsResponse

        d = dict(src_dict)
        capabilities = []
        _capabilities = d.pop("capabilities")
        for capabilities_item_data in _capabilities:
            capabilities_item = ModelCapability(capabilities_item_data)

            capabilities.append(capabilities_item)

        created_at = d.pop("created_at")

        display_name = d.pop("display_name")

        id = d.pop("id")

        kind = ModelKind(d.pop("kind"))

        spec = ModelSpecResponse.from_dict(d.pop("spec"))

        status = d.pop("status")

        updated_at = d.pop("updated_at")

        def _parse_backend_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        backend_id = _parse_backend_id(d.pop("backend_id", UNSET))

        def _parse_chat_capabilities(
            data: object,
        ) -> ChatModelCapabilities | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                chat_capabilities_type_1 = ChatModelCapabilities.from_dict(data)

                return chat_capabilities_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ChatModelCapabilities | None | Unset, data)

        chat_capabilities = _parse_chat_capabilities(d.pop("chat_capabilities", UNSET))

        def _parse_runtime_presets(
            data: object,
        ) -> None | RuntimePresetsResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                runtime_presets_type_1 = RuntimePresetsResponse.from_dict(data)

                return runtime_presets_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | RuntimePresetsResponse | Unset, data)

        runtime_presets = _parse_runtime_presets(d.pop("runtime_presets", UNSET))

        def _parse_runtime_state(
            data: object,
        ) -> ModelRuntimeStateResponse | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                runtime_state_type_1 = ModelRuntimeStateResponse.from_dict(data)

                return runtime_state_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ModelRuntimeStateResponse | None | Unset, data)

        runtime_state = _parse_runtime_state(d.pop("runtime_state", UNSET))

        unified_model_response = cls(
            capabilities=capabilities,
            created_at=created_at,
            display_name=display_name,
            id=id,
            kind=kind,
            spec=spec,
            status=status,
            updated_at=updated_at,
            backend_id=backend_id,
            chat_capabilities=chat_capabilities,
            runtime_presets=runtime_presets,
            runtime_state=runtime_state,
        )

        unified_model_response.additional_properties = d
        return unified_model_response

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
