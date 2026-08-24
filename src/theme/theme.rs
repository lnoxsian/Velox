use crate::screen::cell::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorColorConfig {
    #[default]
    Default,
    Inverted,
    Custom(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabAccentColorConfig {
    #[default]
    Default,
    Ansi(usize),
    DefaultFg,
    DefaultBg,
    Cursor,
    Custom(Color),
}

pub struct Theme {
    pub default_fg: Color,
    pub default_bg: Color,
    pub ansi_colors: [Color; 16],
    pub initial_fg: Color,
    pub initial_bg: Color,
    pub initial_ansi_colors: [Color; 16],
    pub cursor_color: Option<Color>,
    pub cursor_text_color: Option<Color>,
    pub cursor_color_mode: CursorColorConfig,
    pub cursor_text_color_mode: CursorColorConfig,
    pub initial_cursor_color: Option<Color>,
    pub initial_cursor_text_color: Option<Color>,
    pub tab_accent_color: Option<Color>,
    pub tab_accent_color_mode: TabAccentColorConfig,
    pub initial_tab_accent_color: Option<Color>,
    pub initial_tab_accent_color_mode: TabAccentColorConfig,
}

impl Theme {
    pub fn new() -> Self {
        let default_fg = Color {
            r: 248,
            g: 248,
            b: 242,
        };
        let default_bg = Color {
            r: 39,
            g: 40,
            b: 34,
        };
        let ansi_colors = [
            Color {
                r: 39,
                g: 40,
                b: 34,
            }, // Black
            Color {
                r: 249,
                g: 38,
                b: 114,
            }, // Red
            Color {
                r: 166,
                g: 226,
                b: 46,
            }, // Green
            Color {
                r: 244,
                g: 191,
                b: 117,
            }, // Yellow
            Color {
                r: 102,
                g: 217,
                b: 239,
            }, // Blue
            Color {
                r: 174,
                g: 129,
                b: 255,
            }, // Magenta
            Color {
                r: 161,
                g: 239,
                b: 228,
            }, // Cyan
            Color {
                r: 248,
                g: 248,
                b: 242,
            }, // White
            // Brights
            Color {
                r: 117,
                g: 113,
                b: 94,
            }, // Bright Black
            Color {
                r: 249,
                g: 38,
                b: 114,
            }, // Bright Red
            Color {
                r: 166,
                g: 226,
                b: 46,
            }, // Bright Green
            Color {
                r: 244,
                g: 191,
                b: 117,
            }, // Bright Yellow
            Color {
                r: 102,
                g: 217,
                b: 239,
            }, // Bright Blue
            Color {
                r: 174,
                g: 129,
                b: 255,
            }, // Bright Magenta
            Color {
                r: 161,
                g: 239,
                b: 228,
            }, // Bright Cyan
            Color {
                r: 248,
                g: 248,
                b: 240,
            }, // Bright White
        ];
        Self {
            default_fg,
            default_bg,
            ansi_colors,
            initial_fg: default_fg,
            initial_bg: default_bg,
            initial_ansi_colors: ansi_colors,
            cursor_color: None,
            cursor_text_color: None,
            cursor_color_mode: CursorColorConfig::Default,
            cursor_text_color_mode: CursorColorConfig::Default,
            initial_cursor_color: None,
            initial_cursor_text_color: None,
            tab_accent_color: None,
            tab_accent_color_mode: TabAccentColorConfig::Default,
            initial_tab_accent_color: None,
            initial_tab_accent_color_mode: TabAccentColorConfig::Default,
        }
    }

    pub fn from_config(config: &crate::config::config::Config) -> Self {
        let mut theme = Self::new();
        if let Some(fg) = config.default_fg()
            && let Some(c) = crate::config::config::parse_hex_color(fg)
        {
            theme.default_fg = c;
        }
        if let Some(bg) = config.default_bg()
            && let Some(c) = crate::config::config::parse_hex_color(bg)
        {
            theme.default_bg = c;
        }
        if let Some(colors) = &config.colors {
            let fields = [
                (&colors.black, 0),
                (&colors.red, 1),
                (&colors.green, 2),
                (&colors.yellow, 3),
                (&colors.blue, 4),
                (&colors.magenta, 5),
                (&colors.cyan, 6),
                (&colors.white, 7),
                (&colors.bright_black, 8),
                (&colors.bright_red, 9),
                (&colors.bright_green, 10),
                (&colors.bright_yellow, 11),
                (&colors.bright_blue, 12),
                (&colors.bright_magenta, 13),
                (&colors.bright_cyan, 14),
                (&colors.bright_white, 15),
            ];
            for (opt, idx) in &fields {
                if let Some(hex) = opt
                    && let Some(c) = crate::config::config::parse_hex_color(hex)
                {
                    theme.ansi_colors[*idx] = c;
                }
            }
        }
        if let Some(cc) = config.cursor_color() {
            match cc.trim().to_lowercase().as_str() {
                "default" => {
                    theme.cursor_color_mode = CursorColorConfig::Default;
                    theme.cursor_color = None;
                }
                "inverted" | "invert" => {
                    theme.cursor_color_mode = CursorColorConfig::Inverted;
                    theme.cursor_color = None;
                }
                _ => {
                    if let Some(c) = crate::config::config::parse_hex_color(cc) {
                        theme.cursor_color_mode = CursorColorConfig::Custom(c);
                        theme.cursor_color = Some(c);
                    }
                }
            }
        }
        if let Some(ctc) = config.cursor_text_color() {
            match ctc.trim().to_lowercase().as_str() {
                "default" => {
                    theme.cursor_text_color_mode = CursorColorConfig::Default;
                    theme.cursor_text_color = None;
                }
                "inverted" | "invert" => {
                    theme.cursor_text_color_mode = CursorColorConfig::Inverted;
                    theme.cursor_text_color = None;
                }
                _ => {
                    if let Some(c) = crate::config::config::parse_hex_color(ctc) {
                        theme.cursor_text_color_mode = CursorColorConfig::Custom(c);
                        theme.cursor_text_color = Some(c);
                    }
                }
            }
        }
        if let Some(tac) = config.tab_accent_color() {
            match tac.trim().to_lowercase().as_str() {
                "default" | "none" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Default;
                    theme.tab_accent_color = None;
                }
                "black" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(0);
                    theme.tab_accent_color = Some(theme.ansi_colors[0]);
                }
                "red" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(1);
                    theme.tab_accent_color = Some(theme.ansi_colors[1]);
                }
                "green" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(2);
                    theme.tab_accent_color = Some(theme.ansi_colors[2]);
                }
                "yellow" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(3);
                    theme.tab_accent_color = Some(theme.ansi_colors[3]);
                }
                "blue" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(4);
                    theme.tab_accent_color = Some(theme.ansi_colors[4]);
                }
                "magenta" | "purple" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(5);
                    theme.tab_accent_color = Some(theme.ansi_colors[5]);
                }
                "cyan" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(6);
                    theme.tab_accent_color = Some(theme.ansi_colors[6]);
                }
                "white" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Ansi(7);
                    theme.tab_accent_color = Some(theme.ansi_colors[7]);
                }
                "default_fg" | "fg" | "foreground" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::DefaultFg;
                    theme.tab_accent_color = Some(theme.default_fg);
                }
                "default_bg" | "bg" | "background" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::DefaultBg;
                    theme.tab_accent_color = Some(theme.default_bg);
                }
                "cursor" | "cursor_color" => {
                    theme.tab_accent_color_mode = TabAccentColorConfig::Cursor;
                    theme.tab_accent_color = theme.cursor_color;
                }
                _ => {
                    if let Some(c) = crate::config::config::parse_hex_color(tac) {
                        theme.tab_accent_color_mode = TabAccentColorConfig::Custom(c);
                        theme.tab_accent_color = Some(c);
                    }
                }
            }
        }
        theme.save_initial_colors();
        theme
    }

    pub fn save_initial_colors(&mut self) {
        self.initial_fg = self.default_fg;
        self.initial_bg = self.default_bg;
        self.initial_ansi_colors = self.ansi_colors;
        self.initial_cursor_color = self.cursor_color;
        self.initial_cursor_text_color = self.cursor_text_color;
        self.initial_tab_accent_color = self.tab_accent_color;
        self.initial_tab_accent_color_mode = self.tab_accent_color_mode;
    }

    pub fn resolve_tab_accent_color(&self) -> Color {
        match self.tab_accent_color_mode {
            TabAccentColorConfig::Custom(c) => c,
            TabAccentColorConfig::Ansi(idx) => self.ansi_colors[idx % 16],
            TabAccentColorConfig::DefaultFg => self.default_fg,
            TabAccentColorConfig::DefaultBg => self.default_bg,
            TabAccentColorConfig::Cursor => self
                .cursor_color
                .or(self.initial_cursor_color)
                .unwrap_or(self.default_fg),
            TabAccentColorConfig::Default => self
                .tab_accent_color
                .or(self.cursor_color)
                .unwrap_or(self.ansi_colors[4]),
        }
    }

    pub fn resolve_cursor_color(&self, cell_fg: Color) -> Color {
        if let Some(c) = self.cursor_color {
            c
        } else {
            match self.cursor_color_mode {
                CursorColorConfig::Custom(c) => c,
                CursorColorConfig::Inverted | CursorColorConfig::Default => cell_fg,
            }
        }
    }

    pub fn resolve_cursor_text_color(&self, cell_bg: Color) -> Color {
        if let Some(c) = self.cursor_text_color {
            c
        } else {
            match self.cursor_text_color_mode {
                CursorColorConfig::Custom(c) => c,
                CursorColorConfig::Inverted => cell_bg,
                CursorColorConfig::Default => {
                    if self.cursor_color.is_some()
                        || matches!(self.cursor_color_mode, CursorColorConfig::Custom(_))
                    {
                        self.default_bg
                    } else {
                        cell_bg
                    }
                }
            }
        }
    }

    pub fn get_ansi_color(&self, idx: u16, _is_bg: bool) -> Color {
        self.ansi_colors[(idx as usize) % 16]
    }

    pub fn get_256_color(&self, idx: u8) -> Color {
        if idx < 16 {
            self.ansi_colors[idx as usize]
        } else if idx < 232 {
            let val = idx - 16;
            let r = (val / 36) * 51;
            let g = ((val % 36) / 6) * 51;
            let b = (val % 6) * 51;
            Color { r, g, b }
        } else {
            let gray = 8 + (idx - 232) * 10;
            Color {
                r: gray,
                g: gray,
                b: gray,
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config::Config;

    #[test]
    fn test_theme_cursor_colors_default() {
        let config: Config = toml::from_str("").unwrap();
        let theme = Theme::from_config(&config);

        assert_eq!(theme.cursor_color, None);
        assert_eq!(theme.cursor_text_color, None);
        assert_eq!(theme.cursor_color_mode, CursorColorConfig::Default);
        assert_eq!(theme.cursor_text_color_mode, CursorColorConfig::Default);

        let cell_fg = Color {
            r: 10,
            g: 20,
            b: 30,
        };
        let cell_bg = Color {
            r: 40,
            g: 50,
            b: 60,
        };
        // Default mode: cursor color is cell_fg, text color is cell_bg
        assert_eq!(theme.resolve_cursor_color(cell_fg), cell_fg);
        assert_eq!(theme.resolve_cursor_text_color(cell_bg), cell_bg);
    }

    #[test]
    fn test_theme_cursor_colors_inverted() {
        let toml_str = r##"
            [window]
            cursor_color = "inverted"
            cursor_text_color = "inverted"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        let theme = Theme::from_config(&config);

        assert_eq!(theme.cursor_color, None);
        assert_eq!(theme.cursor_text_color, None);
        assert_eq!(theme.cursor_color_mode, CursorColorConfig::Inverted);
        assert_eq!(theme.cursor_text_color_mode, CursorColorConfig::Inverted);

        let cell_fg = Color {
            r: 100,
            g: 150,
            b: 200,
        };
        let cell_bg = Color {
            r: 10,
            g: 20,
            b: 30,
        };
        assert_eq!(theme.resolve_cursor_color(cell_fg), cell_fg);
        assert_eq!(theme.resolve_cursor_text_color(cell_bg), cell_bg);
    }

    #[test]
    fn test_theme_cursor_colors_custom() {
        let toml_str = r##"
            [window]
            cursor_color = "#ff0000"
            cursor_text_color = "#00ff00"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        let theme = Theme::from_config(&config);

        let red = Color { r: 255, g: 0, b: 0 };
        let green = Color { r: 0, g: 255, b: 0 };

        assert_eq!(theme.cursor_color, Some(red));
        assert_eq!(theme.cursor_text_color, Some(green));
        assert_eq!(theme.cursor_color_mode, CursorColorConfig::Custom(red));
        assert_eq!(
            theme.cursor_text_color_mode,
            CursorColorConfig::Custom(green)
        );

        let cell_fg = Color {
            r: 10,
            g: 20,
            b: 30,
        };
        let cell_bg = Color {
            r: 40,
            g: 50,
            b: 60,
        };
        assert_eq!(theme.resolve_cursor_color(cell_fg), red);
        assert_eq!(theme.resolve_cursor_text_color(cell_bg), green);
    }

    #[test]
    fn test_theme_custom_cursor_with_default_text_color() {
        let toml_str = r##"
            [window]
            cursor_color = "#ffffff"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        let theme = Theme::from_config(&config);

        let white = Color {
            r: 255,
            g: 255,
            b: 255,
        };
        assert_eq!(theme.cursor_color, Some(white));
        assert_eq!(theme.cursor_text_color, None);

        let cell_fg = Color {
            r: 10,
            g: 20,
            b: 30,
        };
        let cell_bg = Color {
            r: 40,
            g: 50,
            b: 60,
        };
        assert_eq!(theme.resolve_cursor_color(cell_fg), white);
        // When cursor is custom and text color is default, text color falls back to default_bg
        assert_eq!(theme.resolve_cursor_text_color(cell_bg), theme.default_bg);
    }

    #[test]
    fn test_theme_tab_accent_colors() {
        // 1. Default empty config without explicit tab accent: tab_accent_color is None, falls back to ansi_colors[4]
        let config: Config = toml::from_str("").unwrap();
        let theme = Theme::from_config(&config);
        assert_eq!(theme.tab_accent_color, None);
        assert_eq!(theme.resolve_tab_accent_color(), theme.ansi_colors[4]);

        // 2. Custom cursor color without explicit tab accent: falls back to cursor color
        let toml_cursor = r##"
            [window]
            cursor_color = "#ff00aa"
        "##;
        let config_cursor: Config = toml::from_str(toml_cursor).unwrap();
        let theme_cursor = Theme::from_config(&config_cursor);
        assert_eq!(theme_cursor.tab_accent_color, None);
        assert_eq!(
            theme_cursor.resolve_tab_accent_color(),
            Color {
                r: 255,
                g: 0,
                b: 170
            }
        );

        // 3. Explicit tab accent color in [tabs]
        let toml_tabs = r##"
            [tabs]
            tab_accent_color = "#3b8eea"
        "##;
        let config_tabs: Config = toml::from_str(toml_tabs).unwrap();
        let theme_tabs = Theme::from_config(&config_tabs);
        let accent_color = Color {
            r: 59,
            g: 142,
            b: 234,
        };
        assert_eq!(theme_tabs.tab_accent_color, Some(accent_color));
        assert_eq!(theme_tabs.resolve_tab_accent_color(), accent_color);

        // 4. Tab accent overrides cursor color if both provided
        let toml_both = r##"
            [window]
            cursor_color = "#ff0000"
            [tabs]
            tab_accent_color = "#00ff00"
        "##;
        let config_both: Config = toml::from_str(toml_both).unwrap();
        let theme_both = Theme::from_config(&config_both);
        assert_eq!(
            theme_both.resolve_tab_accent_color(),
            Color { r: 0, g: 255, b: 0 }
        );

        // 5. Default generated config has tab_accent_color = "blue"
        let default_cfg = crate::config::defaults::default_config();
        let theme_default = Theme::from_config(&default_cfg);
        let default_blue = Color {
            r: 36,
            g: 114,
            b: 200,
        };
        assert_eq!(
            theme_default.tab_accent_color_mode,
            TabAccentColorConfig::Ansi(4)
        );
        assert_eq!(theme_default.resolve_tab_accent_color(), default_blue);

        // 6. Named base theme colors from [colors]
        let toml_named_theme = r##"
            [colors]
            default_fg = "#cdd6f4"
            default_bg = "#1e1e2e"
            black = "#45475a"
            red = "#f38ba8"
            green = "#a6e3a1"
            yellow = "#f9e2af"
            blue = "#89b4fa"
            magenta = "#f5c2e7"
            cyan = "#94e2d5"
            white = "#bac2de"
            [tabs]
            tab_accent_color = "red"
        "##;
        let config_named: Config = toml::from_str(toml_named_theme).unwrap();
        let theme_named = Theme::from_config(&config_named);
        assert_eq!(
            theme_named.tab_accent_color_mode,
            TabAccentColorConfig::Ansi(1)
        );
        let expected_red = Color {
            r: 243,
            g: 139,
            b: 168,
        };
        assert_eq!(theme_named.resolve_tab_accent_color(), expected_red);
        assert_eq!(theme_named.tab_accent_color, Some(expected_red));

        // Test all other named color aliases:
        let color_tests = [
            (
                "black",
                0,
                Color {
                    r: 69,
                    g: 71,
                    b: 90,
                },
            ),
            (
                "green",
                2,
                Color {
                    r: 166,
                    g: 227,
                    b: 161,
                },
            ),
            (
                "yellow",
                3,
                Color {
                    r: 249,
                    g: 226,
                    b: 175,
                },
            ),
            (
                "blue",
                4,
                Color {
                    r: 137,
                    g: 180,
                    b: 250,
                },
            ),
            (
                "magenta",
                5,
                Color {
                    r: 245,
                    g: 194,
                    b: 231,
                },
            ),
            (
                "purple",
                5,
                Color {
                    r: 245,
                    g: 194,
                    b: 231,
                },
            ),
            (
                "cyan",
                6,
                Color {
                    r: 148,
                    g: 226,
                    b: 213,
                },
            ),
            (
                "white",
                7,
                Color {
                    r: 186,
                    g: 194,
                    b: 222,
                },
            ),
        ];
        for (name, idx, expected_color) in color_tests {
            let toml = format!(
                r##"
                [colors]
                black = "#45475a"
                red = "#f38ba8"
                green = "#a6e3a1"
                yellow = "#f9e2af"
                blue = "#89b4fa"
                magenta = "#f5c2e7"
                cyan = "#94e2d5"
                white = "#bac2de"
                [tabs]
                tab_accent_color = "{}"
                "##,
                name
            );
            let cfg: Config = toml::from_str(&toml).unwrap();
            let th = Theme::from_config(&cfg);
            assert_eq!(th.tab_accent_color_mode, TabAccentColorConfig::Ansi(idx));
            assert_eq!(th.resolve_tab_accent_color(), expected_color);
        }

        // 7. Special theme color slots: default_fg, default_bg, cursor
        let toml_special = r##"
            [window]
            cursor_color = "#ff5500"
            [colors]
            default_fg = "#112233"
            default_bg = "#445566"
            [tabs]
            tab_accent_color = "default_fg"
        "##;
        let cfg_fg: Config = toml::from_str(toml_special).unwrap();
        let th_fg = Theme::from_config(&cfg_fg);
        assert_eq!(th_fg.tab_accent_color_mode, TabAccentColorConfig::DefaultFg);
        assert_eq!(
            th_fg.resolve_tab_accent_color(),
            Color {
                r: 17,
                g: 34,
                b: 51
            }
        );

        let toml_bg = r##"
            [colors]
            default_fg = "#112233"
            default_bg = "#445566"
            [tabs]
            tab_accent_color = "default_bg"
        "##;
        let cfg_bg: Config = toml::from_str(toml_bg).unwrap();
        let th_bg = Theme::from_config(&cfg_bg);
        assert_eq!(th_bg.tab_accent_color_mode, TabAccentColorConfig::DefaultBg);
        assert_eq!(
            th_bg.resolve_tab_accent_color(),
            Color {
                r: 68,
                g: 85,
                b: 102
            }
        );

        let toml_cur = r##"
            [window]
            cursor_color = "#ff5500"
            [tabs]
            tab_accent_color = "cursor"
        "##;
        let cfg_cur: Config = toml::from_str(toml_cur).unwrap();
        let th_cur = Theme::from_config(&cfg_cur);
        assert_eq!(th_cur.tab_accent_color_mode, TabAccentColorConfig::Cursor);
        assert_eq!(
            th_cur.resolve_tab_accent_color(),
            Color {
                r: 255,
                g: 85,
                b: 0
            }
        );
    }
}
