from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="DownloadModelRequest")


@_attrs_define
class DownloadModelRequest:
    """Request body for `POST /v1/models/download`.

    Attributes:
        model_id (str): Model ID from `/v1/models`.
    """

    model_id: str

    def to_dict(self) -> dict[str, Any]:
        model_id = self.model_id

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "model_id": model_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        model_id = d.pop("model_id")

        download_model_request = cls(
            model_id=model_id,
        )

        return download_model_request
