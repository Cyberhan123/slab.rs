from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.task_status import TaskStatus
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.task_progress_response import TaskProgressResponse
    from ..models.video_generation_request_data import VideoGenerationRequestData
    from ..models.video_generation_result_data import VideoGenerationResultData


T = TypeVar("T", bound="VideoGenerationTaskResponse")


@_attrs_define
class VideoGenerationTaskResponse:
    """
    Attributes:
        backend_id (str):
        created_at (str):
        fps (float):
        frames (int):
        height (int):
        model_path (str):
        prompt (str):
        request_data (VideoGenerationRequestData):
        status (TaskStatus):
        task_id (str):
        task_type (str):
        updated_at (str):
        width (int):
        error_msg (None | str | Unset):
        model_id (None | str | Unset):
        negative_prompt (None | str | Unset):
        progress (None | TaskProgressResponse | Unset):
        reference_image_url (None | str | Unset):
        result_data (None | Unset | VideoGenerationResultData):
        video_url (None | str | Unset):
    """

    backend_id: str
    created_at: str
    fps: float
    frames: int
    height: int
    model_path: str
    prompt: str
    request_data: VideoGenerationRequestData
    status: TaskStatus
    task_id: str
    task_type: str
    updated_at: str
    width: int
    error_msg: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET
    negative_prompt: None | str | Unset = UNSET
    progress: None | TaskProgressResponse | Unset = UNSET
    reference_image_url: None | str | Unset = UNSET
    result_data: None | Unset | VideoGenerationResultData = UNSET
    video_url: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.task_progress_response import TaskProgressResponse
        from ..models.video_generation_result_data import VideoGenerationResultData

        backend_id = self.backend_id

        created_at = self.created_at

        fps = self.fps

        frames = self.frames

        height = self.height

        model_path = self.model_path

        prompt = self.prompt

        request_data = self.request_data.to_dict()

        status = self.status.value

        task_id = self.task_id

        task_type = self.task_type

        updated_at = self.updated_at

        width = self.width

        error_msg: None | str | Unset
        if isinstance(self.error_msg, Unset):
            error_msg = UNSET
        else:
            error_msg = self.error_msg

        model_id: None | str | Unset
        if isinstance(self.model_id, Unset):
            model_id = UNSET
        else:
            model_id = self.model_id

        negative_prompt: None | str | Unset
        if isinstance(self.negative_prompt, Unset):
            negative_prompt = UNSET
        else:
            negative_prompt = self.negative_prompt

        progress: dict[str, Any] | None | Unset
        if isinstance(self.progress, Unset):
            progress = UNSET
        elif isinstance(self.progress, TaskProgressResponse):
            progress = self.progress.to_dict()
        else:
            progress = self.progress

        reference_image_url: None | str | Unset
        if isinstance(self.reference_image_url, Unset):
            reference_image_url = UNSET
        else:
            reference_image_url = self.reference_image_url

        result_data: dict[str, Any] | None | Unset
        if isinstance(self.result_data, Unset):
            result_data = UNSET
        elif isinstance(self.result_data, VideoGenerationResultData):
            result_data = self.result_data.to_dict()
        else:
            result_data = self.result_data

        video_url: None | str | Unset
        if isinstance(self.video_url, Unset):
            video_url = UNSET
        else:
            video_url = self.video_url

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "backend_id": backend_id,
                "created_at": created_at,
                "fps": fps,
                "frames": frames,
                "height": height,
                "model_path": model_path,
                "prompt": prompt,
                "request_data": request_data,
                "status": status,
                "task_id": task_id,
                "task_type": task_type,
                "updated_at": updated_at,
                "width": width,
            }
        )
        if error_msg is not UNSET:
            field_dict["error_msg"] = error_msg
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if negative_prompt is not UNSET:
            field_dict["negative_prompt"] = negative_prompt
        if progress is not UNSET:
            field_dict["progress"] = progress
        if reference_image_url is not UNSET:
            field_dict["reference_image_url"] = reference_image_url
        if result_data is not UNSET:
            field_dict["result_data"] = result_data
        if video_url is not UNSET:
            field_dict["video_url"] = video_url

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.task_progress_response import TaskProgressResponse
        from ..models.video_generation_request_data import VideoGenerationRequestData
        from ..models.video_generation_result_data import VideoGenerationResultData

        d = dict(src_dict)
        backend_id = d.pop("backend_id")

        created_at = d.pop("created_at")

        fps = d.pop("fps")

        frames = d.pop("frames")

        height = d.pop("height")

        model_path = d.pop("model_path")

        prompt = d.pop("prompt")

        request_data = VideoGenerationRequestData.from_dict(d.pop("request_data"))

        status = TaskStatus(d.pop("status"))

        task_id = d.pop("task_id")

        task_type = d.pop("task_type")

        updated_at = d.pop("updated_at")

        width = d.pop("width")

        def _parse_error_msg(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error_msg = _parse_error_msg(d.pop("error_msg", UNSET))

        def _parse_model_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        model_id = _parse_model_id(d.pop("model_id", UNSET))

        def _parse_negative_prompt(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        negative_prompt = _parse_negative_prompt(d.pop("negative_prompt", UNSET))

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

        def _parse_reference_image_url(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        reference_image_url = _parse_reference_image_url(
            d.pop("reference_image_url", UNSET)
        )

        def _parse_result_data(
            data: object,
        ) -> None | Unset | VideoGenerationResultData:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                result_data_type_1 = VideoGenerationResultData.from_dict(data)

                return result_data_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | VideoGenerationResultData, data)

        result_data = _parse_result_data(d.pop("result_data", UNSET))

        def _parse_video_url(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        video_url = _parse_video_url(d.pop("video_url", UNSET))

        video_generation_task_response = cls(
            backend_id=backend_id,
            created_at=created_at,
            fps=fps,
            frames=frames,
            height=height,
            model_path=model_path,
            prompt=prompt,
            request_data=request_data,
            status=status,
            task_id=task_id,
            task_type=task_type,
            updated_at=updated_at,
            width=width,
            error_msg=error_msg,
            model_id=model_id,
            negative_prompt=negative_prompt,
            progress=progress,
            reference_image_url=reference_image_url,
            result_data=result_data,
            video_url=video_url,
        )

        video_generation_task_response.additional_properties = d
        return video_generation_task_response

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
