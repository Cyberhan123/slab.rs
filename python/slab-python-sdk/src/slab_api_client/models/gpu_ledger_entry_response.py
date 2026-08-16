from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field
from typing_extensions import Self

from ..types import UNSET, Unset

T = TypeVar("T", bound="GpuLedgerEntryResponse")


@_attrs_define
class GpuLedgerEntryResponse:
    """One resident model's ledger entry (diagnostics). The ledger is
    attribution — probe-measured free bytes remain the decision input.

        Attributes:
            backend (str): Backend canonical id, e.g. "ggml.llama".
            mmproj_resident (bool): Whether a multimodal projector is resident alongside the model.
            model_path (str): Model weights file path as loaded.
            num_workers (int):
            recorded_at (str): RFC3339 timestamp of the recorded load.
            measured_delta_bytes (int | None | Unset): Measured free-VRAM delta across the load (probe before vs after).
            mmproj_bytes (int | None | Unset): Projector file size in bytes (best-effort stat).
            model_id (None | str | Unset): Model id when the load was dispatched for a catalog model.
            resolved_context_length (int | None | Unset): Engine-resolved `n_ctx` (what `auto` sized to), when reported.
            weights_bytes (int | None | Unset): Weights file size in bytes (best-effort stat).
    """

    backend: str
    mmproj_resident: bool
    model_path: str
    num_workers: int
    recorded_at: str
    measured_delta_bytes: int | None | Unset = UNSET
    mmproj_bytes: int | None | Unset = UNSET
    model_id: None | str | Unset = UNSET
    resolved_context_length: int | None | Unset = UNSET
    weights_bytes: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        backend = self.backend

        mmproj_resident = self.mmproj_resident

        model_path = self.model_path

        num_workers = self.num_workers

        recorded_at = self.recorded_at

        measured_delta_bytes: int | None | Unset
        if isinstance(self.measured_delta_bytes, Unset):
            measured_delta_bytes = UNSET
        else:
            measured_delta_bytes = self.measured_delta_bytes

        mmproj_bytes: int | None | Unset
        if isinstance(self.mmproj_bytes, Unset):
            mmproj_bytes = UNSET
        else:
            mmproj_bytes = self.mmproj_bytes

        model_id: None | str | Unset
        if isinstance(self.model_id, Unset):
            model_id = UNSET
        else:
            model_id = self.model_id

        resolved_context_length: int | None | Unset
        if isinstance(self.resolved_context_length, Unset):
            resolved_context_length = UNSET
        else:
            resolved_context_length = self.resolved_context_length

        weights_bytes: int | None | Unset
        if isinstance(self.weights_bytes, Unset):
            weights_bytes = UNSET
        else:
            weights_bytes = self.weights_bytes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "backend": backend,
                "mmproj_resident": mmproj_resident,
                "model_path": model_path,
                "num_workers": num_workers,
                "recorded_at": recorded_at,
            }
        )
        if measured_delta_bytes is not UNSET:
            field_dict["measured_delta_bytes"] = measured_delta_bytes
        if mmproj_bytes is not UNSET:
            field_dict["mmproj_bytes"] = mmproj_bytes
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if resolved_context_length is not UNSET:
            field_dict["resolved_context_length"] = resolved_context_length
        if weights_bytes is not UNSET:
            field_dict["weights_bytes"] = weights_bytes

        return field_dict

    @classmethod
    def from_dict(cls, src_dict: Mapping[str, Any]) -> Self:
        d = dict(src_dict)
        backend = d.pop("backend")

        mmproj_resident = d.pop("mmproj_resident")

        model_path = d.pop("model_path")

        num_workers = d.pop("num_workers")

        recorded_at = d.pop("recorded_at")

        def _parse_measured_delta_bytes(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        measured_delta_bytes = _parse_measured_delta_bytes(
            d.pop("measured_delta_bytes", UNSET)
        )

        def _parse_mmproj_bytes(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        mmproj_bytes = _parse_mmproj_bytes(d.pop("mmproj_bytes", UNSET))

        def _parse_model_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_id = _parse_model_id(d.pop("model_id", UNSET))

        def _parse_resolved_context_length(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        resolved_context_length = _parse_resolved_context_length(
            d.pop("resolved_context_length", UNSET)
        )

        def _parse_weights_bytes(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        weights_bytes = _parse_weights_bytes(d.pop("weights_bytes", UNSET))

        gpu_ledger_entry_response = cls(
            backend=backend,
            mmproj_resident=mmproj_resident,
            model_path=model_path,
            num_workers=num_workers,
            recorded_at=recorded_at,
            measured_delta_bytes=measured_delta_bytes,
            mmproj_bytes=mmproj_bytes,
            model_id=model_id,
            resolved_context_length=resolved_context_length,
            weights_bytes=weights_bytes,
        )

        gpu_ledger_entry_response.additional_properties = d
        return gpu_ledger_entry_response

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
