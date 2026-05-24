from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="VideoGenerationRequestData")


@_attrs_define
class VideoGenerationRequestData:
    """
    Attributes:
        fps (float):
        height (int):
        model (str):
        prompt (str):
        video_frames (int):
        width (int):
        cfg_scale (float | None | Unset):
        guidance (float | None | Unset):
        model_id (None | str | Unset):
        negative_prompt (None | str | Unset):
        reference_image_path (None | str | Unset):
        sample_method (None | str | Unset):
        scheduler (None | str | Unset):
        seed (int | None | Unset):
        steps (int | None | Unset):
        strength (float | None | Unset):
    """

    fps: float
    height: int
    model: str
    prompt: str
    video_frames: int
    width: int
    cfg_scale: float | None | Unset = UNSET
    guidance: float | None | Unset = UNSET
    model_id: None | str | Unset = UNSET
    negative_prompt: None | str | Unset = UNSET
    reference_image_path: None | str | Unset = UNSET
    sample_method: None | str | Unset = UNSET
    scheduler: None | str | Unset = UNSET
    seed: int | None | Unset = UNSET
    steps: int | None | Unset = UNSET
    strength: float | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        fps = self.fps

        height = self.height

        model = self.model

        prompt = self.prompt

        video_frames = self.video_frames

        width = self.width

        cfg_scale: float | None | Unset
        if isinstance(self.cfg_scale, Unset):
            cfg_scale = UNSET
        else:
            cfg_scale = self.cfg_scale

        guidance: float | None | Unset
        if isinstance(self.guidance, Unset):
            guidance = UNSET
        else:
            guidance = self.guidance

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

        reference_image_path: None | str | Unset
        if isinstance(self.reference_image_path, Unset):
            reference_image_path = UNSET
        else:
            reference_image_path = self.reference_image_path

        sample_method: None | str | Unset
        if isinstance(self.sample_method, Unset):
            sample_method = UNSET
        else:
            sample_method = self.sample_method

        scheduler: None | str | Unset
        if isinstance(self.scheduler, Unset):
            scheduler = UNSET
        else:
            scheduler = self.scheduler

        seed: int | None | Unset
        if isinstance(self.seed, Unset):
            seed = UNSET
        else:
            seed = self.seed

        steps: int | None | Unset
        if isinstance(self.steps, Unset):
            steps = UNSET
        else:
            steps = self.steps

        strength: float | None | Unset
        if isinstance(self.strength, Unset):
            strength = UNSET
        else:
            strength = self.strength

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "fps": fps,
                "height": height,
                "model": model,
                "prompt": prompt,
                "video_frames": video_frames,
                "width": width,
            }
        )
        if cfg_scale is not UNSET:
            field_dict["cfg_scale"] = cfg_scale
        if guidance is not UNSET:
            field_dict["guidance"] = guidance
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if negative_prompt is not UNSET:
            field_dict["negative_prompt"] = negative_prompt
        if reference_image_path is not UNSET:
            field_dict["reference_image_path"] = reference_image_path
        if sample_method is not UNSET:
            field_dict["sample_method"] = sample_method
        if scheduler is not UNSET:
            field_dict["scheduler"] = scheduler
        if seed is not UNSET:
            field_dict["seed"] = seed
        if steps is not UNSET:
            field_dict["steps"] = steps
        if strength is not UNSET:
            field_dict["strength"] = strength

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        fps = d.pop("fps")

        height = d.pop("height")

        model = d.pop("model")

        prompt = d.pop("prompt")

        video_frames = d.pop("video_frames")

        width = d.pop("width")

        def _parse_cfg_scale(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        cfg_scale = _parse_cfg_scale(d.pop("cfg_scale", UNSET))

        def _parse_guidance(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        guidance = _parse_guidance(d.pop("guidance", UNSET))

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

        def _parse_reference_image_path(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        reference_image_path = _parse_reference_image_path(
            d.pop("reference_image_path", UNSET)
        )

        def _parse_sample_method(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        sample_method = _parse_sample_method(d.pop("sample_method", UNSET))

        def _parse_scheduler(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        scheduler = _parse_scheduler(d.pop("scheduler", UNSET))

        def _parse_seed(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        seed = _parse_seed(d.pop("seed", UNSET))

        def _parse_steps(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        steps = _parse_steps(d.pop("steps", UNSET))

        def _parse_strength(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        strength = _parse_strength(d.pop("strength", UNSET))

        video_generation_request_data = cls(
            fps=fps,
            height=height,
            model=model,
            prompt=prompt,
            video_frames=video_frames,
            width=width,
            cfg_scale=cfg_scale,
            guidance=guidance,
            model_id=model_id,
            negative_prompt=negative_prompt,
            reference_image_path=reference_image_path,
            sample_method=sample_method,
            scheduler=scheduler,
            seed=seed,
            steps=steps,
            strength=strength,
        )

        video_generation_request_data.additional_properties = d
        return video_generation_request_data

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
