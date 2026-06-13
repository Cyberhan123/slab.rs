from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="ModelRuntimeStateResponse")


@_attrs_define
class ModelRuntimeStateResponse:
    """Runtime lifecycle state for a local catalog model.

    Attributes:
        active (bool): Whether this catalog model is currently serving an inference request.
        active_refs (int): Number of active inference references on the backend.
        backend_id (str): Runtime backend identifier for local models, e.g. `"ggml.llama"`.
        loaded (bool): Whether this catalog model is currently resident in its runtime backend.
    """

    active: bool
    active_refs: int
    backend_id: str
    loaded: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        active = self.active

        active_refs = self.active_refs

        backend_id = self.backend_id

        loaded = self.loaded

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "active": active,
                "active_refs": active_refs,
                "backend_id": backend_id,
                "loaded": loaded,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        active = d.pop("active")

        active_refs = d.pop("active_refs")

        backend_id = d.pop("backend_id")

        loaded = d.pop("loaded")

        model_runtime_state_response = cls(
            active=active,
            active_refs=active_refs,
            backend_id=backend_id,
            loaded=loaded,
        )

        model_runtime_state_response.additional_properties = d
        return model_runtime_state_response

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
