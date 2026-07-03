```python
import os
import math
from PIL import Image, ImageDraw, ImageFilter, ImageFont

def transform_points(points, angle_degrees, scale, tx, ty):
    angle = math.radians(angle_degrees)
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    transformed = []
    for x, y in points:
        sx = x * scale
        sy = y * scale
        rx = sx * cos_a - sy * sin_a
        ry = sx * sin_a + sy * cos_a
        transformed.append((rx + tx, ry + ty))
    return transformed

def draw_anchor_shape(draw_ctx, fill, clear):
    # Crescent (bottom curve)
    draw_ctx.ellipse([1024 - 450, 910 - 450, 1024 + 450, 910 + 450], fill=fill)
    draw_ctx.ellipse([1024 - 360, 910 - 360, 1024 + 360, 910 + 360], fill=clear)
    draw_ctx.rectangle([0, 0, 2048, 910], fill=clear)
    
    # Flukes (points at crescent ends)
    draw_ctx.polygon([(574, 910), (520, 760), (620, 810), (664, 910)], fill=fill)
    draw_ctx.polygon([(1474, 910), (1528, 760), (1428, 810), (1384, 910)], fill=fill)
    
    # Bottom peak (center point)
    draw_ctx.polygon([(974, 1360), (1024, 1410), (1074, 1360)], fill=fill)
    
    # Shank (vertical column)
    draw_ctx.rectangle([984, 480, 1064, 1310], fill=fill)
    
    # Stock (horizontal bar)
    draw_ctx.rectangle([724, 580, 1324, 660], fill=fill)
    draw_ctx.ellipse([724 - 40, 620 - 40, 724 + 40, 620 + 40], fill=fill)
    draw_ctx.ellipse([1324 - 40, 620 - 40, 1324 + 40, 620 + 40], fill=fill)
    
    # Ring (top shackle)
    draw_ctx.ellipse([1024 - 110, 440 - 110, 1024 + 110, 440 + 110], fill=fill)
    draw_ctx.ellipse([1024 - 60, 440 - 60, 1024 + 60, 440 + 60], fill=clear)

def main():
    os.makedirs("assets", exist_ok=True)

    # ==========================================
    # PART 1: GENERATE CIRCULAR BADGE (512x512)
    # ==========================================
    # Work at 4x scale (2048x2048) for perfect supersampled anti-aliasing
    badge = Image.new("RGBA", (2048, 2048), (0, 0, 0, 0))
    draw = ImageDraw.Draw(badge)

    # 1. Circle Mask for the Badge
    mask = Image.new("L", (2048, 2048), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.ellipse([60, 60, 1988, 1988], fill=255)

    # 2. Rich Navy Diagonal Gradient Background
    grad = Image.new("RGBA", (2, 2))
    grad.putpixel((0, 0), (17, 34, 64, 255))
    grad.putpixel((1, 0), (27, 50, 90, 255))
    grad.putpixel((0, 1), (11, 23, 46, 255))
    grad.putpixel((1, 1), (22, 42, 78, 255))
    bg = grad.resize((2048, 2048), Image.Resampling.BILINEAR)
    badge.paste(bg, (0, 0), mask)

    # 3. Sailor Collar Red Chevrons at the bottom (with shadow and borders)
    # Chevron shadow layer
    chev_shadow = Image.new("RGBA", (2048, 2048), (0, 0, 0, 0))
    draw_cs = ImageDraw.Draw(chev_shadow)
    for Y_tip, Y_left in [(1750, 1450), (1850, 1550), (1950, 1650)]:
        poly = [
            (1024 - 800, Y_left - 60), (1024, Y_tip - 60), (1024 + 800, Y_left - 60),
            (1024 + 800, Y_left), (1024, Y_tip), (1024 - 800, Y_left)
        ]
        draw_cs.polygon(poly, fill=(10, 20, 40, 120))
    chev_shadow = chev_shadow.filter(ImageFilter.GaussianBlur(15))
    badge.paste(chev_shadow, (10, 10), chev_shadow)

    # Chevron red paths
    for Y_tip, Y_left in [(1750, 1450), (1850, 1550), (1950, 1650)]:
        poly = [
            (1024 - 800, Y_left - 60), (1024, Y_tip - 60), (1024 + 800, Y_left - 60),
            (1024 + 800, Y_left), (1024, Y_tip), (1024 - 800, Y_left)
        ]
        draw.polygon(poly, fill="#C0392B", outline="#962D22", width=4)

    # 4. Spinach Leaves (Wreath-like wrapper around anchor)
    leaf_template = [
        (0, 0),
        (-25, -15),
        (-45, -40),
        (-50, -70),
        (-30, -100),
        (0, -120),  # Tip
        (30, -100),
        (50, -70),
        (45, -40),
        (25, -15)
    ]

    # Leaf Shadow
    leaf_shadow = Image.new("RGBA", (2048, 2048), (0, 0, 0, 0))
    draw_ls = ImageDraw.Draw(leaf_shadow)
    for side in ["left", "right"]:
        angles = [130, 150, 170, 190, 210] if side == "left" else [50, 30, 10, -10, -30]
        for angle in angles:
            rot = angle - 180 if side == "left" else angle
            rad = math.radians(angle)
            tx = 1024 + 620 * math.cos(rad)
            ty = 950 + 620 * math.sin(rad)
            poly = transform_points(leaf_template, rot, 1.0, tx, ty)
            draw_ls.polygon(poly, fill=(10, 20, 40, 100))
    leaf_shadow = leaf_shadow.filter(ImageFilter.GaussianBlur(10))
    badge.paste(leaf_shadow, (10, 10), leaf_shadow)

    # Leaf Green Bodies & Veins
    for side in ["left", "right"]:
        angles = [130, 150, 170, 190, 210] if side == "left" else [50, 30, 10, -10, -30]
        for angle in angles:
            rot = angle - 180 if side == "left" else angle
            rad = math.radians(angle)
            tx = 1024 + 620 * math.cos(rad)
            ty = 950 + 620 * math.sin(rad)
            
            # Leaf fill
            poly = transform_points(leaf_template, rot, 1.0, tx, ty)
            draw.polygon(poly, fill="#27AE60", outline="#1E7E43", width=4)
            
            # Leaf main vein
            vein = transform_points([(0, 20), (0, -100)], rot, 1.0, tx, ty)
            draw.line(vein, fill="#58D68D", width=6)

    # 5. Stylized White Anchor (with Drop Shadow)
    # Anchor Shadow
    anchor_shadow = Image.new("RGBA", (2048, 2048), (0, 0, 0, 0))
    draw_as = ImageDraw.Draw(anchor_shadow)
    draw_anchor_shape(draw_as, fill=(10, 20, 40, 120), clear=(0, 0, 0, 0))
    anchor_shadow = anchor_shadow.filter(ImageFilter.GaussianBlur(15))
    badge.paste(anchor_shadow, (15, 15), anchor_shadow)

    # White Anchor Body
    anchor_white = Image.new("RGBA", (2048, 2048), (0, 0, 0, 0))
    draw_aw = ImageDraw.Draw(anchor_white)
    draw_anchor_shape(draw_aw, fill=(255, 255, 255, 255), clear=(0, 0, 0, 0))
    badge.alpha_composite(anchor_white)

    # 6. Gold Ring Frame
    # Main gold body
    draw.ellipse([1024 - 914, 1024 - 914, 1024 + 914, 1024 + 914], outline="#E6B022", width=100)
    # Shadow and inner/outer highlight borders
    draw.ellipse([1024 - 964, 1024 - 964, 1024 + 964, 1024 + 964], outline="#B7871E", width=4)
    draw.ellipse([1024 - 864, 1024 - 864, 1024 + 864, 1024 + 864], outline="#B7871E", width=4)

    # 7. Apply Circle Clipping Mask to remove any overflow outside the gold border
    badge.putalpha(mask)

    # 8. Downsample to target size 512x512
    logo_final = badge.resize((512, 512), Image.Resampling.LANCZOS)
    logo_final.save("assets/logo.png", "PNG")

    # ==========================================
    # PART 2: GENERATE WIDE BANNER (1280x384)
    # ==========================================
    # Create gradient background
    grad_banner = Image.new("RGBA", (2, 2))
    grad_banner.putpixel((0, 0), (11, 23, 46, 255))
    grad_banner.putpixel((1, 0), (20, 35, 60, 255))
    grad_banner.putpixel((0, 1), (15, 28, 52, 255))
    grad_banner.putpixel((1, 1), (31, 53, 90, 255))
    banner = grad_banner.resize((1280, 384), Image.Resampling.BILINEAR)
    draw_banner = ImageDraw.Draw(banner)

    # Paste small badge logo on the left
    logo_small = logo_final.resize((280, 280), Image.Resampling.LANCZOS)
    banner.paste(logo_small, (80, 52), logo_small)

    # Font Setup
    font_paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "DejaVuSans-Bold.ttf",
        "DejaVuSans.ttf"
    ]
    font_title = None
    font_subtitle = None
    for path in font_paths:
        try:
            font_title = ImageFont.truetype(path, 80)
            font_subtitle = ImageFont.truetype(path, 32)
            break
        except IOError:
            continue

    if font_title is None:
        try:
            font_title = ImageFont.load_default(size=80)
            font_subtitle = ImageFont.load_default(size=32)
        except TypeError:
            font_title = ImageFont.load_default()
            font_subtitle = ImageFont.load_default()

    # Draw Text
    draw_banner.text((400, 160), "DECKHAND", fill="#FFFFFF", font=font_title, anchor="lm")
    draw_banner.text((400, 240), "Cargo workspace hygiene", fill="#E6B022", font=font_subtitle, anchor="lm")

    # Add a premium bottom accent gold line
    draw_banner.rectangle([0, 380, 1280, 384], fill="#E6B022")

    # Save wide banner
    banner.save("assets/logo-wide.png", "PNG")

if __name__ == "__main__":
    main()

# Recommended active workspace: /home/sal/.gemini/antigravity-cli/scratch/deckhand-logo
```
