from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.task_status import TaskStatus
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.audio_transcription_request_data import AudioTranscriptionRequestData
    from ..models.audio_transcription_result_data import AudioTranscriptionResultData
    from ..models.task_progress_response import TaskProgressResponse
    from ..models.timed_text_segment_response import TimedTextSegmentResponse
    from ..models.transcribe_decode_options_response import (
        TranscribeDecodeOptionsResponse,
    )
    from ..models.transcribe_vad_options_response import TranscribeVadOptionsResponse


T = TypeVar("T", bound="AudioTranscriptionTaskResponse")


@_attrs_define
class AudioTranscriptionTaskResponse:
    """
    Attributes:
        backend_id (str):
        created_at (str):
        request_data (AudioTranscriptionRequestData):
        source_path (str):
        status (TaskStatus):
        task_id (str):
        task_type (str):
        updated_at (str):
        decode_json (None | TranscribeDecodeOptionsResponse | Unset):
        detect_language (bool | None | Unset):
        error_msg (None | str | Unset):
        language (None | str | Unset):
        model_id (None | str | Unset):
        progress (None | TaskProgressResponse | Unset):
        prompt (None | str | Unset):
        result_data (AudioTranscriptionResultData | None | Unset):
        segments (list[TimedTextSegmentResponse] | None | Unset):
        transcript_text (None | str | Unset):
        vad_json (None | TranscribeVadOptionsResponse | Unset):
    """

    backend_id: str
    created_at: str
    request_data: AudioTranscriptionRequestData
    source_path: str
    status: TaskStatus
    task_id: str
    task_type: str
    updated_at: str
    decode_json: None | TranscribeDecodeOptionsResponse | Unset = UNSET
    detect_language: bool | None | Unset = UNSET
    error_msg: None | str | Unset = UNSET
    language: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET
    progress: None | TaskProgressResponse | Unset = UNSET
    prompt: None | str | Unset = UNSET
    result_data: AudioTranscriptionResultData | None | Unset = UNSET
    segments: list[TimedTextSegmentResponse] | None | Unset = UNSET
    transcript_text: None | str | Unset = UNSET
    vad_json: None | TranscribeVadOptionsResponse | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.audio_transcription_result_data import (
            AudioTranscriptionResultData,
        )
        from ..models.task_progress_response import TaskProgressResponse
        from ..models.transcribe_decode_options_response import (
            TranscribeDecodeOptionsResponse,
        )
        from ..models.transcribe_vad_options_response import (
            TranscribeVadOptionsResponse,
        )

        backend_id = self.backend_id

        created_at = self.created_at

        request_data = self.request_data.to_dict()

        source_path = self.source_path

        status = self.status.value

        task_id = self.task_id

        task_type = self.task_type

        updated_at = self.updated_at

        decode_json: dict[str, Any] | None | Unset
        if isinstance(self.decode_json, Unset):
            decode_json = UNSET
        elif isinstance(self.decode_json, TranscribeDecodeOptionsResponse):
            decode_json = self.decode_json.to_dict()
        else:
            decode_json = self.decode_json

        detect_language: bool | None | Unset
        if isinstance(self.detect_language, Unset):
            detect_language = UNSET
        else:
            detect_language = self.detect_language

        error_msg: None | str | Unset
        if isinstance(self.error_msg, Unset):
            error_msg = UNSET
        else:
            error_msg = self.error_msg

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

        progress: dict[str, Any] | None | Unset
        if isinstance(self.progress, Unset):
            progress = UNSET
        elif isinstance(self.progress, TaskProgressResponse):
            progress = self.progress.to_dict()
        else:
            progress = self.progress

        prompt: None | str | Unset
        if isinstance(self.prompt, Unset):
            prompt = UNSET
        else:
            prompt = self.prompt

        result_data: dict[str, Any] | None | Unset
        if isinstance(self.result_data, Unset):
            result_data = UNSET
        elif isinstance(self.result_data, AudioTranscriptionResultData):
            result_data = self.result_data.to_dict()
        else:
            result_data = self.result_data

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

        transcript_text: None | str | Unset
        if isinstance(self.transcript_text, Unset):
            transcript_text = UNSET
        else:
            transcript_text = self.transcript_text

        vad_json: dict[str, Any] | None | Unset
        if isinstance(self.vad_json, Unset):
            vad_json = UNSET
        elif isinstance(self.vad_json, TranscribeVadOptionsResponse):
            vad_json = self.vad_json.to_dict()
        else:
            vad_json = self.vad_json

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "backend_id": backend_id,
                "created_at": created_at,
                "request_data": request_data,
                "source_path": source_path,
                "status": status,
                "task_id": task_id,
                "task_type": task_type,
                "updated_at": updated_at,
            }
        )
        if decode_json is not UNSET:
            field_dict["decode_json"] = decode_json
        if detect_language is not UNSET:
            field_dict["detect_language"] = detect_language
        if error_msg is not UNSET:
            field_dict["error_msg"] = error_msg
        if language is not UNSET:
            field_dict["language"] = language
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if progress is not UNSET:
            field_dict["progress"] = progress
        if prompt is not UNSET:
            field_dict["prompt"] = prompt
        if result_data is not UNSET:
            field_dict["result_data"] = result_data
        if segments is not UNSET:
            field_dict["segments"] = segments
        if transcript_text is not UNSET:
            field_dict["transcript_text"] = transcript_text
        if vad_json is not UNSET:
            field_dict["vad_json"] = vad_json

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.audio_transcription_request_data import (
            AudioTranscriptionRequestData,
        )
        from ..models.audio_transcription_result_data import (
            AudioTranscriptionResultData,
        )
        from ..models.task_progress_response import TaskProgressResponse
        from ..models.timed_text_segment_response import TimedTextSegmentResponse
        from ..models.transcribe_decode_options_response import (
            TranscribeDecodeOptionsResponse,
        )
        from ..models.transcribe_vad_options_response import (
            TranscribeVadOptionsResponse,
        )

        d = dict(src_dict)
        backend_id = d.pop("backend_id")

        created_at = d.pop("created_at")

        request_data = AudioTranscriptionRequestData.from_dict(d.pop("request_data"))

        source_path = d.pop("source_path")

        status = TaskStatus(d.pop("status"))

        task_id = d.pop("task_id")

        task_type = d.pop("task_type")

        updated_at = d.pop("updated_at")

        def _parse_decode_json(
            data: object,
        ) -> None | TranscribeDecodeOptionsResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                decode_json_type_1 = TranscribeDecodeOptionsResponse.from_dict(data)

                return decode_json_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TranscribeDecodeOptionsResponse | Unset, data)

        decode_json = _parse_decode_json(d.pop("decode_json", UNSET))

        def _parse_detect_language(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        detect_language = _parse_detect_language(d.pop("detect_language", UNSET))

        def _parse_error_msg(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error_msg = _parse_error_msg(d.pop("error_msg", UNSET))

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

        def _parse_progress(data: object) -> None | TaskProgressResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                progress_type_1 = TaskProgressResponse.from_dict(data)

                return progress_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TaskProgressResponse | Unset, data)

        progress = _parse_progress(d.pop("progress", UNSET))

        def _parse_prompt(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        prompt = _parse_prompt(d.pop("prompt", UNSET))

        def _parse_result_data(
            data: object,
        ) -> AudioTranscriptionResultData | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                result_data_type_1 = AudioTranscriptionResultData.from_dict(data)

                return result_data_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(AudioTranscriptionResultData | None | Unset, data)

        result_data = _parse_result_data(d.pop("result_data", UNSET))

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

        def _parse_transcript_text(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        transcript_text = _parse_transcript_text(d.pop("transcript_text", UNSET))

        def _parse_vad_json(
            data: object,
        ) -> None | TranscribeVadOptionsResponse | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                vad_json_type_1 = TranscribeVadOptionsResponse.from_dict(data)

                return vad_json_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TranscribeVadOptionsResponse | Unset, data)

        vad_json = _parse_vad_json(d.pop("vad_json", UNSET))

        audio_transcription_task_response = cls(
            backend_id=backend_id,
            created_at=created_at,
            request_data=request_data,
            source_path=source_path,
            status=status,
            task_id=task_id,
            task_type=task_type,
            updated_at=updated_at,
            decode_json=decode_json,
            detect_language=detect_language,
            error_msg=error_msg,
            language=language,
            model_id=model_id,
            progress=progress,
            prompt=prompt,
            result_data=result_data,
            segments=segments,
            transcript_text=transcript_text,
            vad_json=vad_json,
        )

        audio_transcription_task_response.additional_properties = d
        return audio_transcription_task_response

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
