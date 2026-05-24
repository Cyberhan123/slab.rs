from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define

from ..models.model_capability import ModelCapability
from ..models.model_kind import ModelKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.model_spec_request import ModelSpecRequest
    from ..models.runtime_presets_request import RuntimePresetsRequest


T = TypeVar("T", bound="CreateModelRequest")


@_attrs_define
class CreateModelRequest:
    """Request body for `POST /v1/models`.

    Attributes:
        display_name (str):
        kind (ModelKind):
        backend_id (None | str | Unset): Runtime backend identifier for local models, e.g. `"ggml.llama"`.
        capabilities (list[ModelCapability] | None | Unset):
        runtime_presets (None | RuntimePresetsRequest | Unset):
        spec (ModelSpecRequest | None | Unset):
        status (None | str | Unset): Initial status. If omitted, defaults to `"ready"` for cloud models and
            `"not_downloaded"` for local models.
    """

    display_name: str
    kind: ModelKind
    backend_id: None | str | Unset = UNSET
    capabilities: list[ModelCapability] | None | Unset = UNSET
    runtime_presets: None | RuntimePresetsRequest | Unset = UNSET
    spec: ModelSpecRequest | None | Unset = UNSET
    status: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        from ..models.model_spec_request import ModelSpecRequest
        from ..models.runtime_presets_request import RuntimePresetsRequest

        display_name = self.display_name

        kind = self.kind.value

        backend_id: None | str | Unset
        if isinstance(self.backend_id, Unset):
            backend_id = UNSET
        else:
            backend_id = self.backend_id

        capabilities: list[str] | None | Unset
        if isinstance(self.capabilities, Unset):
            capabilities = UNSET
        elif isinstance(self.capabilities, list):
            capabilities = []
            for capabilities_type_0_item_data in self.capabilities:
                capabilities_type_0_item = capabilities_type_0_item_data.value
                capabilities.append(capabilities_type_0_item)

        else:
            capabilities = self.capabilities

        runtime_presets: dict[str, Any] | None | Unset
        if isinstance(self.runtime_presets, Unset):
            runtime_presets = UNSET
        elif isinstance(self.runtime_presets, RuntimePresetsRequest):
            runtime_presets = self.runtime_presets.to_dict()
        else:
            runtime_presets = self.runtime_presets

        spec: dict[str, Any] | None | Unset
        if isinstance(self.spec, Unset):
            spec = UNSET
        elif isinstance(self.spec, ModelSpecRequest):
            spec = self.spec.to_dict()
        else:
            spec = self.spec

        status: None | str | Unset
        if isinstance(self.status, Unset):
            status = UNSET
        else:
            status = self.status

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "display_name": display_name,
                "kind": kind,
            }
        )
        if backend_id is not UNSET:
            field_dict["backend_id"] = backend_id
        if capabilities is not UNSET:
            field_dict["capabilities"] = capabilities
        if runtime_presets is not UNSET:
            field_dict["runtime_presets"] = runtime_presets
        if spec is not UNSET:
            field_dict["spec"] = spec
        if status is not UNSET:
            field_dict["status"] = status

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.model_spec_request import ModelSpecRequest
        from ..models.runtime_presets_request import RuntimePresetsRequest

        d = dict(src_dict)
        display_name = d.pop("display_name")

        kind = ModelKind(d.pop("kind"))

        def _parse_backend_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        backend_id = _parse_backend_id(d.pop("backend_id", UNSET))

        def _parse_capabilities(data: object) -> list[ModelCapability] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                capabilities_type_0 = []
                _capabilities_type_0 = data
                for capabilities_type_0_item_data in _capabilities_type_0:
                    capabilities_type_0_item = ModelCapability(
                        capabilities_type_0_item_data
                    )

                    capabilities_type_0.append(capabilities_type_0_item)

                return capabilities_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[ModelCapability] | None | Unset, data)

        capabilities = _parse_capabilities(d.pop("capabilities", UNSET))

        def _parse_runtime_presets(
            data: object,
        ) -> None | RuntimePresetsRequest | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                runtime_presets_type_1 = RuntimePresetsRequest.from_dict(data)

                return runtime_presets_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | RuntimePresetsRequest | Unset, data)

        runtime_presets = _parse_runtime_presets(d.pop("runtime_presets", UNSET))

        def _parse_spec(data: object) -> ModelSpecRequest | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                spec_type_1 = ModelSpecRequest.from_dict(data)

                return spec_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ModelSpecRequest | None | Unset, data)

        spec = _parse_spec(d.pop("spec", UNSET))

        def _parse_status(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        status = _parse_status(d.pop("status", UNSET))

        create_model_request = cls(
            display_name=display_name,
            kind=kind,
            backend_id=backend_id,
            capabilities=capabilities,
            runtime_presets=runtime_presets,
            spec=spec,
            status=status,
        )

        return create_model_request
