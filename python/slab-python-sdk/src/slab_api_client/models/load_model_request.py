from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="LoadModelRequest")


@_attrs_define
class LoadModelRequest:
    """Request body for `POST /v1/models/load`.

    Attributes:
        backend_id (None | str | Unset): Legacy backend identifier, e.g. `"ggml.llama"`.
        model_id (None | str | Unset): Catalog model id from `/v1/models`. Preferred for local lifecycle operations.
        model_path (None | str | Unset): Legacy path to the model weights file.
        num_workers (int | None | Unset): Optional worker override.
    """

    backend_id: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET
    model_path: None | str | Unset = UNSET
    num_workers: int | None | Unset = UNSET

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

        model_path: None | str | Unset
        if isinstance(self.model_path, Unset):
            model_path = UNSET
        else:
            model_path = self.model_path

        num_workers: int | None | Unset
        if isinstance(self.num_workers, Unset):
            num_workers = UNSET
        else:
            num_workers = self.num_workers

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if backend_id is not UNSET:
            field_dict["backend_id"] = backend_id
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if model_path is not UNSET:
            field_dict["model_path"] = model_path
        if num_workers is not UNSET:
            field_dict["num_workers"] = num_workers

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

        def _parse_model_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_path = _parse_model_path(d.pop("model_path", UNSET))

        def _parse_num_workers(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        num_workers = _parse_num_workers(d.pop("num_workers", UNSET))

        load_model_request = cls(
            backend_id=backend_id,
            model_id=model_id,
            model_path=model_path,
            num_workers=num_workers,
        )

        return load_model_request
