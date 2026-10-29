"""Generate Fairway's simple application mark using Pillow (build-time only)."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent / 'src-tauri/icons'
root.mkdir(parents=True, exist_ok=True)

def icon(size):
    image = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=size * .22, fill='#23715b')
    # A clean F mark with a leaf-shaped top stroke.
    draw.rounded_rectangle((size*.28, size*.25, size*.40, size*.77), radius=size*.035, fill='#ffffff')
    draw.rounded_rectangle((size*.36, size*.48, size*.65, size*.59), radius=size*.035, fill='#ffffff')
    draw.polygon([(size*.36,size*.25), (size*.73,size*.25), (size*.64,size*.37), (size*.36,size*.37)], fill='#c9e5d5')
    return image

for size in (32, 128, 256, 512): icon(size).save(root / f'{size}x{size}.png')
icon(256).save(root / 'icon.ico', sizes=[(16,16), (32,32), (48,48), (64,64), (128,128), (256,256)])
icon(1024).save(root / 'icon.icns')
