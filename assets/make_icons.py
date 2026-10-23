# Renders deer.svg into the app icons: icon.png (window/taskbar), icon.ico (exe) and the MSIX package's
# logos in msix/Assets (Store package; named with the scale/targetsize qualifiers makepri picks from).
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

# MSIX logos: the Start menu/taskbar icon (44pt), the Start tile (150pt) and the Store logo (50pt),
# at 100% and 200% scale, plus exact pixel sizes for the taskbar and Explorer (unplated: no tile color behind).
msix = {f"Square44x44Logo.scale-{s}.png": 44 * s // 100 for s in (100, 200)}
msix |= {f"Square150x150Logo.scale-{s}.png": 150 * s // 100 for s in (100, 200)}
msix |= {f"StoreLogo.scale-{s}.png": 50 * s // 100 for s in (100, 200)}
for t in (16, 24, 32, 48, 256):
    msix[f"Square44x44Logo.targetsize-{t}.png"] = t
    msix[f"Square44x44Logo.targetsize-{t}_altform-unplated.png"] = t
os.makedirs("msix/Assets", exist_ok=True)
for name, size in msix.items():
    icon.resize((size, size), Image.LANCZOS).save(os.path.join("msix/Assets", name))
