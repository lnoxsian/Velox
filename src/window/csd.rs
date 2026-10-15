use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdButtonType {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsdButtonLayout {
    pub left: Vec<CsdButtonType>,
    pub right: Vec<CsdButtonType>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CsdButtonRect {
    pub button_type: CsdButtonType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub center_x: f32,
    pub center_y: f32,
}

impl CsdButtonRect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

impl Default for CsdButtonLayout {
    fn default() -> Self {
        Self {
            left: Vec::new(),
            right: vec![
                CsdButtonType::Minimize,
                CsdButtonType::Maximize,
                CsdButtonType::Close,
            ],
        }
    }
}

impl CsdButtonLayout {
    /// Parse GTK `button-layout` string format (e.g. `":minimize,maximize,close"` or `"close,minimize,maximize:"`).
    pub fn parse(s: &str) -> Self {
        let cleaned = s.trim().trim_matches('\'').trim_matches('"');
        let parts: Vec<&str> = cleaned.split(':').collect();
        if parts.len() != 2 {
            return Self::default();
        }

        let parse_side = |side: &str| -> Vec<CsdButtonType> {
            side.split(',')
                .filter_map(|token| match token.trim() {
                    "close" => Some(CsdButtonType::Close),
                    "maximize" => Some(CsdButtonType::Maximize),
                    "minimize" => Some(CsdButtonType::Minimize),
                    _ => None,
                })
                .collect()
        };

        let left = parse_side(parts[0]);
        let right = parse_side(parts[1]);

        if left.is_empty() && right.is_empty() {
            Self::default()
        } else {
            Self { left, right }
        }
    }

    /// Query the system desktop settings for GTK button layout.
    /// Checks `VELOX_BUTTON_LAYOUT` env var, then queries desktop gsettings
    /// (Cinnamon first if under Cinnamon desktop, otherwise GNOME).
    pub fn detect_system() -> Self {
        if let Ok(val) = std::env::var("VELOX_BUTTON_LAYOUT") {
            if !val.trim().is_empty() {
                return Self::parse(&val);
            }
        }

        let is_cinnamon = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("cinnamon"))
            .unwrap_or(false);

        let query_gnome = || {
            Command::new("gsettings")
                .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if s.contains(':') {
                            return Some(Self::parse(&s));
                        }
                    }
                    None
                })
        };

        let query_cinnamon = || {
            Command::new("gsettings")
                .args(["get", "org.cinnamon.desktop.wm.preferences", "button-layout"])
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if s.contains(':') {
                            return Some(Self::parse(&s));
                        }
                    }
                    None
                })
        };

        if is_cinnamon {
            if let Some(layout) = query_cinnamon() {
                return layout;
            }
            if let Some(layout) = query_gnome() {
                return layout;
            }
        } else {
            if let Some(layout) = query_gnome() {
                return layout;
            }
            if let Some(layout) = query_cinnamon() {
                return layout;
            }
        }

        Self::default()
    }

    /// Computes precise visual and hitbox bounding geometry for every button in the layout.
    pub fn compute_button_rects(
        &self,
        viewport_width: u32,
        csd_height: u32,
    ) -> Vec<CsdButtonRect> {
        let mut rects = Vec::with_capacity(self.left.len() + self.right.len());
        let win_w = viewport_width as f32;
        let win_h = csd_height as f32;
        let center_y = win_h / 2.0;

        // Left buttons
        let _m = self.left.len();
        for (i, &btn_type) in self.left.iter().enumerate() {
            let slot = i as f32;
            let center_x = 22.0 + slot * 32.0;
            let (hit_x, hit_w) = if i == 0 {
                (0.0, 38.0)
            } else {
                (6.0 + slot * 32.0, 32.0)
            };
            rects.push(CsdButtonRect {
                button_type: btn_type,
                x: hit_x,
                y: 0.0,
                width: hit_w,
                height: win_h,
                center_x,
                center_y,
            });
        }

        // Right buttons
        let n = self.right.len();
        for (i, &btn_type) in self.right.iter().enumerate() {
            let slot = (n - 1 - i) as f32;
            let center_x = win_w - 22.0 - slot * 32.0;
            let (hit_x, hit_w) = if i == n - 1 {
                (win_w - 38.0, 38.0)
            } else {
                (win_w - 38.0 - slot * 32.0, 32.0)
            };
            rects.push(CsdButtonRect {
                button_type: btn_type,
                x: hit_x,
                y: 0.0,
                width: hit_w,
                height: win_h,
                center_x,
                center_y,
            });
        }

        rects
    }

    /// Hit-tests a coordinate `(px, py)` against the CSD buttons.
    pub fn hit_test(
        &self,
        px: f32,
        py: f32,
        viewport_width: u32,
        csd_height: u32,
    ) -> Option<CsdButtonType> {
        if py < 0.0 || py >= csd_height as f32 || px < 0.0 || px >= viewport_width as f32 {
            return None;
        }
        let rects = self.compute_button_rects(viewport_width, csd_height);
        for rect in rects {
            if rect.contains(px, py) {
                return Some(rect.button_type);
            }
        }
        None
    }

    /// Returns safe `(min_x, max_x)` bounds for centering title text without overlapping buttons.
    pub fn safe_title_bounds(&self, viewport_width: u32, _csd_height: u32) -> (f32, f32) {
        let win_w = viewport_width as f32;
        let left_w = if self.left.is_empty() {
            16.0
        } else {
            22.0 + (self.left.len() as f32) * 32.0 + 8.0
        };
        let right_w = if self.right.is_empty() {
            win_w - 16.0
        } else {
            win_w - (22.0 + (self.right.len() as f32) * 32.0 + 8.0)
        };
        (left_w, right_w.max(left_w))
    }
}

