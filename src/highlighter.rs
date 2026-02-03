use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

pub struct SyntectHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntectHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme = get_theme();

        Self { syntax_set, theme }
    }

    fn rgb_to_ansi_color(r: u8, g: u8, b: u8) -> Color {
        Color::Rgb(r, g, b)
    }

    fn syntect_style_to_ansi(style: &SyntectStyle) -> Style {
        let mut ansi_style = Style::new();

        ansi_style = ansi_style.fg(Self::rgb_to_ansi_color(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ));

        if style.font_style.contains(FontStyle::BOLD) {
            ansi_style = ansi_style.bold();
        }
        if style.font_style.contains(FontStyle::ITALIC) {
            ansi_style = ansi_style.italic();
        }
        if style.font_style.contains(FontStyle::UNDERLINE) {
            ansi_style = ansi_style.underline();
        }

        ansi_style
    }
}

impl Highlighter for SyntectHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();

        let syntax = self
            .syntax_set
            .find_syntax_by_extension("sql")
            .or_else(|| Some(self.syntax_set.find_syntax_plain_text()))
            .unwrap();

        let mut highlighter = HighlightLines::new(syntax, &self.theme);

        let ranges = match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges,
            Err(_) => {
                styled.push((Style::new(), line.to_string()));
                return styled;
            }
        };

        for (style, text) in ranges {
            let ansi_style = Self::syntect_style_to_ansi(&style);
            styled.push((ansi_style, text.to_string()));
        }

        styled
    }
}

fn get_theme() -> Theme {
    let theme_set = ThemeSet::load_defaults();
    let dark_theme = theme_set.themes["base16-ocean.dark"].clone();
    let light_theme = theme_set.themes["base16-ocean.light"].clone();
    match theme_mode(QueryOptions::default()) {
        Ok(ThemeMode::Dark) => dark_theme,
        Ok(ThemeMode::Light) => light_theme,
        _ => dark_theme,
    }
}
