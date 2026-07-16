from __future__ import annotations

import math
from pathlib import Path
from typing import Callable

from PIL import Image, ImageEnhance


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "public" / "pet-assets" / "frog-3d-sheet-alpha.png"
OUT_DIR = ROOT / "public" / "pet-assets" / "gifs"
CANVAS = 360


POSES = {
    "idle": (0, 0),
    "blink": (1, 0),
    "look": (2, 0),
    "side": (0, 1),
    "hop": (1, 1),
    "surprise": (2, 1),
    "alert": (0, 2),
    "disco": (1, 2),
    "false": (2, 2),
}


def trim_alpha(image: Image.Image, padding: int = 18) -> Image.Image:
    alpha = image.getchannel("A")
    box = alpha.point(lambda value: 255 if value > 8 else 0).getbbox()
    if not box:
        return image
    left = max(0, box[0] - padding)
    top = max(0, box[1] - padding)
    right = min(image.width, box[2] + padding)
    bottom = min(image.height, box[3] + padding)
    return image.crop((left, top, right, bottom))


def fit_pose(image: Image.Image, max_size: int = 292) -> Image.Image:
    scale = min(max_size / image.width, max_size / image.height)
    size = (max(1, round(image.width * scale)), max(1, round(image.height * scale)))
    return image.resize(size, Image.Resampling.LANCZOS)


def paste_center(sprite: Image.Image, dx: float = 0, dy: float = 0) -> Image.Image:
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    x = round((CANVAS - sprite.width) / 2 + dx)
    y = round((CANVAS - sprite.height) / 2 + dy)
    canvas.alpha_composite(sprite, (x, y))
    return canvas


def transform_pose(
    image: Image.Image,
    scale: float = 1.0,
    rotate: float = 0.0,
    dx: float = 0,
    dy: float = 0,
    color: Callable[[Image.Image], Image.Image] | None = None,
) -> Image.Image:
    next_image = image
    if color:
        next_image = color(next_image)
    if abs(scale - 1.0) > 0.001:
        size = (
            max(1, round(next_image.width * scale)),
            max(1, round(next_image.height * scale)),
        )
        next_image = next_image.resize(size, Image.Resampling.LANCZOS)
    if abs(rotate) > 0.001:
        next_image = next_image.rotate(rotate, resample=Image.Resampling.BICUBIC, expand=True)
    return paste_center(next_image, dx, dy)


def save_gif(path: Path, frames: list[Image.Image], duration: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(
        path,
        save_all=True,
        append_images=frames[1:],
        duration=duration,
        loop=0,
        disposal=2,
        optimize=False,
        transparency=0,
    )


def wave_frames(
    pose: Image.Image,
    count: int,
    duration: int,
    path: Path,
    scale_amp: float = 0.018,
    y_amp: float = 5.0,
    rotate_amp: float = 0.0,
    x_amp: float = 0.0,
    color: Callable[[Image.Image], Image.Image] | None = None,
) -> None:
    frames = []
    for index in range(count):
        phase = math.sin(index / count * math.tau)
        frames.append(
            transform_pose(
                pose,
                scale=1.0 + phase * scale_amp,
                rotate=phase * rotate_amp,
                dx=phase * x_amp,
                dy=-phase * y_amp,
                color=color,
            )
        )
    save_gif(path, frames, duration)


def pulse_red(image: Image.Image, ratio: float) -> Image.Image:
    overlay = Image.new("RGBA", image.size, (238, 48, 55, round(74 * ratio)))
    next_image = Image.alpha_composite(image, overlay)
    return ImageEnhance.Color(next_image).enhance(1.1 + 0.35 * ratio)


def build_gifs() -> None:
    sheet = Image.open(SOURCE).convert("RGBA")
    cell_w = sheet.width // 3
    cell_h = sheet.height // 3
    poses: dict[str, Image.Image] = {}
    for name, (col, row) in POSES.items():
        cell = sheet.crop((col * cell_w, row * cell_h, (col + 1) * cell_w, (row + 1) * cell_h))
        poses[name] = fit_pose(trim_alpha(cell))

    wave_frames(poses["idle"], 24, 72, OUT_DIR / "frog-idle.gif", scale_amp=0.014, y_amp=4.0)
    wave_frames(poses["blink"], 18, 76, OUT_DIR / "frog-blink.gif", scale_amp=0.012, y_amp=3.0)
    wave_frames(poses["look"], 22, 70, OUT_DIR / "frog-look.gif", scale_amp=0.01, y_amp=2.0, rotate_amp=2.0, x_amp=5.0)
    wave_frames(poses["side"], 22, 72, OUT_DIR / "frog-side.gif", scale_amp=0.012, y_amp=3.5, rotate_amp=-1.6)

    hop_frames = []
    for index in range(24):
        t = index / 24
        jump = math.sin(t * math.pi)
        squash = math.sin(t * math.tau)
        hop_frames.append(transform_pose(poses["hop"], scale=1.0 + 0.035 * squash, rotate=-5.0 * math.sin(t * math.tau), dy=-66 * jump))
    save_gif(OUT_DIR / "frog-hop.gif", hop_frames, 54)

    surprise_frames = []
    for index in range(18):
        phase = math.sin(index / 18 * math.tau)
        surprise_frames.append(transform_pose(poses["surprise"], scale=1.0 + 0.025 * abs(phase), rotate=phase * 2.0, dy=-abs(phase) * 5.0))
    save_gif(OUT_DIR / "frog-surprise.gif", surprise_frames, 58)

    alert_frames = []
    for index in range(20):
        phase = math.sin(index / 20 * math.tau)
        ratio = 0.5 + 0.5 * phase
        alert_frames.append(transform_pose(poses["alert"], scale=1.0 + 0.03 * ratio, rotate=phase * 3.2, dx=phase * 4.0, color=lambda img, r=ratio: pulse_red(img, r)))
    save_gif(OUT_DIR / "frog-alert.gif", alert_frames, 48)

    disco_frames = []
    for index in range(22):
        t = index / 22
        phase = math.sin(t * math.tau)
        disco_frames.append(transform_pose(poses["disco"], scale=1.0 + 0.04 * math.cos(t * math.tau), rotate=phase * 11.0, dx=phase * 18.0, dy=-abs(math.cos(t * math.tau)) * 14.0))
    save_gif(OUT_DIR / "frog-disco.gif", disco_frames, 50)

    false_frames = []
    for index in range(22):
        phase = math.sin(index / 22 * math.tau)
        false_frames.append(transform_pose(poses["false"], scale=1.0 + 0.01 * phase, rotate=phase * 2.8, dy=-phase * 2.0))
    save_gif(OUT_DIR / "frog-false.gif", false_frames, 76)


if __name__ == "__main__":
    build_gifs()
