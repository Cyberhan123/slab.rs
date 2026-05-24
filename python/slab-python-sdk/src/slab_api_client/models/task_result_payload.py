from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.timed_text_segment_response import TimedTextSegmentResponse


T = TypeVar("T", bound="TaskResultPayload")


@_attrs_define
class TaskResultPayload:
    """Result payload returned by `GET /v1/tasks/{id}/result`.

    Fields are populated depending on the task type:
    - Single-image tasks: `image` contains a `data:image/png;base64,…` data URI.
    - Multi-image diffusion tasks: `images` contains an array of data URIs; `image`
      also holds the first one for backward compatibility.
    - Video tasks: `video_path` holds the path of the assembled MP4 file.
    - Text-producing tasks (whisper, etc.): `text` contains the UTF-8 result.

        Attributes:
            image (None | str | Unset): Base64-encoded PNG data URI, present for single-image and as the first
                image for multi-image task results.
            images (list[str] | None | Unset): Array of base64-encoded PNG data URIs for multi-image task results.
            output_path (None | str | Unset): Absolute output path for file-producing utility tasks such as FFmpeg
                conversion.
            segments (list[TimedTextSegmentResponse] | None | Unset): Timed text segments, present for Whisper
                transcriptions with timestamps.
            text (None | str | Unset): Text content, present for `whisper` and other text-producing task results.
            video_path (None | str | Unset): Absolute path to the assembled MP4 video file for video task results.
    """

    image: None | str | Unset = UNSET
    images: list[str] | None | Unset = UNSET
    output_path: None | str | Unset = UNSET
    segments: list[TimedTextSegmentResponse] | None | Unset = UNSET
    text: None | str | Unset = UNSET
    video_path: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        image: None | str | Unset
        if isinstance(self.image, Unset):
            image = UNSET
        else:
            image = self.image

        images: list[str] | None | Unset
        if isinstance(self.images, Unset):
            images = UNSET
        elif isinstance(self.images, list):
            images = self.images

        else:
            images = self.images

        output_path: None | str | Unset
        if isinstance(self.output_path, Unset):
            output_path = UNSET
        else:
            output_path = self.output_path

        segments: list[dict[str, Any]] | None | Unset
        if isinstance(self.segments, Unset):
            segments = UNSET
        elif isinstance(self.segments, list):
            segments = []
            for segments_type_0_item_data in self.segments:
                segments_type_0_item = segments_type_0_item_data.to_dict()
                segments.append(segments_type_0_item)

        else:
            segments = self.segments

        text: None | str | Unset
        if isinstance(self.text, Unset):
            text = UNSET
        else:
            text = self.text

        video_path: None | str | Unset
        if isinstance(self.video_path, Unset):
            video_path = UNSET
        else:
            video_path = self.video_path

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if image is not UNSET:
            field_dict["image"] = image
        if images is not UNSET:
            field_dict["images"] = images
        if output_path is not UNSET:
            field_dict["output_path"] = output_path
        if segments is not UNSET:
            field_dict["segments"] = segments
        if text is not UNSET:
            field_dict["text"] = text
        if video_path is not UNSET:
            field_dict["video_path"] = video_path

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.timed_text_segment_response import TimedTextSegmentResponse

        d = dict(src_dict)

        def _parse_image(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        image = _parse_image(d.pop("image", UNSET))

        def _parse_images(data: object) -> list[str] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                images_type_0 = cast(list[str], data)

                return images_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[str] | None | Unset, data)

        images = _parse_images(d.pop("images", UNSET))

        def _parse_output_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        output_path = _parse_output_path(d.pop("output_path", UNSET))

        def _parse_segments(
            data: object,
        ) -> list[TimedTextSegmentResponse] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                segments_type_0 = []
                _segments_type_0 = data
                for segments_type_0_item_data in _segments_type_0:
                    segments_type_0_item = TimedTextSegmentResponse.from_dict(
                        segments_type_0_item_data
                    )

                    segments_type_0.append(segments_type_0_item)

                return segments_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[TimedTextSegmentResponse] | None | Unset, data)

        segments = _parse_segments(d.pop("segments", UNSET))

        def _parse_text(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        text = _parse_text(d.pop("text", UNSET))

        def _parse_video_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        video_path = _parse_video_path(d.pop("video_path", UNSET))

        task_result_payload = cls(
            image=image,
            images=images,
            output_path=output_path,
            segments=segments,
            text=text,
            video_path=video_path,
        )

        task_result_payload.additional_properties = d
        return task_result_payload

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
