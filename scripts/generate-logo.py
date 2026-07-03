#!/usr/bin/env python3
"""Generate Popeye-themed Deckhand logo assets."""

import math
import os
from PIL import Image, ImageDraw, ImageFont

ASSETS_DIR = os.path.join(os.path.dirname(__file__), "..", "assets")
os.makedirs(ASSETS_DIR, exist_ok=True)

NAVY = (26, 42, 76)
RED = (206, 44, 44)
WHITE = (248, 248, 248)
GREEN = (46, 139, 87)
GOLD = (218, 165, 32)
DARK = (18, 28, 48)


def draw_rounded_rect(draw, xy, radius, fill, outline=None, width=1):
    x0, y0, x1, y1 = xy
    draw.rounded_rectangle(xy, radius=radius, fill=fill, outline=outline, width=width)


def draw_anchor(draw, cx, cy, size, color, thickness):
    """Draw a stylized anchor."""
    # Vertical shaft
    top = cy - size * 0.55
    bottom = cy + size * 0.45
    draw.line([(cx, top), (cx, bottom)], fill=color, width=thickness)

    # Top ring
    ring_r = size * 0.12
    draw.ellipse(
        [cx - ring_r, top - ring_r, cx + ring_r, top + ring_r],
        outline=color,
        width=max(1, thickness // 2),
    )

    # Crossbar
    bar_w = size * 0.45
    bar_y = cy - size * 0.15
    draw.line([(cx - bar_w, bar_y), (cx + bar_w, bar_y)], fill=color, width=thickness)

    # Arrow heads on crossbar
    head = size * 0.12
    for sign in (-1, 1):
        x = cx + sign * bar_w
        draw.polygon(
            [(x, bar_y - head), (x + sign * head * 0.7, bar_y), (x, bar_y + head)],
            fill=color,
        )

    # Bottom flukes (curved arms)
    arm_r = size * 0.32
    # Left arm arc
    bbox = [cx - arm_r * 2, bottom - arm_r, cx, bottom + arm_r]
    draw.arc(bbox, start=180, end=270, fill=color, width=thickness)
    # Right arm arc
    bbox = [cx, bottom - arm_r, cx + arm_r * 2, bottom + arm_r]
    draw.arc(bbox, start=270, end=360, fill=color, width=thickness)

    # Fluke points
    fluke = size * 0.15
    draw.polygon(
        [(cx - arm_r * 2 + fluke, bottom), (cx - arm_r * 2, bottom - fluke), (cx - arm_r * 2, bottom + fluke)],
        fill=color,
    )
    draw.polygon(
        [(cx + arm_r * 2 - fluke, bottom), (cx + arm_r * 2, bottom - fluke), (cx + arm_r * 2, bottom + fluke)],
        fill=color,
    )


def draw_spinach_leaf(draw, cx, cy, w, h, angle, color):
    """Draw a simple spinach leaf rotated by angle (degrees)."""
    im = Image.new("RGBA", (int(w * 2), int(h * 2)), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    # Leaf body
    d.ellipse([w - w, h - h * 0.4, w + w, h + h * 0.4], fill=color)
    d.ellipse([w - w * 0.4, h - h, w + w * 0.4, h + h], fill=color)
    # Stem
    d.line([(w, h), (w, h + h * 0.9)], fill=color, width=max(1, int(w * 0.12)))
    rotated = im.rotate(angle, resample=Image.Resampling.BICUBIC, expand=True)
    # Paste centered at cx, cy
    rw, rh = rotated.size
    draw._image.paste(rotated, (cx - rw // 2, cy - rh // 2), rotated)


def make_main_logo(size=512):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cx, cy = size // 2, size // 2
    margin = size // 18

    # Outer dark ring
    draw.ellipse([margin, margin, size - margin, size - margin], fill=DARK)
    # Inner navy circle
    inner_margin = margin + size // 40
    draw.ellipse(
        [inner_margin, inner_margin, size - inner_margin, size - inner_margin],
        fill=NAVY,
        outline=GOLD,
        width=size // 60,
    )

    # Spinach leaves behind anchor
    leaf_color = (*GREEN, 220)
    draw_spinach_leaf(draw, cx - size * 0.22, cy - size * 0.18, size * 0.16, size * 0.28, -35, leaf_color)
    draw_spinach_leaf(draw, cx + size * 0.24, cy - size * 0.20, size * 0.15, size * 0.26, 40, leaf_color)
    draw_spinach_leaf(draw, cx - size * 0.05, cy + size * 0.28, size * 0.14, size * 0.24, 175, leaf_color)

    # Anchor
    draw_anchor(draw, cx, cy, size * 0.50, WHITE, size // 22)

    # Sailor collar chevrons at bottom
    collar_h = size * 0.18
    y_base = size - margin - size * 0.05
    points_left = [
        (cx - size * 0.30, y_base),
        (cx, y_base - collar_h),
        (cx - size * 0.10, y_base),
    ]
    points_right = [
        (cx + size * 0.30, y_base),
        (cx, y_base - collar_h),
        (cx + size * 0.10, y_base),
    ]
    draw.polygon(points_left, fill=RED)
    draw.polygon(points_right, fill=RED)

    return img


def make_wide_logo(width=1280, height=384):
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Background badge area
    pad = height // 12
    draw_rounded_rect(draw, (pad, pad, width - pad, height - pad), radius=height // 8, fill=NAVY, outline=GOLD, width=height // 80)

    # Logo circle on the left
    logo_size = int(height * 0.62)
    logo = make_main_logo(logo_size)
    img.paste(logo, (height // 4, (height - logo_size) // 2), logo)

    # Text
    try:
        font_large = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", int(height * 0.32))
        font_small = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", int(height * 0.12))
    except Exception:
        font_large = ImageFont.load_default()
        font_small = ImageFont.load_default()

    text_x = height // 4 + logo_size + height // 10
    draw.text((text_x, height * 0.22), "DECKHAND", fill=WHITE, font=font_large)
    draw.text((text_x, height * 0.58), "Cargo workspace hygiene", fill=GOLD, font=font_small)

    # Small spinach leaf accent
    draw_spinach_leaf(draw, width - height // 4, height // 3, height * 0.10, height * 0.16, -20, (*GREEN, 200))

    return img


def make_favicon(size=64):
    return make_main_logo(size)


def main():
    main_logo = make_main_logo(512)
    main_logo.save(os.path.join(ASSETS_DIR, "logo.png"))

    wide_logo = make_wide_logo(1280, 384)
    wide_logo.save(os.path.join(ASSETS_DIR, "logo-wide.png"))

    favicon = make_favicon(64)
    favicon.save(os.path.join(ASSETS_DIR, "favicon.png"))

    print("Generated:")
    for name in ("logo.png", "logo-wide.png", "favicon.png"):
        path = os.path.join(ASSETS_DIR, name)
        print(f"  {path} ({os.path.getsize(path)} bytes)")


if __name__ == "__main__":
    main()
