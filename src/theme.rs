//! Semantic Catppuccin-inspired tokens shared by every terminal surface.
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Mocha,
    Latte,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub base: Color,
    pub panel: Color,
    pub raised: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub good: Color,
    pub warning: Color,
    pub error: Color,
    pub secondary: Color,
}

impl Theme {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Mocha => Self::Latte,
            Self::Latte => Self::Mocha,
        };
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Mocha => "Mocha",
            Self::Latte => "Latte",
        }
    }
    pub fn palette(self) -> Palette {
        use Color::Rgb;
        match self {
            Self::Mocha => Palette {
                base: Rgb(17, 17, 27),
                panel: Rgb(24, 24, 37),
                raised: Rgb(49, 50, 68),
                text: Rgb(205, 214, 244),
                muted: Rgb(166, 173, 200),
                border: Rgb(88, 91, 112),
                accent: Rgb(137, 180, 250),
                good: Rgb(166, 227, 161),
                warning: Rgb(249, 226, 175),
                error: Rgb(243, 139, 168),
                secondary: Rgb(203, 166, 247),
            },
            Self::Latte => Palette {
                base: Rgb(239, 241, 245),
                panel: Rgb(250, 251, 253),
                raised: Rgb(220, 224, 232),
                text: Rgb(48, 52, 70),
                muted: Rgb(83, 88, 111),
                border: Rgb(140, 143, 161),
                accent: Rgb(29, 75, 165),
                good: Rgb(39, 105, 39),
                warning: Rgb(133, 85, 0),
                error: Rgb(180, 25, 65),
                secondary: Rgb(111, 45, 168),
            },
        }
    }
}
