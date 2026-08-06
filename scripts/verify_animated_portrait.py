from pathlib import Path
import sys

from PIL import Image, ImageChops, ImageDraw


gif_path = Path(sys.argv[1])
qa_path = Path(sys.argv[2])
image = Image.open(gif_path)
if image.n_frames != 48 or image.size != (720, 720):
    raise SystemExit(f"unexpected GIF metadata: frames={image.n_frames}, size={image.size}")
if image.info.get("loop") != 0:
    raise SystemExit(f"GIF is not configured to loop forever: loop={image.info.get('loop')}")

chosen = [0, 12, 14, 24, 34, 36]
frames = []
for index in chosen:
    image.seek(index)
    frames.append(image.convert("RGB").copy())

sheet = Image.new("RGB", (720 * 3, 720 * 2), "white")
draw = ImageDraw.Draw(sheet)
for position, (index, frame) in enumerate(zip(chosen, frames)):
    x = position % 3 * 720
    y = position // 3 * 720
    sheet.paste(frame, (x, y))
    draw.text((x + 12, y + 12), f"frame {index}", fill="white", stroke_width=2, stroke_fill="black")
sheet.save(qa_path, quality=90)

image.seek(0)
first = image.convert("RGB")
image.seek(image.n_frames - 1)
last = image.convert("RGB")
print(
    f"verified frames={image.n_frames} size={image.size} "
    f"duration_ms={image.info.get('duration')} loop={image.info.get('loop')} "
    f"first_last_changed={ImageChops.difference(first, last).getbbox() is not None} "
    f"qa={qa_path.resolve()}"
)
