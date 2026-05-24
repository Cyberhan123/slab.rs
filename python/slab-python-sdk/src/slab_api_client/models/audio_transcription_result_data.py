from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.timed_text_segment_response import TimedTextSegmentResponse


T = TypeVar("T", bound="AudioTranscriptionResultData")


@_attrs_define
class AudioTranscriptionResultData:
    """
    Attributes:
        segments (list[TimedTextSegmentResponse]):
        text (str):
    """

    segments: list[TimedTextSegmentResponse]
    text: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        segments = []
        for segments_item_data in self.segments:
            segments_item = segments_item_data.to_dict()
            segments.append(segments_item)

        text = self.text

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "segments": segments,
                "text": text,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.timed_text_segment_response import TimedTextSegmentResponse

        d = dict(src_dict)
        segments = []
        _segments = d.pop("segments")
        for segments_item_data in _segments:
            segments_item = TimedTextSegmentResponse.from_dict(segments_item_data)

            segments.append(segments_item)

        text = d.pop("text")

        audio_transcription_result_data = cls(
            segments=segments,
            text=text,
        )

        audio_transcription_result_data.additional_properties = d
        return audio_transcription_result_data

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
