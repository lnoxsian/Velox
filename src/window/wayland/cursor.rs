use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use wayland_client::QueueHandle;
use wayland_client::protocol::{wl_compositor, wl_pointer, wl_shm, wl_surface};

use super::WaylandState;
use super::shm::WaylandShmBuffer;
use crate::window::event::CursorIcon;

pub struct LoadedCursor {
    pub buffer: WaylandShmBuffer,
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
}

pub struct WaylandCursorManager {
    pub cursor_surface: wl_surface::WlSurface,
    pub cursors: HashMap<CursorIcon, LoadedCursor>,
    pub active_icon: CursorIcon,
}

impl WaylandCursorManager {
    pub fn new(
        compositor: &wl_compositor::WlCompositor,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandState>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let cursor_surface = compositor.create_surface(qh, ());
        let (theme_name, target_size) = detect_cursor_theme_and_size();

        let icons = [
            (
                CursorIcon::Default,
                &["left_ptr", "default", "arrow", "top_left_arrow"][..],
            ),
            (CursorIcon::Text, &["xterm", "text", "ibeam"][..]),
            (
                CursorIcon::Pointer,
                &["pointer", "pointing_hand", "hand2", "hand1"][..],
            ),
            (
                CursorIcon::ColResize,
                &[
                    "col-resize",
                    "sb_h_double_arrow",
                    "ew-resize",
                    "h_double_arrow",
                    "split_h",
                ][..],
            ),
            (
                CursorIcon::RowResize,
                &[
                    "row-resize",
                    "sb_v_double_arrow",
                    "ns-resize",
                    "v_double_arrow",
                    "split_v",
                ][..],
            ),
        ];

        let mut cursors = HashMap::new();

        for (icon, aliases) in icons {
            let loaded = load_cursor_image(&theme_name, aliases, target_size)
                .unwrap_or_else(|| create_fallback_cursor(icon));

            if let Ok(mut shm_buf) = WaylandShmBuffer::new(shm, qh, loaded.width, loaded.height) {
                let dest = shm_buf.pixels_mut();
                let copy_len = dest.len().min(loaded.pixels.len());
                dest[..copy_len].copy_from_slice(&loaded.pixels[..copy_len]);

                cursors.insert(
                    icon,
                    LoadedCursor {
                        buffer: shm_buf,
                        width: loaded.width,
                        height: loaded.height,
                        hotspot_x: loaded.hotspot_x,
                        hotspot_y: loaded.hotspot_y,
                    },
                );
            }
        }

        Ok(Self {
            cursor_surface,
            cursors,
            active_icon: CursorIcon::Default,
        })
    }

    pub fn apply_cursor(&mut self, serial: u32, icon: CursorIcon, pointer: &wl_pointer::WlPointer) {
        self.active_icon = icon;
        if let Some(cursor) = self.cursors.get(&icon) {
            self.cursor_surface
                .attach(Some(&cursor.buffer.buffer), 0, 0);
            self.cursor_surface
                .damage_buffer(0, 0, cursor.width as i32, cursor.height as i32);
            self.cursor_surface.commit();
            pointer.set_cursor(
                serial,
                Some(&self.cursor_surface),
                cursor.hotspot_x as i32,
                cursor.hotspot_y as i32,
            );
        }
    }
}

pub struct CursorImageData {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub pixels: Vec<u32>,
}

fn detect_cursor_theme_and_size() -> (String, u32) {
    let mut theme = std::env::var("XCURSOR_THEME").ok();
    let mut size = std::env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|s| s.parse::<u32>().ok());

    if theme.is_none() || size.is_none() {
        // Query GNOME or Cinnamon gsettings
        if let Ok(out) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "cursor-theme"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                let cleaned = s.trim().trim_matches('\'').trim_matches('"');
                if !cleaned.is_empty() && theme.is_none() {
                    theme = Some(cleaned.to_string());
                }
            }
        }

        if size.is_none() {
            if let Ok(out) = Command::new("gsettings")
                .args(["get", "org.gnome.desktop.interface", "cursor-size"])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Ok(sz) = s.trim().parse::<u32>() {
                        if sz > 0 {
                            size = Some(sz);
                        }
                    }
                }
            }
        }
    }

    (
        theme.unwrap_or_else(|| "default".to_string()),
        size.unwrap_or(24),
    )
}

