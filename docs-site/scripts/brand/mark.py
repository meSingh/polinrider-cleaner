"""The PolinRider Scanner mark: the Scanline P.

A rounded ink tile, a white geometric P and a teal scan line through the
bowl. Drawn on a 256 grid (the 64-grid draft times four). Two drawings:

  master  true curves and round joins, for 24px and up (nav, footer,
          app icons, social images). The P's box is centred on the tile.
  small   the same P cut to whole pixels at 16 and 32 (every edge on a
          multiple of 16 units), for favicon.svg and favicon.ico.

The tile is always ink and the P always white, so the mark needs no dark
version. Writes public/favicon.svg, src/assets/brand/mark.svg and
src/assets/brand/apple-touch-icon.svg (full bleed). scripts/brand/render.mjs
then renders the PNG icons and favicon.ico from them in headless Chrome.
"""
import os

SITE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
INK, WHITE, TEAL = '#0f1220', '#ffffff', '#0b6a80'
LABEL = 'PolinRider Scanner and Cleaner'


def master(full_bleed=False):
    tile = (f'<rect width="256" height="256" fill="{INK}"/>' if full_bleed
            else f'<rect x="16" y="16" width="224" height="224" rx="60" fill="{INK}"/>')
    # Stem x 72 to 100, bowl to x 184 and y 54 to 162: the box spans 72 to
    # 184 and 54 to 202, centred on 128 both ways. Stroke 28 everywhere.
    p = (f'<path d="M86 188V68H130A40 40 0 0 1 130 148H86" fill="none" stroke="{WHITE}" '
         'stroke-width="28" stroke-linecap="round" stroke-linejoin="round"/>')
    beam = f'<rect x="52" y="122" width="152" height="20" rx="10" fill="{TEAL}"/>'
    inner = p + beam
    if full_bleed:
        # The tile becomes the whole square; the letter keeps its share of it.
        inner = f'<g transform="translate(128 128) scale(1.142857) translate(-128 -128)">{inner}</g>'
    return tile + inner


def small():
    # 16px: tile 1..15, stem 4..6, P 4..12 both ways, counter 6..8, bowl
    # bottom 8..10, scan line 8..9 from 3 to 13. Doubled at 32px.
    p = ('M64 64H144A48 48 0 0 1 144 160H96V192H64Z'
         'M96 96H144A16 16 0 0 1 144 128H96Z')
    return (f'<rect x="16" y="16" width="224" height="224" rx="60" fill="{INK}"/>'
            f'<path d="{p}" fill="{WHITE}" fill-rule="evenodd"/>'
            f'<rect x="48" y="128" width="160" height="16" fill="{TEAL}"/>')


def svg(body):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" role="img" '
            f'aria-label="{LABEL}">{body}</svg>\n')


def write(rel, text):
    path = os.path.join(SITE, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, 'w').write(text)


write('public/favicon.svg', svg(small()))
write('src/assets/brand/mark.svg', svg(master()))
write('src/assets/brand/apple-touch-icon.svg', svg(master(full_bleed=True)))
print('ok')
