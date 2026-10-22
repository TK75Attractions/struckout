"""Writes anchor.svg, pocket_watch_face.svg and pocket_watch_hand.svg next to this script.

The shapes are computed so that the watch face and hand share the canvas center (627, 627).
"""
from __future__ import annotations
from pathlib import Path
import math
import sys
import xml.etree.ElementTree as ET

C = 627
WHITE, BLUE, YELLOW, BLACK = '#FFFFFF', '#0A6CFF', '#FFF200', '#000000'
HEADER = '<svg xmlns="http://www.w3.org/2000/svg" width="1254" height="1254" viewBox="0 0 1254 1254">'

def n(value: float) -> str:
    return f'{value:.1f}'.rstrip('0').rstrip('.')

def point(radius: float, angle: float, cy: float = C) -> str:
    a = math.radians(angle)
    return f'{n(C + radius * math.cos(a))} {n(cy + radius * math.sin(a))}'

def arc(radius: float, start: float, end: float, cy: float = C) -> str:
    return f'M{point(radius, start, cy)} A{n(radius)} {n(radius)} 0 {int(abs(end-start)>180)} {int(end>start)} {point(radius, end, cy)}'

def path(d: str, fill: str, stroke: float = 10) -> str:
    return f'    <path d="{d}" fill="{fill}" stroke="{BLACK}" stroke-width="{n(stroke)}" stroke-linejoin="round"/>'

def group(name: str, comment: str, elements: list[str]) -> str:
    return f'  <!-- {comment} -->\n  <g id="{name}">\n' + '\n'.join(elements) + '\n  </g>'

def write(output: Path, name: str, groups: list[str]) -> None:
    text = HEADER + '\n' + '\n'.join(groups) + '\n</svg>\n'
    ET.fromstring(text)
    with (output / name).open('w', encoding='utf-8', newline='\n') as handle:
        handle.write(text)

def anchor(output: Path) -> None:
    outer, inner, cy = 554, 484, 640
    join_y = cy + math.sqrt(inner**2 - 45**2)
        # The shank and U-shaped crown share one contour; no internal black seam.
    d = (f'M582 260 L672 260 L672 {n(join_y)} '
         f'A484 484 0 0 0 {point(inner,30,cy)} L944 820 L1194 760 '
         f'L{point(outer,30,cy)} A554 554 0 0 1 {point(outer,150,cy)} '
         f'L60 760 L310 820 L{point(inner,150,cy)} '
         f'A484 484 0 0 0 582 {n(join_y)} Z')
    accents = []
    for left, color in [(True, BLUE), (False, YELLOW)]:
        end = 115 if left else 65
        endpoint = 150 if left else 30
        tip, shoulder = ('60 760', '310 820') if left else ('1194 760', '944 820')
        sweep = 0 if left else 1
        accents.append(path(f'M{tip} L{shoulder} L{point(inner,endpoint,cy)} '
            f'A484 484 0 0 {sweep} {point(inner,end,cy)} L{point(outer,end,cy)} '
            f'A554 554 0 0 {1-sweep} {point(outer,endpoint,cy)} Z', color))
    write(output, 'anchor.svg', [
        group('anchor-body', 'Continuous shank, thick curved arms and outward flukes.', [path(d, WHITE)]),
        group('anchor-accents', 'Mirrored arm sectors with ten-pixel black seams.', accents),
        group('anchor-stock', 'Unbroken stock sits in front of the shank.', [path('M60 380 L1194 380 L1164 450 L90 450 Z', WHITE)]),
        group('anchor-ring', 'Clean annulus; the shank never enters its opening.', [
            '    <circle cx="627" cy="170" r="85" fill="none" stroke="#000000" stroke-width="60"/>',
            '    <circle cx="627" cy="170" r="85" fill="none" stroke="#FFFFFF" stroke-width="50"/>'])])

