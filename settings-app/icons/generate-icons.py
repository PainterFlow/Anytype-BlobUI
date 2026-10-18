"""Generate the Windows app icon from the unchanged 4096 px source PNG.

Run with Python and Pillow. Each size is resampled directly from the original
with Lanczos filtering; PNG-compressed ICO frames retain the alpha channel.
"""

from pathlib import Path

from PIL import Image


icons = Path(__file__).resolve().parent
with Image.open(icons / "blob logo 4k.png") as source:
    image = source.convert("RGBA")
    image.save(
        icons / "icon.ico",
        format="ICO",
        sizes=[(size, size) for size in (16, 24, 32, 48, 64, 128, 256)],
    )
