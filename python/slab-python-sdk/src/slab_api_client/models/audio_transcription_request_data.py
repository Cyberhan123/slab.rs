from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.transcribe_decode_options_response import (
        TranscribeDecodeOptionsResponse,
    )
    from ..models.transcribe_vad_options_response import TranscribeVadOptionsResponse


T = TypeVar("T", bound="AudioTranscriptionRequestData")


@_attrs_define
class AudioTranscriptionRequestData:
    """
    Attributes:
        source_path (str):
        decode (None | TranscribeDecodeOptionsResponse | Unset):
        detect_language (bool | None | Unset):
        language (None | str | Unset):
        model_id (None | str | Unset):
        prompt (None | str | Unset):
        vad (None | TranscribeVadOptionsResponse | Unset):
    """

    source_path: str
    decode: None | TranscribeDecodeOptionsResponse | Unset = UNSET
    detect_language: bool | None | Unset = UNSET
    language: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET
    prompt: None | str | Unset = UNSET
    vad: None | TranscribeVadOptionsResponse | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.transcribe_decode_options_response import (
            TranscribeDecodeOptionsResponse,
        )
        from ..models.transcribe_vad_options_response import (
            TranscribeVadOptionsResponse,
        )

        source_path = self.source_path

        decode: dict[str, Any] | None | Unset
        if isinstance(self.decode, Unset):
            decode = UNSET
        elif isinstance(self.decode, TranscribeDecodeOptionsResponse):
            decode = self.decode.to_dict()
        else:
            decode = self.decode

        detect_language: bool | None | Unset
        if isinstance(self.detect_language, Unset):
            detect_language = UNSET
        else:
            detect_language = self.detect_language

        language: None | str | Unset
        if isinstance(self.language, Unset):
            language = UNSET
        else:
            language = self.language

        model_id: None | str | Unset
        if isinstance(self.model_id, Unset):
            model_id = UNSET
        else:
            model_id = self.model_id

        prompt: None | str | Unset
        if isinstance(self.prompt, Unset):
            prompt = UNSET
        else:
            prompt = self.prompt

        vad: dict[str, Any] | None | Unset
        if isinstance(self.vad, Unset):
            vad = UNSET
        elif isinstance(self.vad, TranscribeVadOptionsResponse):
            vad = self.vad.to_dict()
        else:
            vad = self.vad

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "source_path": source_path,
            }
        )
        if decode is not UNSET:
            field_dict["decode"] = decode
        if detect_language is not UNSET:
            field_dict["detect_language"] = detect_language
        if language is not UNSET:
            field_dict["language"] = language
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if prompt is not UNSET:
            field_dict["prompt"] = prompt
        if vad is not UNSET:
            field_dict["vad"] = vad

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.transcribe_decode_options_response import (
            TranscribeDecodeOptionsResponse,
        )
        from ..models.transcribe_vad_options_response import (
            TranscribeVadOptionsResponse,
        )

        d = dict(src_dict)
        source_path = d.pop("source_path")

        def _parse_decode(
            data: object,
        ) -> None | TranscribeDecodeOptionsResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                decode_type_1 = TranscribeDecodeOptionsResponse.from_dict(data)

                return decode_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TranscribeDecodeOptionsResponse | Unset, data)

        decode = _parse_decode(d.pop("decode", UNSET))

        def _parse_detect_language(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        detect_language = _parse_detect_language(d.pop("detect_language", UNSET))

        def _parse_language(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        language = _parse_language(d.pop("language", UNSET))

        def _parse_model_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_id = _parse_model_id(d.pop("model_id", UNSET))

        def _parse_prompt(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        prompt = _parse_prompt(d.pop("prompt", UNSET))

        def _parse_vad(data: object) -> None | TranscribeVadOptionsResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                vad_type_1 = TranscribeVadOptionsResponse.from_dict(data)

                return vad_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TranscribeVadOptionsResponse | Unset, data)

        vad = _parse_vad(d.pop("vad", UNSET))

        audio_transcription_request_data = cls(
            source_path=source_path,
            decode=decode,
            detect_language=detect_language,
            language=language,
            model_id=model_id,
            prompt=prompt,
            vad=vad,
        )

        audio_transcription_request_data.additional_properties = d
        return audio_transcription_request_data

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
