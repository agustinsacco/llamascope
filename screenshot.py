#!/usr/bin/env python3
import os
import sys
import subprocess
from PIL import Image, ImageDraw, ImageFont
from rich.text import Text
from rich.console import Console
from rich.terminal_theme import TerminalTheme, ColorTriplet

# Define Catppuccin Mocha Terminal Theme
theme = TerminalTheme(
    background=(30, 30, 46),      # Base
    foreground=(205, 214, 244),  # Text
    normal=[
        (69, 71, 90),     # Black (Surface 1)
        (243, 139, 168),  # Red
        (166, 227, 161),  # Green
        (249, 226, 175),  # Yellow
        (137, 180, 250),  # Blue
        (245, 194, 231),  # Magenta
        (148, 226, 213),  # Cyan
        (186, 194, 222),  # White (Subtext 1)
    ],
    bright=[
        (88, 91, 112),    # Bright Black (Surface 2)
        (243, 139, 168),  # Bright Red
        (166, 227, 161),  # Bright Green
        (249, 226, 175),  # Bright Yellow
        (137, 180, 250),  # Bright Blue
        (245, 194, 231),  # Bright Magenta
        (148, 226, 213),  # Bright Cyan
        (166, 173, 200),  # Bright White (Subtext 0)
    ]
)

def get_tmux_dimensions(session_name):
    res = subprocess.run(
        ["tmux", "display-message", "-p", "-F", "#{pane_width} #{pane_height}", "-t", session_name],
        capture_output=True, text=True, check=True
    )
    w, h = res.stdout.strip().split()
    return int(w), int(h)

def capture_tmux_pane(session_name):
    res = subprocess.run(
        ["tmux", "capture-pane", "-e", "-t", session_name, "-p"],
        capture_output=True, text=True, check=True
    )
    return res.stdout

def main():
    session_name = "llamascope"
    if len(sys.argv) > 1:
        output_path = sys.argv[1]
    else:
        # Default output path in the conversation brain directory
        output_path = "/home/stark/.gemini/antigravity-cli/brain/69ff1c80-b96f-4187-9b9f-ac6cf8510115/llamascope_screenshot.png"
        
    print(f"Capturing tmux session '{session_name}'...")
    try:
        width, height = get_tmux_dimensions(session_name)
        pane_data = capture_tmux_pane(session_name)
    except subprocess.CalledProcessError as e:
        print(f"Error communicating with tmux: {e}")
        print("Please ensure the tmux session is running.")
        sys.exit(1)
        
    print(f"Pane dimensions: {width} columns x {height} rows")
    
    font_path_reg = '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
    font_path_bold = '/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf'
    font_path_sans = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
    font_size = 20
    cell_width = 12
    cell_height = 24
    
    font_reg = ImageFont.truetype(font_path_reg, font_size)
    font_bold = ImageFont.truetype(font_path_bold, font_size)
    font_sans = ImageFont.truetype(font_path_sans, font_size)
    ascent, descent = font_reg.getmetrics()
    
    # Initialize image
    bg_rgb = (theme.background_color.red, theme.background_color.green, theme.background_color.blue)
    img = Image.new("RGB", (width * cell_width, height * cell_height), bg_rgb)
    draw = ImageDraw.Draw(img)
    
    console = Console()
    lines = pane_data.splitlines()
    
    for r in range(height):
        if r < len(lines):
            line = lines[r]
        else:
            line = ""
            
        t = Text.from_ansi(line)
        t.set_length(width)
        
        y_rect_top = r * cell_height
        y_rect_bottom = (r + 1) * cell_height
        y_text = y_rect_top + ascent
        
        for c in range(width):
            char = t.plain[c]
            style = t.get_style_at_offset(console, c)
            
            x_left = c * cell_width
            x_right = (c + 1) * cell_width
            
            # Resolve background color
            char_bg_rgb = None
            if style.bgcolor:
                bg_triplet = style.bgcolor.get_truecolor(theme, foreground=False)
                char_bg_rgb = (bg_triplet.red, bg_triplet.green, bg_triplet.blue)
                
            if char_bg_rgb and char_bg_rgb != bg_rgb:
                draw.rectangle(
                    [x_left, y_rect_top, x_right, y_rect_bottom],
                    fill=char_bg_rgb
                )
                
            # Resolve foreground color
            char_fg_rgb = (theme.foreground_color.red, theme.foreground_color.green, theme.foreground_color.blue)
            if style.color:
                fg_triplet = style.color.get_truecolor(theme, foreground=True)
                char_fg_rgb = (fg_triplet.red, fg_triplet.green, fg_triplet.blue)
                
            # Determine font style (bold, regular, or braille fallback)
            if '\u2800' <= char <= '\u28ff':
                font = font_sans
            else:
                font = font_bold if style.bold else font_reg
            
            # Draw char if not space or if it has custom style
            if char != ' ' or char_bg_rgb:
                draw.text((x_left, y_text), char, font=font, fill=char_fg_rgb, anchor="ls")
                
    # Save the output image
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    img.save(output_path)
    print(f"Screenshot successfully saved to: {output_path}")

if __name__ == "__main__":
    main()
