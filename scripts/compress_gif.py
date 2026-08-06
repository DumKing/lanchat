from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image


def load_frames(source: Path):
    image = Image.open(source)
    frames = []
    durations = []
    for index in range(image.n_frames):
        image.seek(index)
        frames.append(image.convert("RGB").copy())
        durations.append(image.info.get("duration", 80))
    return frames, sum(durations)


def compress(frames, total_duration, output: Path, size: int, count: int, colors: int):
    indexes = [round(i * len(frames) / count) % len(frames) for i in range(count)]
    resized = [frames[i].resize((size, size), Image.Resampling.LANCZOS) for i in indexes]

    sample = Image.new("RGB", (size * 4, size * 2))
    for position, frame in enumerate(resized[:: max(1, count // 8)][:8]):
        sample.paste(frame, ((position % 4) * size, (position // 4) * size))
    palette = sample.quantize(colors=colors, method=Image.Quantize.MEDIANCUT)
    quantized = [
        frame.quantize(palette=palette, dither=Image.Dither.FLOYDSTEINBERG)
        for frame in resized
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    quantized[0].save(
        output,
        save_all=True,
        append_images=quantized[1:],
        duration=round(total_duration / count),
        loop=0,
        optimize=True,
        disposal=2,
    )
    return output.stat().st_size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("source")
    parser.add_argument("output")
    parser.add_argument("--max-bytes", type=int, default=5_000_000)
    args = parser.parse_args()

    source = Path(args.source)
    output = Path(args.output)
    frames, duration = load_frames(source)
    candidates = [
        (520, 32, 112),
        (480, 32, 96),
        (450, 30, 96),
        (420, 30, 80),
        (400, 28, 80),
        (384, 28, 64),
    ]
    for size, count, colors in candidates:
        byte_count = compress(frames, duration, output, size, count, colors)
        print(f"attempt size={size} frames={count} colors={colors} bytes={byte_count}")
        if byte_count <= args.max_bytes:
            print(f"saved={output.resolve()} bytes={byte_count}")
            return
    raise SystemExit("unable to reach requested size")


if __name__ == "__main__":
    main()
