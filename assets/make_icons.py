# Renders deer.svg into the app icons: icon.png (window/taskbar) and icon.ico (exe).
# Needs ImageMagick (`convert`) and Pillow. Run from the repo root: python3 assets/make_icons.py
import subprocess, tempfile, os
from PIL import Image

with tempfile.TemporaryDirectory() as tmp:
    big = os.path.join(tmp, "big.png")
    # 1000-unit viewBox at 300 dpi ≈ 3125px, then downscaled for clean edges.
    subprocess.run(["convert", "-background", "none", "-density", "300", "assets/deer.svg", big], check=True)
    icon = Image.open(big).convert("RGBA").resize((1024, 1024), Image.LANCZOS)
png = icon.resize((256, 256), Image.LANCZOS)
png.save("assets/icon.png")
png.save("assets/icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
