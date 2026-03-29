#!/usr/bin/env python3
from pathlib import Path
import json
import shutil

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "docs" / "brand"
OUT = BRAND / "render"
APP_ICON_SOURCE = ROOT / "Chainworks Forge" / "Assets.xcassets" / "chainworks_app_icon_clean.svg"

SVG_SPECS = [
    (APP_ICON_SOURCE, "chainworks-forge-app-icon.html", 1024, 1024),
    (BRAND / "chainworks-forge-logo-horizontal.svg", "chainworks-forge-logo-horizontal.html", 1600, 560),
    (BRAND / "chainworks-forge-readme-hero.svg", "chainworks-forge-readme-hero.html", 1600, 900),
]

APP_ICON_SET = ROOT / "Chainworks Forge" / "Assets.xcassets" / "AppIcon.appiconset"
ICON_SIZES = [16, 32, 64, 128, 256, 512, 1024]


def write_html(svg_path: Path, html_name: str, width: int, height: int) -> None:
    svg = svg_path.read_text(encoding="utf-8")
    html = f"""<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
html, body {{
  margin: 0;
  padding: 0;
  width: {width}px;
  height: {height}px;
  overflow: hidden;
  background: transparent;
}}
body {{
  display: grid;
  place-items: center;
}}
svg {{
  width: {width}px;
  height: {height}px;
  display: block;
}}
</style>
</head>
<body>
{svg}
</body>
</html>
"""
    (OUT / html_name).write_text(html, encoding="utf-8")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for svg_path, html_name, width, height in SVG_SPECS:
        write_html(svg_path, html_name, width, height)

    contents = {
        "images": [],
        "info": {"author": "xcode", "version": 1},
    }
    for size in [16, 32, 128, 256, 512]:
        for scale, pixel in [("1x", size), ("2x", size * 2)]:
            filename = f"icon_{pixel}x{pixel}.png"
            contents["images"].append(
                {"filename": filename, "idiom": "mac", "scale": scale, "size": f"{size}x{size}"}
            )
    (APP_ICON_SET / "Contents.json").write_text(json.dumps(contents, indent=2) + "\n", encoding="utf-8")

    # Remove stale icon PNGs so regenerated assets are authoritative.
    for file in APP_ICON_SET.glob("icon_*.png"):
        file.unlink()

    # Convenience manifest for the render step.
    manifest = {
        "icon_master_html": str(OUT / "chainworks-forge-app-icon.html"),
        "icon_master_png": str(OUT / "chainworks-forge-app-icon.png"),
        "icon_master_source_svg": str(APP_ICON_SOURCE),
        "horizontal_logo_html": str(OUT / "chainworks-forge-logo-horizontal.html"),
        "horizontal_logo_png": str(OUT / "chainworks-forge-logo-horizontal.png"),
        "readme_hero_html": str(OUT / "chainworks-forge-readme-hero.html"),
        "readme_hero_png": str(OUT / "chainworks-forge-readme-hero.png"),
        "app_icon_set": str(APP_ICON_SET),
        "icon_sizes": ICON_SIZES,
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
