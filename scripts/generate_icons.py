#!/usr/bin/env python3
"""
Icon Generation Script for Velox Terminal
Generates multi-resolution icons (16x16 up to 1024x1024 and favicon.ico)
from SVG vector assets with maximum PNG compression and optimization.
"""

import sys
import os
import argparse
import subprocess
import shutil
from pathlib import Path

try:
    from PIL import Image
    HAS_PIL = True
except ImportError:
    HAS_PIL = False

DEFAULT_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024]

def render_svg_with_cairosvg(svg_path: Path, output_path: Path, size: int) -> bool:
    try:
        import cairosvg
        cairosvg.svg2png(url=str(svg_path), write_to=str(output_path), output_width=size, output_height=size)
        return True
    except Exception:
        return False

def render_svg_with_resvg(svg_path: Path, output_path: Path, size: int) -> bool:
    if shutil.which("resvg"):
        res = subprocess.run(
            ["resvg", "-w", str(size), "-h", str(size), str(svg_path), str(output_path)],
            capture_output=True
        )
        return res.returncode == 0
    return False

def render_svg_with_rsvg_convert(svg_path: Path, output_path: Path, size: int) -> bool:
    if shutil.which("rsvg-convert"):
        res = subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), "-o", str(output_path), str(svg_path)],
            capture_output=True
        )
        return res.returncode == 0
    return False

def render_svg_with_inkscape(svg_path: Path, output_path: Path, size: int) -> bool:
    if shutil.which("inkscape"):
        res = subprocess.run(
            ["inkscape", "-w", str(size), "-h", str(size), str(svg_path), "-o", str(output_path)],
            capture_output=True
        )
        return res.returncode == 0
    return False

def render_svg_with_imagemagick(svg_path: Path, output_path: Path, size: int) -> bool:
    cmd = shutil.which("magick") or shutil.which("convert")
    if cmd:
        res = subprocess.run(
            [cmd, "-background", "none", "-resize", f"{size}x{size}", str(svg_path), str(output_path)],
            capture_output=True
        )
        return res.returncode == 0
    return False

def render_via_pil_resample(png_source: Path, output_path: Path, size: int) -> bool:
    if not HAS_PIL or not png_source.exists():
        return False
    try:
        with Image.open(png_source) as img:
            resized = img.resize((size, size), Image.Resampling.LANCZOS)
            resized.save(output_path, "PNG", optimize=True, compress_level=9)
        return True
    except Exception as e:
        print(f"Error resampling with PIL: {e}")
        return False

def optimize_png(png_path: Path):
    """Applies maximum PNG zlib compression and strip metadata pass."""
    if not png_path.exists():
        return

    # 1. PIL optimization pass
    if HAS_PIL:
        try:
            with Image.open(png_path) as img:
                img.save(png_path, "PNG", optimize=True, compress_level=9)
        except Exception:
            pass

    # 2. CLI optimizer tool passes if available on host
    if shutil.which("oxipng"):
        subprocess.run(["oxipng", "-o", "max", "--strip", "safe", str(png_path)], capture_output=True)
    elif shutil.which("optipng"):
        subprocess.run(["optipng", "-o7", "-quiet", str(png_path)], capture_output=True)
    elif shutil.which("pngquant"):
        subprocess.run(["pngquant", "--ext", ".png", "--force", "--speed", "1", str(png_path)], capture_output=True)

def generate_icons(svg_file: Path, out_dir: Path, sizes: list[int]):
    out_dir.mkdir(parents=True, exist_ok=True)
    
    png_source = svg_file.with_suffix('.png')
    
    print(f"Generating Velox icon set from: {svg_file}")
    print(f"Output directory: {out_dir}")
    
    generated_pngs = []
    
    for size in sizes:
        out_png = out_dir / f"icon_{size}x{size}.png"
        success = False
        
        # Try SVG renderers in priority order
        renderers = [
            ("cairosvg", render_svg_with_cairosvg),
            ("resvg", render_svg_with_resvg),
            ("rsvg-convert", render_svg_with_rsvg_convert),
            ("inkscape", render_svg_with_inkscape),
            ("imagemagick", render_svg_with_imagemagick),
        ]
        
        for name, renderer in renderers:
            if renderer(svg_file, out_png, size):
                optimize_png(out_png)
                file_size_kb = out_png.stat().st_size / 1024.0
                print(f"  [+] [{name}] Generated {out_png.name} ({size}x{size}) [{file_size_kb:.1f} KB]")
                success = True
                break
                
        if not success:
            # Fallback to high-res PNG resampling via Pillow
            if render_via_pil_resample(png_source, out_png, size):
                optimize_png(out_png)
                file_size_kb = out_png.stat().st_size / 1024.0
                print(f"  [+] [PIL LANCZOS] Generated {out_png.name} ({size}x{size}) [{file_size_kb:.1f} KB]")
                success = True
                
        if not success:
            print(f"  [-] Failed to generate icon for resolution {size}x{size}")
        else:
            generated_pngs.append(out_png)

    # Generate combined ICO format if PIL is available
    if HAS_PIL and (out_dir / "icon_256x256.png").exists():
        try:
            ico_path = out_dir / "velox_icon.ico"
            with Image.open(out_dir / "icon_256x256.png") as img:
                img.save(
                    ico_path,
                    format="ICO",
                    sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
                )
            ico_size_kb = ico_path.stat().st_size / 1024.0
            print(f"  [+] [PIL ICO] Generated combined icon set: {ico_path.name} [{ico_size_kb:.1f} KB]")
        except Exception as e:
            print(f"  [WARNING] Could not generate ICO file: {e}")

    print("Icon generation and optimization completed successfully!")

def main():
    parser = argparse.ArgumentParser(description="Generate multi-resolution icons for Velox Terminal.")
    parser.add_argument(
        "--svg",
        type=Path,
        default=Path("assets/icons/velox_terminal_icon_final.svg"),
        help="Path to input SVG icon"
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("assets/generated_icons"),
        help="Target directory for generated icons"
    )
    parser.add_argument(
        "--sizes",
        nargs="+",
        type=int,
        default=DEFAULT_SIZES,
        help="Space-separated list of resolutions (e.g. 16 32 64 128 256 512 1024)"
    )

    args = parser.parse_args()
    
    if not args.svg.exists():
        png_alt = args.svg.with_suffix(".png")
        if not png_alt.exists():
            print(f"Error: Neither SVG file ({args.svg}) nor fallback PNG ({png_alt}) exists.", file=sys.stderr)
            sys.exit(1)

    generate_icons(args.svg, args.out_dir, args.sizes)

if __name__ == "__main__":
    main()
