from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="VideoGenerationRequest")


@_attrs_define
class VideoGenerationRequest:
    """Request body for `POST /v1/video/generations`.

    Attributes:
        model (str): The model identifier to use.
        prompt (str): Text description of the desired video content.
        cfg_scale (float | None | Unset): Classifier-Free Guidance scale (default `7.0`).
        fps (float | Unset): Output frames per second (default `8`).
        guidance (float | None | Unset): Distilled guidance (default `3.5`).
        height (int | Unset): Frame height in pixels (default `512`).
        init_image (None | str | Unset): Init-image for video2video (base64 data URI).
        model_id (None | str | Unset): Optional catalog model identifier used for history attribution.
        negative_prompt (None | str | Unset): Negative text prompt.
        sample_method (None | str | Unset): Sampling method.
        scheduler (None | str | Unset): Sigma schedule.
        seed (int | None | Unset): RNG seed (default `42`).
        steps (int | None | Unset): Number of denoising steps (default `20`).
        strength (float | None | Unset): Strength for init-image influence (default `0.75`).
        video_frames (int | Unset): Number of video frames to generate (default `16`).
        width (int | Unset): Frame width in pixels (default `512`).
    """

    model: str
    prompt: str
    cfg_scale: float | None | Unset = UNSET
    fps: float | Unset = UNSET
    guidance: float | None | Unset = UNSET
    height: int | Unset = UNSET
    init_image: None | str | Unset = UNSET
    model_id: None | str | Unset = UNSET
    negative_prompt: None | str | Unset = UNSET
    sample_method: None | str | Unset = UNSET
    scheduler: None | str | Unset = UNSET
    seed: int | None | Unset = UNSET
    steps: int | None | Unset = UNSET
    strength: float | None | Unset = UNSET
    video_frames: int | Unset = UNSET
    width: int | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        model = self.model

        prompt = self.prompt

        cfg_scale: float | None | Unset
        if isinstance(self.cfg_scale, Unset):
            cfg_scale = UNSET
        else:
            cfg_scale = self.cfg_scale

        fps = self.fps

        guidance: float | None | Unset
        if isinstance(self.guidance, Unset):
            guidance = UNSET
        else:
            guidance = self.guidance

        height = self.height

        init_image: None | str | Unset
        if isinstance(self.init_image, Unset):
            init_image = UNSET
        else:
            init_image = self.init_image

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

        video_frames = self.video_frames

        width = self.width

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "model": model,
                "prompt": prompt,
            }
        )
        if cfg_scale is not UNSET:
            field_dict["cfg_scale"] = cfg_scale
        if fps is not UNSET:
            field_dict["fps"] = fps
        if guidance is not UNSET:
            field_dict["guidance"] = guidance
        if height is not UNSET:
            field_dict["height"] = height
        if init_image is not UNSET:
            field_dict["init_image"] = init_image
        if model_id is not UNSET:
            field_dict["model_id"] = model_id
        if negative_prompt is not UNSET:
            field_dict["negative_prompt"] = negative_prompt
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
        if video_frames is not UNSET:
            field_dict["video_frames"] = video_frames
        if width is not UNSET:
            field_dict["width"] = width

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        model = d.pop("model")

        prompt = d.pop("prompt")

        def _parse_cfg_scale(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        cfg_scale = _parse_cfg_scale(d.pop("cfg_scale", UNSET))

        fps = d.pop("fps", UNSET)

        def _parse_guidance(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        guidance = _parse_guidance(d.pop("guidance", UNSET))

        height = d.pop("height", UNSET)

        def _parse_init_image(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        init_image = _parse_init_image(d.pop("init_image", UNSET))

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

        video_frames = d.pop("video_frames", UNSET)

        width = d.pop("width", UNSET)

        video_generation_request = cls(
            model=model,
            prompt=prompt,
            cfg_scale=cfg_scale,
            fps=fps,
            guidance=guidance,
            height=height,
            init_image=init_image,
            model_id=model_id,
            negative_prompt=negative_prompt,
            sample_method=sample_method,
            scheduler=scheduler,
            seed=seed,
            steps=steps,
            strength=strength,
            video_frames=video_frames,
            width=width,
        )

        video_generation_request.additional_properties = d
        return video_generation_request

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
