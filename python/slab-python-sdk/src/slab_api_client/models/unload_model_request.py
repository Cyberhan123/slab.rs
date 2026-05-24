from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="UnloadModelRequest")


@_attrs_define
class UnloadModelRequest:
    """Request body for `POST /v1/models/unload`.

    Attributes:
        backend_id (None | str | Unset): Legacy backend identifier for direct runtime unloads.
        model_id (None | str | Unset): Catalog model id from `/v1/models`. Preferred for local lifecycle operations.
    """

    backend_id: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        backend_id: None | str | Unset
        if isinstance(self.backend_id, Unset):
            backend_id = UNSET
        else:
            backend_id = self.backend_id

        model_id: None | str | Unset
        if isinstance(self.model_id, Unset):
            model_id = UNSET
        else:
            model_id = self.model_id

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if backend_id is not UNSET:
            field_dict["backend_id"] = backend_id
        if model_id is not UNSET:
            field_dict["model_id"] = model_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_backend_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        backend_id = _parse_backend_id(d.pop("backend_id", UNSET))

        def _parse_model_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_id = _parse_model_id(d.pop("model_id", UNSET))

        unload_model_request = cls(
            backend_id=backend_id,
            model_id=model_id,
        )

        return unload_model_request
