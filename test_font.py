import os
from PIL import Image, ImageDraw, ImageFont

def main():
    font_path_mono = '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
    font_path_sans = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
    font_size = 20
    cell_width = 12
    cell_height = 24
    
    font_mono = ImageFont.truetype(font_path_mono, font_size)
    font_sans = ImageFont.truetype(font_path_sans, font_size)
    
    ascent, descent = font_mono.getmetrics()
    
    # Image size: 10 columns, 5 rows
    img = Image.new("RGB", (10 * cell_width, 5 * cell_height), (30, 30, 46))
    draw = ImageDraw.Draw(img)
    
    # Try drawing braille characters
    braille_chars = "⠁⠃⠇⠏⠟⠻⠽⠾⠿"
    for i, char in enumerate(braille_chars):
        x = i * cell_width
        y = ascent
        # Draw with DejaVuSans (non-mono, but supporting Braille)
        draw.text((x, y), char, font=font_sans, fill=(166, 227, 161), anchor="ls")
        
    img.save("/home/stark/apps/llamascope/test_font.png")
    print("Saved test_font.png successfully.")

if __name__ == "__main__":
    main()
