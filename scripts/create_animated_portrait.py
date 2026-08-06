from __future__ import annotations

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter


def smooth_ellipse_mask(size, boxes):
    mask = Image.new("L", size, 0)
    draw = ImageDraw.Draw(mask)
    for box in boxes:
        draw.ellipse(box, fill=255)
    return mask.filter(ImageFilter.GaussianBlur(max(3, size[0] // 120)))


def remap(image: Image.Image, dx: np.ndarray, dy: np.ndarray) -> Image.Image:
    arr = np.asarray(image.convert("RGB"), dtype=np.float32)
    h, w = arr.shape[:2]
    yy, xx = np.mgrid[0:h, 0:w]
    sx = np.clip(xx - dx, 0, w - 1.001)
    sy = np.clip(yy - dy, 0, h - 1.001)
    x0 = sx.astype(np.int32)
    y0 = sy.astype(np.int32)
    x1 = np.minimum(x0 + 1, w - 1)
    y1 = np.minimum(y0 + 1, h - 1)
    wx = (sx - x0)[..., None]
    wy = (sy - y0)[..., None]
    out = (
        arr[y0, x0] * (1 - wx) * (1 - wy)
        + arr[y0, x1] * wx * (1 - wy)
        + arr[y1, x0] * (1 - wx) * wy
        + arr[y1, x1] * wx * wy
    )
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), "RGB")


def blink_amount(t: float) -> float:
    value = 0.0
    for center, width in ((0.28, 0.035), (0.72, 0.045)):
        distance = min(abs(t - center), 1 - abs(t - center))
        value = max(value, math.exp(-0.5 * (distance / width) ** 2))
    return min(1.0, value * 1.18)


def add_petals_and_sparkles(frame: Image.Image, t: float) -> Image.Image:
    w, h = frame.size
    overlay = Image.new("RGBA", frame.size, (0, 0, 0, 0))

    petals = [
        (0.05, 0.08, 0.8, 8), (0.16, 0.37, 1.1, 6), (0.27, 0.70, 0.9, 7),
        (0.39, 0.15, 1.2, 5), (0.52, 0.53, 0.7, 8), (0.65, 0.86, 1.0, 6),
        (0.78, 0.25, 0.85, 7), (0.90, 0.62, 1.15, 5), (0.97, 0.93, 0.75, 7),
        (0.33, 0.91, 1.25, 5), (0.70, 0.05, 0.95, 6), (0.12, 0.78, 1.05, 6),
    ]
    for idx, (x0, phase, speed, radius) in enumerate(petals):
        progress = (phase + t * speed) % 1.0
        x = (x0 + 0.035 * math.sin(2 * math.pi * (progress * 1.3 + idx * 0.17))) * w
        y = (-0.08 + progress * 1.16) * h
        angle = 360 * (t * speed + idx * 0.13)
        petal = Image.new("RGBA", (radius * 4, radius * 4), (0, 0, 0, 0))
        pd = ImageDraw.Draw(petal)
        pd.ellipse(
            (radius, radius // 2, radius * 3, radius * 3),
            fill=(249, 190 + idx % 3 * 12, 255, 155),
            outline=(255, 238, 255, 185),
            width=1,
        )
        petal = petal.rotate(angle, resample=Image.Resampling.BICUBIC, expand=True)
        overlay.alpha_composite(petal, (int(x - petal.width / 2), int(y - petal.height / 2)))

    crystal_points = [
        (0.19, 0.20), (0.37, 0.12), (0.51, 0.50), (0.66, 0.18),
        (0.83, 0.10), (0.88, 0.68), (0.63, 0.79), (0.33, 0.76),
        (0.72, 0.56), (0.94, 0.88), (0.15, 0.52),
    ]
    glow = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    od = ImageDraw.Draw(overlay)
    for idx, (px, py) in enumerate(crystal_points):
        pulse = max(0.0, math.sin(2 * math.pi * (t * (1 + idx % 3) + idx * 0.19))) ** 5
        if pulse < 0.05:
            continue
        x, y = int(px * w), int(py * h)
        r = int((3 + 8 * pulse) * w / 720)
        gd.ellipse((x - r * 2, y - r * 2, x + r * 2, y + r * 2), fill=(220, 192, 255, int(80 * pulse)))
        alpha = int(235 * pulse)
        od.line((x - r * 2, y, x + r * 2, y), fill=(255, 250, 255, alpha), width=1)
        od.line((x, y - r * 2, x, y + r * 2), fill=(255, 250, 255, alpha), width=1)
        od.polygon(((x, y - r), (x + r // 2, y), (x, y + r), (x - r // 2, y)), fill=(255, 255, 255, alpha))
    glow = glow.filter(ImageFilter.GaussianBlur(max(2, w // 240)))
    overlay = Image.alpha_composite(glow, overlay)
    return Image.alpha_composite(frame.convert("RGBA"), overlay).convert("RGB")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--blink", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--size", type=int, default=720)
    parser.add_argument("--frames", type=int, default=48)
    args = parser.parse_args()

    base = Image.open(args.base).convert("RGB").resize((args.size, args.size), Image.Resampling.LANCZOS)
    closed = Image.open(args.blink).convert("RGB").resize(base.size, Image.Resampling.LANCZOS)
    w, h = base.size
    blink_mask = smooth_ellipse_mask(
        base.size,
        [
            (int(.435*w), int(.255*h), int(.555*w), int(.355*h)),
            (int(.565*w), int(.285*h), int(.680*w), int(.385*h)),
        ],
    )
    yy, xx = np.mgrid[0:h, 0:w]
    head = np.exp(-(((xx - .55*w)/(.24*w))**2 + ((yy - .30*h)/(.28*h))**2) * 2.0)
    hair_back = np.exp(-(((xx - .22*w)/(.30*w))**2 + ((yy - .53*h)/(.42*h))**2) * 1.7)
    hair_lower = np.exp(-(((xx - .35*w)/(.38*w))**2 + ((yy - .74*h)/(.32*h))**2) * 2.2)

    frames = []
    for index in range(args.frames):
        t = index / args.frames
        phase = 2 * math.pi * t
        turn = math.sin(phase)
        dx = 3.2 * turn * head
        dy = 1.1 * (math.cos(phase) - 1) * head
        dx += (4.6 * math.sin(phase + .65) + 1.7 * math.sin(phase * 2 + .2)) * hair_back
        dy += 1.7 * math.cos(phase + .4) * hair_back
        dx += 2.8 * math.sin(phase - .35) * hair_lower

        current = remap(base, dx, dy)
        amount = blink_amount(t)
        if amount > .01:
            closed_frame = remap(closed, dx, dy)
            effective = blink_mask.point(lambda p: int(p * amount))
            current = Image.composite(closed_frame, current, effective)
        current = add_petals_and_sparkles(current, t)
        frames.append(current)

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(
        output,
        save_all=True,
        append_images=frames[1:],
        duration=83,
        loop=0,
        optimize=True,
        disposal=2,
    )
    print(f"saved={output.resolve()} frames={len(frames)} size={output.stat().st_size}")


if __name__ == "__main__":
    main()
