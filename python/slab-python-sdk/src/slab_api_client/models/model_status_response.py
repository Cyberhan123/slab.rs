from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="ModelStatusResponse")


@_attrs_define
class ModelStatusResponse:
    """Response body for load / status endpoints.

    Attributes:
        backend (str): Backend identifier.
        status (str): Human-readable status string.
        context_length (int | None | Unset): Effective runtime context window length in tokens.
        training_context_length (int | None | Unset): Training context window length reported by the loaded model.
    """

    backend: str
    status: str
    context_length: int | None | Unset = UNSET
    training_context_length: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        backend = self.backend

        status = self.status

        context_length: int | None | Unset
        if isinstance(self.context_length, Unset):
            context_length = UNSET
        else:
            context_length = self.context_length

        training_context_length: int | None | Unset
        if isinstance(self.training_context_length, Unset):
            training_context_length = UNSET
        else:
            training_context_length = self.training_context_length

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "backend": backend,
                "status": status,
            }
        )
        if context_length is not UNSET:
            field_dict["context_length"] = context_length
        if training_context_length is not UNSET:
            field_dict["training_context_length"] = training_context_length

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        backend = d.pop("backend")

        status = d.pop("status")

        def _parse_context_length(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        context_length = _parse_context_length(d.pop("context_length", UNSET))

        def _parse_training_context_length(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        training_context_length = _parse_training_context_length(
            d.pop("training_context_length", UNSET)
        )

        model_status_response = cls(
            backend=backend,
            status=status,
            context_length=context_length,
            training_context_length=training_context_length,
        )

        model_status_response.additional_properties = d
        return model_status_response

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