/// Detects if `(px, py)` falls within the resize border region of the window.
pub fn detect_resize_edge(
    px: f32,
    py: f32,
    width: u32,
    height: u32,
    border_px: f32,
) -> Option<crate::window::WindowResizeEdge> {
    use crate::window::WindowResizeEdge;
    let w = width as f32;
    let h = height as f32;
    if px < 0.0 || px > w || py < 0.0 || py > h {
        return None;
    }

    let on_left = px <= border_px;
    let on_right = px >= w - border_px;
    let on_top = py <= border_px;
    let on_bottom = py >= h - border_px;

    if on_top && on_left {
        Some(WindowResizeEdge::TopLeft)
    } else if on_top && on_right {
        Some(WindowResizeEdge::TopRight)
    } else if on_bottom && on_left {
        Some(WindowResizeEdge::BottomLeft)
    } else if on_bottom && on_right {
        Some(WindowResizeEdge::BottomRight)
    } else if on_top {
        Some(WindowResizeEdge::Top)
    } else if on_bottom {
        Some(WindowResizeEdge::Bottom)
    } else if on_left {
        Some(WindowResizeEdge::Left)
    } else if on_right {
        Some(WindowResizeEdge::Right)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csd_button_layout_parsing() {
        let layout1 = CsdButtonLayout::parse("':minimize,maximize,close'");
        assert_eq!(layout1.left, vec![]);
        assert_eq!(
            layout1.right,
            vec![
                CsdButtonType::Minimize,
                CsdButtonType::Maximize,
                CsdButtonType::Close
            ]
        );

        let layout2 = CsdButtonLayout::parse("'close,minimize,maximize:'");
        assert_eq!(
            layout2.left,
            vec![
                CsdButtonType::Close,
                CsdButtonType::Minimize,
                CsdButtonType::Maximize
            ]
        );
        assert_eq!(layout2.right, vec![]);

        let layout3 = CsdButtonLayout::parse("':close'");
        assert_eq!(layout3.left, vec![]);
        assert_eq!(layout3.right, vec![CsdButtonType::Close]);
    }

    #[test]
    fn test_csd_hit_test_right_aligned() {
        let layout = CsdButtonLayout::default();
        let width = 800;
        let height = 38;

        // Top right corner (800 - 10 = 790, y = 19) should hit Close
        assert_eq!(
            layout.hit_test(790.0, 19.0, width, height),
            Some(CsdButtonType::Close)
        );

        // Middle button (750.0, 19.0) should hit Maximize
        assert_eq!(
            layout.hit_test(750.0, 19.0, width, height),
            Some(CsdButtonType::Maximize)
        );

        // Leftmost right button (720.0, 19.0) should hit Minimize
        assert_eq!(
            layout.hit_test(720.0, 19.0, width, height),
            Some(CsdButtonType::Minimize)
        );

        // Center of window should hit None
        assert_eq!(layout.hit_test(400.0, 19.0, width, height), None);

        // Below titlebar should hit None
        assert_eq!(layout.hit_test(790.0, 42.0, width, height), None);
    }

    #[test]
    fn test_detect_resize_edge() {
        let (w, h) = (800, 600);
        let border = 6.0;

        assert_eq!(
            detect_resize_edge(2.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::TopLeft)
        );
        assert_eq!(
            detect_resize_edge(798.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::TopRight)
        );
        assert_eq!(
            detect_resize_edge(2.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::BottomLeft)
        );
        assert_eq!(
            detect_resize_edge(798.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::BottomRight)
        );
        assert_eq!(
            detect_resize_edge(400.0, 2.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Top)
        );
        assert_eq!(
            detect_resize_edge(400.0, 598.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Bottom)
        );
        assert_eq!(
            detect_resize_edge(2.0, 300.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Left)
        );
        assert_eq!(
            detect_resize_edge(798.0, 300.0, w, h, border),
            Some(crate::window::WindowResizeEdge::Right)
        );
        assert_eq!(detect_resize_edge(400.0, 300.0, w, h, border), None);
    }
}