fn find_cursor_file(theme: &str, aliases: &[&str]) -> Option<PathBuf> {
    let mut search_dirs = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        let p = Path::new(&home);
        search_dirs.push(p.join(".icons"));
        search_dirs.push(p.join(".local/share/icons"));
    }
    search_dirs.push(PathBuf::from("/usr/share/icons"));
    search_dirs.push(PathBuf::from("/usr/local/share/icons"));

    let themes = [theme, "default", "Adwaita", "DMZ-White"];

    for t in themes {
        for base in &search_dirs {
            let cursor_dir = base.join(t).join("cursors");
            for &alias in aliases {
                let candidate = cursor_dir.join(alias);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn load_cursor_image(theme: &str, aliases: &[&str], target_size: u32) -> Option<CursorImageData> {
    let path = find_cursor_file(theme, aliases)?;
    let data = fs::read(path).ok()?;
    parse_xcursor(&data, target_size)
}

/// Parse Xcursor format binary data and extract the image closest to target_size.
pub fn parse_xcursor(data: &[u8], target_size: u32) -> Option<CursorImageData> {
    if data.len() < 16 || &data[0..4] != b"Xcur" {
        return None;
    }

    let header_bytes = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let _version = u32::from_le_bytes(data[8..12].try_into().ok()?);
    let ntoc = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;

    if data.len() < header_bytes + ntoc * 12 {
        return None;
    }

    const XCUR_IMAGE_TYPE: u32 = 0xfffd0002;
    let mut best_candidate: Option<(u32, usize)> = None; // (diff, position)

    for i in 0..ntoc {
        let toc_offset = header_bytes + i * 12;
        let entry_type = u32::from_le_bytes(data[toc_offset..toc_offset + 4].try_into().ok()?);
        let subtype = u32::from_le_bytes(data[toc_offset + 4..toc_offset + 8].try_into().ok()?);
        let position =
            u32::from_le_bytes(data[toc_offset + 8..toc_offset + 12].try_into().ok()?) as usize;

        if entry_type == XCUR_IMAGE_TYPE {
            let diff = (subtype as i32 - target_size as i32).unsigned_abs();
            match best_candidate {
                None => best_candidate = Some((diff, position)),
                Some((best_diff, _)) if diff < best_diff => {
                    best_candidate = Some((diff, position));
                }
                _ => {}
            }
        }
    }

    let (_, pos) = best_candidate?;
    if data.len() < pos + 36 {
        return None;
    }

    let chunk_header = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
    let chunk_type = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?);
    if chunk_type != XCUR_IMAGE_TYPE || chunk_header < 36 {
        return None;
    }

    let width = u32::from_le_bytes(data[pos + 16..pos + 20].try_into().ok()?);
    let height = u32::from_le_bytes(data[pos + 20..pos + 24].try_into().ok()?);
    let hotspot_x = u32::from_le_bytes(data[pos + 24..pos + 28].try_into().ok()?);
    let hotspot_y = u32::from_le_bytes(data[pos + 28..pos + 32].try_into().ok()?);

    let pixel_count = (width as usize) * (height as usize);
    let pixel_bytes_offset = pos + chunk_header;
    let total_pixel_bytes = pixel_count * 4;

    if data.len() < pixel_bytes_offset + total_pixel_bytes {
        return None;
    }

    let mut pixels = Vec::with_capacity(pixel_count);
    for idx in 0..pixel_count {
        let b = pixel_bytes_offset + idx * 4;
        let px = u32::from_le_bytes(data[b..b + 4].try_into().ok()?);
        pixels.push(px);
    }

    Some(CursorImageData {
        width,
        height,
        hotspot_x,
        hotspot_y,
        pixels,
    })
}

/// Generates built-in crisp, anti-aliased bitmap cursors with black-and-white contrast borders
/// ensuring visibility against any light or dark background.
fn create_fallback_cursor(icon: CursorIcon) -> CursorImageData {
    let size = 24u32;
    let mut pixels = vec![0u32; (size * size) as usize];

    let mut put = |x: i32, y: i32, color: u32| {
        if x >= 0 && x < size as i32 && y >= 0 && y < size as i32 {
            pixels[(y as usize) * (size as usize) + (x as usize)] = color;
        }
    };

    let black = 0xFF000000;
    let white = 0xFFFFFFFF;

    match icon {
        CursorIcon::Default => {
            // Classic arrow cursor pointing to (1, 1)
            let shape = [
                "X                       ",
                "XX                      ",
                "X.X                     ",
                "X..X                    ",
                "X...X                   ",
                "X....X                  ",
                "X.....X                 ",
                "X......X                ",
                "X.......X               ",
                "X........X              ",
                "X.....XXXX              ",
                "X..X..X                 ",
                "X.X X..X                ",
                "XX   X..X               ",
                "X     X..X              ",
                "       X..X             ",
                "        XX              ",
            ];
            for (y, row) in shape.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    match ch {
                        'X' => put(x as i32 + 1, y as i32 + 1, black),
                        '.' => put(x as i32 + 1, y as i32 + 1, white),
                        _ => {}
                    }
                }
            }
            CursorImageData {
                width: size,
                height: size,
                hotspot_x: 1,
                hotspot_y: 1,
                pixels,
            }
        }
        CursorIcon::Text => {
            // I-beam cursor centered at (12, 12)
            // Top and bottom horizontal serifs, vertical stem
            for x in 8..=16 {
                put(x, 4, black);
                put(x, 5, white);
                put(x, 6, black);

                put(x, 18, black);
                put(x, 19, white);
                put(x, 20, black);
            }
            for y in 6..18 {
                put(11, y, black);
                put(12, y, white);
                put(13, y, black);
            }
            CursorImageData {
                width: size,
                height: size,
                hotspot_x: 12,
                hotspot_y: 12,
                pixels,
            }
        }
        CursorIcon::Pointer => {
            // Pointing hand cursor
            let shape = [
                "   XX                   ",
                "  X..X                  ",
                "  X..X                  ",
                "  X..X                  ",
                "  X..X   XX   XX        ",
                "  X..X  X..X X..X   XX  ",
                "  X..X  X..X X..X  X..X ",
                "  X..X  X..X X..X  X..X ",
                " XX..X  X..X X..X  X..X ",
                "X..X.X  X..X X..X  X..X ",
                "X....XXXX..XXX..XXXX..X ",
                "X.....................X ",
                " X....................X ",
                "  X...................X ",
                "  X..................X  ",
                "   X................X   ",
                "   X................X   ",
                "    XXXXXXXXXXXXXXXX    ",
            ];
            for (y, row) in shape.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    match ch {
                        'X' => put(x as i32 + 3, y as i32 + 2, black),
                        '.' => put(x as i32 + 3, y as i32 + 2, white),
                        _ => {}
                    }
                }
            }
            CursorImageData {
                width: size,
                height: size,
                hotspot_x: 5,
                hotspot_y: 2,
                pixels,
            }
        }
        CursorIcon::ColResize => {
            // Horizontal split / resize double arrow <-|->
            for y in 7..=17 {
                put(12, y, black);
            }
            for dx in 0..6 {
                let left_x = 10 - dx;
                let right_x = 14 + dx;
                let half_h = dx / 2;
                for dy in -half_h..=half_h {
                    put(left_x, 12 + dy, white);
                    put(right_x, 12 + dy, white);
                }
                put(left_x, 12 - half_h - 1, black);
                put(left_x, 12 + half_h + 1, black);
                put(right_x, 12 - half_h - 1, black);
                put(right_x, 12 + half_h + 1, black);
            }
            for x in 5..=19 {
                put(x, 11, black);
                put(x, 12, white);
                put(x, 13, black);
            }
            CursorImageData {
                width: size,
                height: size,
                hotspot_x: 12,
                hotspot_y: 12,
                pixels,
            }
        }
        CursorIcon::RowResize => {
            // Vertical split / resize double arrow
            for x in 7..=17 {
                put(x, 12, black);
            }
            for dy in 0..6 {
                let top_y = 10 - dy;
                let bottom_y = 14 + dy;
                let half_w = dy / 2;
                for dx in -half_w..=half_w {
                    put(12 + dx, top_y, white);
                    put(12 + dx, bottom_y, white);
                }
                put(12 - half_w - 1, top_y, black);
                put(12 + half_w + 1, top_y, black);
                put(12 - half_w - 1, bottom_y, black);
                put(12 + half_w + 1, bottom_y, black);
            }
            for y in 5..=19 {
                put(11, y, black);
                put(12, y, white);
                put(13, y, black);
            }
            CursorImageData {
                width: size,
                height: size,
                hotspot_x: 12,
                hotspot_y: 12,
                pixels,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_cursors_generation() {
        for icon in [
            CursorIcon::Default,
            CursorIcon::Text,
            CursorIcon::Pointer,
            CursorIcon::ColResize,
            CursorIcon::RowResize,
        ] {
            let cursor = create_fallback_cursor(icon);
            assert_eq!(cursor.width, 24);
            assert_eq!(cursor.height, 24);
            assert!(cursor.hotspot_x < 24);
            assert!(cursor.hotspot_y < 24);
            assert!(cursor.pixels.iter().any(|&p| p != 0));
        }
    }

    #[test]
    fn test_parse_invalid_xcursor() {
        assert!(parse_xcursor(b"invalid data", 24).is_none());
        assert!(parse_xcursor(b"Xcur\0\0\0\0", 24).is_none());
    }
}