def face(output: Path) -> None:
    # The bezel top is y=162; the stem overlaps it and is drawn after the case.
    attachments = [
        '    <circle cx="627" cy="66" r="32" fill="none" stroke="#000000" stroke-width="30"/>',
        '    <circle cx="627" cy="66" r="32" fill="none" stroke="#FFFFFF" stroke-width="20"/>',
        '    <rect x="595" y="128" width="64" height="50" fill="#FFFFFF" stroke="#000000" stroke-width="10"/>',
        '    <rect x="562" y="98" width="130" height="44" rx="12" fill="#FFFFFF" stroke="#000000" stroke-width="10"/>']
    case = ['    <circle cx="627" cy="627" r="470" fill="#000000"/>']
    colors = [WHITE, YELLOW, WHITE, WHITE, BLUE, WHITE, WHITE, WHITE]
    # Eight equal 42-degree sectors on radius 430, with equal 3-degree breaks.
    for i, color in enumerate(colors):
        start = -112.5 + i * 45 + 1.5
        case.append(f'    <path d="{arc(430,start,start+42)}" fill="none" stroke="{color}" stroke-width="70"/>')
    case.append('    <circle cx="627" cy="627" r="382" fill="none" stroke="#FFFFFF" stroke-width="12"/>')
    marks = []
    for i in range(12):
        cardinal = i % 3 == 0
        radius = 330 if cardinal else 345
        width = 26 if cardinal else 14
        marks.append(f'    <path d="M{point(radius,-90+i*30)} L{point(372,-90+i*30)}" fill="none" stroke="#000000" stroke-width="{width+8}"/>')
        marks.append(f'    <path d="M{point(radius,-90+i*30)} L{point(372,-90+i*30)}" fill="none" stroke="#FFFFFF" stroke-width="{width}"/>')
    # Cardinal marks run from 372 to 330, others from 372 to 345; the hand tip stays at 315.
    write(output, 'pocket_watch_face.svg', [group('watch-case', 'Concentric seventy-pixel bezel segments and inner dial ring.', case),
        group('watch-crown', 'Centered bow, rounded crown and short stem joined to the bezel.', attachments),
        group('hour-marks', 'Twelve radial ticks; no pivot is painted on the face.', marks)])

def hand(output: Path) -> None:
    # Union outline: tapered blade, circular hub and short counterweight.
    hub = 34
    blade_half, tail_half = 20, 15
    blade_y = C - math.sqrt(hub**2-blade_half**2)
    tail_y = C + math.sqrt(hub**2-tail_half**2)
    d = (f'M627 312 L639 342 L647 {n(blade_y)} A34 34 0 0 1 642 {n(tail_y)} '
         f'L642 702 L627 717 L612 702 L612 {n(tail_y)} '
         f'A34 34 0 0 1 607 {n(blade_y)} L615 342 Z')
    boundary = 392
    half_at_boundary = 12 + (blade_half-12) * (boundary-342)/(blade_y-342)
    write(output, 'pocket_watch_hand.svg', [
        group('watch-hand', 'One connected outline; rotate about the canvas center (627,627).', [path(d, WHITE, 8)]),
        group('hand-tip', 'Blue tip covers the upper quarter, with an eight-pixel black seam.', [path(f'M627 312 L639 342 L{n(C+half_at_boundary)} {n(boundary)} L{n(C-half_at_boundary)} {n(boundary)} L615 342 Z', BLUE, 8)]),
        group('hand-tail', 'Yellow counterweight joins the circular hub.', [path(f'M612 {n(tail_y)} L642 {n(tail_y)} L642 702 L627 717 L612 702 Z', YELLOW, 8)]),
        group('hand-pivot', 'Small black center stays on the rotation axis.', ['    <circle cx="627" cy="627" r="10" fill="#000000"/>'])])

def main() -> int:
    output = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent
    anchor(output)
    face(output)
    hand(output)
    print('Generated and XML-parsed three SVG files.')
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
