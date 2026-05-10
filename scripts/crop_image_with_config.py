#!/usr/bin/env python3
"""Crop an image with the same CropConfig math as OpenCvCropper.

The crop rectangle matches `crates/champions-infrastructure/src/vision/cropper.rs`.

Edit the globals below, then run:
    python scripts/crop_image_with_config.py
"""

from __future__ import annotations

import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

INPUT_IMAGE_PATH = Path("F:\download\capture.png")
OUTPUT_IMAGE_PATH = Path("F:\download\capture_out.png")
CROP_INDEX = 0
CROP_CONFIG: dict[str, float] = {
    "center_x": 0.51,
    "y_start": 0.65,
    "y_gap": 0.0,
    "size_w": 0.13,
    "width_ratio": 6.0,
}

REQUIRED_CONFIG_KEYS = (
    "center_x",
    "y_start",
    "y_gap",
    "size_w",
    "width_ratio",
)


@dataclass(frozen=True)
class CropConfig:
    center_x: float
    y_start: float
    y_gap: float
    size_w: float
    width_ratio: float


@dataclass(frozen=True)
class CropBox:
    left: int
    top: int
    width: int
    height: int

    @property
    def right(self) -> int:
        return self.left + self.width

    @property
    def bottom(self) -> int:
        return self.top + self.height


def has_direct_config_keys(payload: Mapping[str, Any]) -> bool:
    return all(key in payload for key in REQUIRED_CONFIG_KEYS)


def select_config_mapping(payload: Any, *, source_path: Path) -> Mapping[str, Any]:
    if not isinstance(payload, Mapping):
        raise RuntimeError(f"config must be a JSON object: {source_path}")

    if has_direct_config_keys(payload):
        return payload

    missing_keys = [key for key in REQUIRED_CONFIG_KEYS if key not in payload]
    missing = ", ".join(missing_keys)
    raise RuntimeError(f"config {source_path} is missing required keys: {missing}")


def parse_float(mapping: Mapping[str, Any], key: str) -> float:
    raw = mapping.get(key)
    if isinstance(raw, bool):
        raise RuntimeError(f"config value '{key}' must be numeric")

    try:
        return float(raw)
    except (TypeError, ValueError) as exc:
        raise RuntimeError(f"config value '{key}' must be numeric") from exc


def load_crop_config(payload: Mapping[str, Any]) -> CropConfig:
    mapping = select_config_mapping(payload, source_path=Path("<global CROP_CONFIG>"))
    return CropConfig(
        center_x=parse_float(mapping, "center_x"),
        y_start=parse_float(mapping, "y_start"),
        y_gap=parse_float(mapping, "y_gap"),
        size_w=parse_float(mapping, "size_w"),
        width_ratio=parse_float(mapping, "width_ratio"),
    )


def compute_crop_box(
    frame_width: int,
    frame_height: int,
    config: CropConfig,
    index: int,
) -> CropBox:
    if frame_width <= 0 or frame_height <= 0:
        raise RuntimeError("input image must have a positive size")
    if index < 0:
        raise RuntimeError("index must be 0 or greater")

    w = float(frame_width)
    h = float(frame_height)

    size_h = config.size_w * w
    size_w = size_h * config.width_ratio
    cx = config.center_x * w
    cy = (config.y_start * h) + (float(index) * config.y_gap * h)

    left = int(max(cx - size_w / 2.0, 0.0))
    top = int(max(cy - size_h / 2.0, 0.0))
    crop_w = min(int(size_w), max(frame_width - left, 0))
    crop_h = min(int(size_h), max(frame_height - top, 0))

    if crop_w == 0 or crop_h == 0:
        raise RuntimeError("computed crop size is empty")

    return CropBox(left=left, top=top, width=crop_w, height=crop_h)


def open_rgb_image(path: Path):
    try:
        from PIL import Image
    except ModuleNotFoundError as exc:
        raise RuntimeError(
            "Pillow is required to crop images. Install it with `pip install Pillow`."
        ) from exc

    with Image.open(path) as image:
        return image.convert("RGB")


def main() -> int:
    input_path = INPUT_IMAGE_PATH.expanduser().resolve()
    output_path = OUTPUT_IMAGE_PATH.expanduser().resolve()

    try:
        config = load_crop_config(CROP_CONFIG)
        image = open_rgb_image(input_path)
        crop_box = compute_crop_box(image.width, image.height, config, CROP_INDEX)
        cropped = image.crop(
            (crop_box.left, crop_box.top, crop_box.right, crop_box.bottom)
        )
        output_path.parent.mkdir(parents=True, exist_ok=True)
        cropped.save(output_path)
    except Exception as exc:
        print(f"Failed to crop image: {exc}", file=sys.stderr)
        return 1

    print(
        "Wrote crop "
        f"{crop_box.width}x{crop_box.height} "
        f"at ({crop_box.left}, {crop_box.top}) "
        f"to {output_path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
