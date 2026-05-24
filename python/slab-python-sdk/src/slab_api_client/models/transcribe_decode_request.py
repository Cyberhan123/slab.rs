from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="TranscribeDecodeRequest")


@_attrs_define
class TranscribeDecodeRequest:
    """
    Attributes:
        duration_ms (int | None | Unset): Duration in milliseconds to process (0 means full input).
        entropy_thold (float | None | Unset): Entropy threshold.
        logprob_thold (float | None | Unset): Log probability threshold.
        max_len (int | None | Unset): Maximum segment length in characters.
        max_tokens (int | None | Unset): Maximum tokens per segment.
        no_context (bool | None | Unset): Do not use past transcription as prompt.
        no_speech_thold (float | None | Unset): No-speech threshold.
        no_timestamps (bool | None | Unset): Do not generate timestamps.
        offset_ms (int | None | Unset): Start offset in milliseconds.
        split_on_word (bool | None | Unset): Split timestamps on words instead of tokens.
        suppress_nst (bool | None | Unset): Suppress non-speech tokens.
        tdrz_enable (bool | None | Unset): Enable tinydiarize speaker turn detection.
        temperature (float | None | Unset): Initial decoding temperature.
        temperature_inc (float | None | Unset): Temperature increment for fallback decoding.
        token_timestamps (bool | None | Unset): Enable token-level timestamps.
        word_thold (float | None | Unset): Word timestamp probability threshold.
    """

    duration_ms: int | None | Unset = UNSET
    entropy_thold: float | None | Unset = UNSET
    logprob_thold: float | None | Unset = UNSET
    max_len: int | None | Unset = UNSET
    max_tokens: int | None | Unset = UNSET
    no_context: bool | None | Unset = UNSET
    no_speech_thold: float | None | Unset = UNSET
    no_timestamps: bool | None | Unset = UNSET
    offset_ms: int | None | Unset = UNSET
    split_on_word: bool | None | Unset = UNSET
    suppress_nst: bool | None | Unset = UNSET
    tdrz_enable: bool | None | Unset = UNSET
    temperature: float | None | Unset = UNSET
    temperature_inc: float | None | Unset = UNSET
    token_timestamps: bool | None | Unset = UNSET
    word_thold: float | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        duration_ms: int | None | Unset
        if isinstance(self.duration_ms, Unset):
            duration_ms = UNSET
        else:
            duration_ms = self.duration_ms

        entropy_thold: float | None | Unset
        if isinstance(self.entropy_thold, Unset):
            entropy_thold = UNSET
        else:
            entropy_thold = self.entropy_thold

        logprob_thold: float | None | Unset
        if isinstance(self.logprob_thold, Unset):
            logprob_thold = UNSET
        else:
            logprob_thold = self.logprob_thold

        max_len: int | None | Unset
        if isinstance(self.max_len, Unset):
            max_len = UNSET
        else:
            max_len = self.max_len

        max_tokens: int | None | Unset
        if isinstance(self.max_tokens, Unset):
            max_tokens = UNSET
        else:
            max_tokens = self.max_tokens

        no_context: bool | None | Unset
        if isinstance(self.no_context, Unset):
            no_context = UNSET
        else:
            no_context = self.no_context

        no_speech_thold: float | None | Unset
        if isinstance(self.no_speech_thold, Unset):
            no_speech_thold = UNSET
        else:
            no_speech_thold = self.no_speech_thold

        no_timestamps: bool | None | Unset
        if isinstance(self.no_timestamps, Unset):
            no_timestamps = UNSET
        else:
            no_timestamps = self.no_timestamps

        offset_ms: int | None | Unset
        if isinstance(self.offset_ms, Unset):
            offset_ms = UNSET
        else:
            offset_ms = self.offset_ms

        split_on_word: bool | None | Unset
        if isinstance(self.split_on_word, Unset):
            split_on_word = UNSET
        else:
            split_on_word = self.split_on_word

        suppress_nst: bool | None | Unset
        if isinstance(self.suppress_nst, Unset):
            suppress_nst = UNSET
        else:
            suppress_nst = self.suppress_nst

        tdrz_enable: bool | None | Unset
        if isinstance(self.tdrz_enable, Unset):
            tdrz_enable = UNSET
        else:
            tdrz_enable = self.tdrz_enable

        temperature: float | None | Unset
        if isinstance(self.temperature, Unset):
            temperature = UNSET
        else:
            temperature = self.temperature

        temperature_inc: float | None | Unset
        if isinstance(self.temperature_inc, Unset):
            temperature_inc = UNSET
        else:
            temperature_inc = self.temperature_inc

        token_timestamps: bool | None | Unset
        if isinstance(self.token_timestamps, Unset):
            token_timestamps = UNSET
        else:
            token_timestamps = self.token_timestamps

        word_thold: float | None | Unset
        if isinstance(self.word_thold, Unset):
            word_thold = UNSET
        else:
            word_thold = self.word_thold

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if duration_ms is not UNSET:
            field_dict["duration_ms"] = duration_ms
        if entropy_thold is not UNSET:
            field_dict["entropy_thold"] = entropy_thold
        if logprob_thold is not UNSET:
            field_dict["logprob_thold"] = logprob_thold
        if max_len is not UNSET:
            field_dict["max_len"] = max_len
        if max_tokens is not UNSET:
            field_dict["max_tokens"] = max_tokens
        if no_context is not UNSET:
            field_dict["no_context"] = no_context
        if no_speech_thold is not UNSET:
            field_dict["no_speech_thold"] = no_speech_thold
        if no_timestamps is not UNSET:
            field_dict["no_timestamps"] = no_timestamps
        if offset_ms is not UNSET:
            field_dict["offset_ms"] = offset_ms
        if split_on_word is not UNSET:
            field_dict["split_on_word"] = split_on_word
        if suppress_nst is not UNSET:
            field_dict["suppress_nst"] = suppress_nst
        if tdrz_enable is not UNSET:
            field_dict["tdrz_enable"] = tdrz_enable
        if temperature is not UNSET:
            field_dict["temperature"] = temperature
        if temperature_inc is not UNSET:
            field_dict["temperature_inc"] = temperature_inc
        if token_timestamps is not UNSET:
            field_dict["token_timestamps"] = token_timestamps
        if word_thold is not UNSET:
            field_dict["word_thold"] = word_thold

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_duration_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        duration_ms = _parse_duration_ms(d.pop("duration_ms", UNSET))

        def _parse_entropy_thold(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        entropy_thold = _parse_entropy_thold(d.pop("entropy_thold", UNSET))

        def _parse_logprob_thold(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        logprob_thold = _parse_logprob_thold(d.pop("logprob_thold", UNSET))

        def _parse_max_len(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_len = _parse_max_len(d.pop("max_len", UNSET))

        def _parse_max_tokens(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        max_tokens = _parse_max_tokens(d.pop("max_tokens", UNSET))

        def _parse_no_context(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        no_context = _parse_no_context(d.pop("no_context", UNSET))

        def _parse_no_speech_thold(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        no_speech_thold = _parse_no_speech_thold(d.pop("no_speech_thold", UNSET))

        def _parse_no_timestamps(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        no_timestamps = _parse_no_timestamps(d.pop("no_timestamps", UNSET))

        def _parse_offset_ms(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        offset_ms = _parse_offset_ms(d.pop("offset_ms", UNSET))

        def _parse_split_on_word(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        split_on_word = _parse_split_on_word(d.pop("split_on_word", UNSET))

        def _parse_suppress_nst(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        suppress_nst = _parse_suppress_nst(d.pop("suppress_nst", UNSET))

        def _parse_tdrz_enable(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        tdrz_enable = _parse_tdrz_enable(d.pop("tdrz_enable", UNSET))

        def _parse_temperature(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        temperature = _parse_temperature(d.pop("temperature", UNSET))

        def _parse_temperature_inc(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        temperature_inc = _parse_temperature_inc(d.pop("temperature_inc", UNSET))

        def _parse_token_timestamps(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        token_timestamps = _parse_token_timestamps(d.pop("token_timestamps", UNSET))

        def _parse_word_thold(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        word_thold = _parse_word_thold(d.pop("word_thold", UNSET))

        transcribe_decode_request = cls(
            duration_ms=duration_ms,
            entropy_thold=entropy_thold,
            logprob_thold=logprob_thold,
            max_len=max_len,
            max_tokens=max_tokens,
            no_context=no_context,
            no_speech_thold=no_speech_thold,
            no_timestamps=no_timestamps,
            offset_ms=offset_ms,
            split_on_word=split_on_word,
            suppress_nst=suppress_nst,
            tdrz_enable=tdrz_enable,
            temperature=temperature,
            temperature_inc=temperature_inc,
            token_timestamps=token_timestamps,
            word_thold=word_thold,
        )

        transcribe_decode_request.additional_properties = d
        return transcribe_decode_request

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
