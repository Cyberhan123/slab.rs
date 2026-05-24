from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="TranscribeVadOptionsResponse")


@_attrs_define
class TranscribeVadOptionsResponse:
    """
    Attributes:
        enabled (bool):
        max_speech_duration_s (float | None | Unset):
        min_silence_duration_ms (int | None | Unset):
        min_speech_duration_ms (int | None | Unset):
        model_path (None | str | Unset):
        samples_overlap (float | None | Unset):
        speech_pad_ms (int | None | Unset):
        threshold (float | None | Unset):
    """

    enabled: bool
    max_speech_duration_s: float | None | Unset = UNSET
    min_silence_duration_ms: int | None | Unset = UNSET
    min_speech_duration_ms: int | None | Unset = UNSET
    model_path: None | str | Unset = UNSET
    samples_overlap: float | None | Unset = UNSET
    speech_pad_ms: int | None | Unset = UNSET
    threshold: float | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        enabled = self.enabled

        max_speech_duration_s: float | None | Unset
        if isinstance(self.max_speech_duration_s, Unset):
            max_speech_duration_s = UNSET
        else:
            max_speech_duration_s = self.max_speech_duration_s

        min_silence_duration_ms: int | None | Unset
        if isinstance(self.min_silence_duration_ms, Unset):
            min_silence_duration_ms = UNSET
        else:
            min_silence_duration_ms = self.min_silence_duration_ms

        min_speech_duration_ms: int | None | Unset
        if isinstance(self.min_speech_duration_ms, Unset):
            min_speech_duration_ms = UNSET
        else:
            min_speech_duration_ms = self.min_speech_duration_ms

        model_path: None | str | Unset
        if isinstance(self.model_path, Unset):
            model_path = UNSET
        else:
            model_path = self.model_path

        samples_overlap: float | None | Unset
        if isinstance(self.samples_overlap, Unset):
            samples_overlap = UNSET
        else:
            samples_overlap = self.samples_overlap

        speech_pad_ms: int | None | Unset
        if isinstance(self.speech_pad_ms, Unset):
            speech_pad_ms = UNSET
        else:
            speech_pad_ms = self.speech_pad_ms

        threshold: float | None | Unset
        if isinstance(self.threshold, Unset):
            threshold = UNSET
        else:
            threshold = self.threshold

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "enabled": enabled,
            }
        )
        if max_speech_duration_s is not UNSET:
            field_dict["max_speech_duration_s"] = max_speech_duration_s
        if min_silence_duration_ms is not UNSET:
            field_dict["min_silence_duration_ms"] = min_silence_duration_ms
        if min_speech_duration_ms is not UNSET:
            field_dict["min_speech_duration_ms"] = min_speech_duration_ms
        if model_path is not UNSET:
            field_dict["model_path"] = model_path
        if samples_overlap is not UNSET:
            field_dict["samples_overlap"] = samples_overlap
        if speech_pad_ms is not UNSET:
            field_dict["speech_pad_ms"] = speech_pad_ms
        if threshold is not UNSET:
            field_dict["threshold"] = threshold

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        enabled = d.pop("enabled")

        def _parse_max_speech_duration_s(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        max_speech_duration_s = _parse_max_speech_duration_s(
            d.pop("max_speech_duration_s", UNSET)
        )

        def _parse_min_silence_duration_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        min_silence_duration_ms = _parse_min_silence_duration_ms(
            d.pop("min_silence_duration_ms", UNSET)
        )

        def _parse_min_speech_duration_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        min_speech_duration_ms = _parse_min_speech_duration_ms(
            d.pop("min_speech_duration_ms", UNSET)
        )

        def _parse_model_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_path = _parse_model_path(d.pop("model_path", UNSET))

        def _parse_samples_overlap(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        samples_overlap = _parse_samples_overlap(d.pop("samples_overlap", UNSET))

        def _parse_speech_pad_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        speech_pad_ms = _parse_speech_pad_ms(d.pop("speech_pad_ms", UNSET))

        def _parse_threshold(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        threshold = _parse_threshold(d.pop("threshold", UNSET))

        transcribe_vad_options_response = cls(
            enabled=enabled,
            max_speech_duration_s=max_speech_duration_s,
            min_silence_duration_ms=min_silence_duration_ms,
            min_speech_duration_ms=min_speech_duration_ms,
            model_path=model_path,
            samples_overlap=samples_overlap,
            speech_pad_ms=speech_pad_ms,
            threshold=threshold,
        )

        transcribe_vad_options_response.additional_properties = d
        return transcribe_vad_options_response

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
