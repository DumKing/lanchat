from __future__ import annotations

from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "public" / "pet-assets" / "generated-gif-source"
OUT_DIR = ROOT / "public" / "pet-assets" / "generated-gifs"
CANVAS = 360

SHEETS = [
    ("frog-idle-blink-sheet-alpha.png", [("frog-idle.gif", 0, 82), ("frog-blink.gif", 1, 82)]),
    ("frog-hop-disco-sheet-alpha.png", [("frog-hop.gif", 0, 62), ("frog-disco.gif", 1, 58)]),
    ("frog-alert-false-sheet-alpha.png", [("frog-alert.gif", 0, 58), ("frog-false.gif", 1, 86)]),
]


def trim_alpha(image: Image.Image, padding: int = 20) -> Image.Image:
    alpha = image.getchannel("A")
    box = alpha.point(lambda value: 255 if value > 12 else 0).getbbox()
    if box is None:
        return image
    left = max(0, box[0] - padding)
    top = max(0, box[1] - padding)
    right = min(image.width, box[2] + padding)
    bottom = min(image.height, box[3] + padding)
    return image.crop((left, top, right, bottom))


def normalize_frame(frame: Image.Image) -> Image.Image:
    frame = trim_alpha(frame)
    scale = min(310 / frame.width, 310 / frame.height)
    size = (max(1, round(frame.width * scale)), max(1, round(frame.height * scale)))
    frame = frame.resize(size, Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    x = (CANVAS - frame.width) // 2
    y = (CANVAS - frame.height) // 2
    canvas.alpha_composite(frame, (x, y))
    return canvas


def save_gif(name: str, frames: list[Image.Image], duration: int) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    frames[0].save(
        OUT_DIR / name,
        save_all=True,
        append_images=frames[1:],
        duration=duration,
        loop=0,
        disposal=2,
        optimize=False,
        transparency=0,
    )


def build() -> None:
    for sheet_name, outputs in SHEETS:
        sheet = Image.open(SOURCE_DIR / sheet_name).convert("RGBA")
        cell_width = sheet.width // 6
        cell_height = sheet.height // 2
        for out_name, row, duration in outputs:
            frames = []
            for col in range(6):
                cell = sheet.crop((
                    col * cell_width,
                    row * cell_height,
                    (col + 1) * cell_width,
                    (row + 1) * cell_height,
                ))
                frames.append(normalize_frame(cell))
            save_gif(out_name, frames, duration)


if __name__ == "__main__":
    build()
