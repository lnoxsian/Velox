use std::sync::OnceLock;

const ICON_16: &[u8] = include_bytes!("../../../assets/generated_icons/icon_16x16.png");
const ICON_32: &[u8] = include_bytes!("../../../assets/generated_icons/icon_32x32.png");
const ICON_48: &[u8] = include_bytes!("../../../assets/generated_icons/icon_48x48.png");
const ICON_128: &[u8] = include_bytes!("../../../assets/generated_icons/icon_128x128.png");

static NET_WM_ICON_DATA: OnceLock<Vec<u32>> = OnceLock::new();

/// Returns the EWMH `_NET_WM_ICON` buffer containing 16x16, 32x32, 48x48, and 128x128 icons.
/// Format for each icon in the array: [width, height, ARGB_0, ..., ARGB_{width*height-1}]
pub fn get_net_wm_icon_data() -> &'static [u32] {
    NET_WM_ICON_DATA.get_or_init(build_net_wm_icon_data)
}

fn build_net_wm_icon_data() -> Vec<u32> {
    let mut data = Vec::new();
    for png_bytes in [ICON_16, ICON_32, ICON_48, ICON_128] {
        if let Some((w, h, pixels)) = decode_png_to_icon(png_bytes) {
            data.push(w);
            data.push(h);
            data.extend(pixels);
        }
    }
    data
}

/// Decodes an embedded PNG image into (width, height, ARGB u32 pixels).
pub fn decode_png_to_icon(bytes: &[u8]) -> Option<(u32, u32, Vec<u32>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let width = info.width;
    let height = info.height;

    let frame_bytes = &buf[..info.buffer_size()];
    let mut pixels = Vec::with_capacity((width * height) as usize);

    match info.color_type {
        png::ColorType::Rgba => {
            for &[r, g, b, a] in frame_bytes.as_chunks::<4>().0 {
                pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
            }
        }
        png::ColorType::Rgb => {
            for &[r, g, b] in frame_bytes.as_chunks::<3>().0 {
                pixels.push((0xFF << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
            }
        }
        _ => return None,
    }

    if pixels.len() == (width * height) as usize {
        Some((width, height, pixels))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_decoding() {
        let (w, h, pixels) = decode_png_to_icon(ICON_16).expect("Failed to decode 16x16 icon");
        assert_eq!(w, 16);
        assert_eq!(h, 16);
        assert_eq!(pixels.len(), 256);

        let (w, h, pixels) = decode_png_to_icon(ICON_32).expect("Failed to decode 32x32 icon");
        assert_eq!(w, 32);
        assert_eq!(h, 32);
        assert_eq!(pixels.len(), 1024);

        let (w, h, pixels) = decode_png_to_icon(ICON_48).expect("Failed to decode 48x48 icon");
        assert_eq!(w, 48);
        assert_eq!(h, 48);
        assert_eq!(pixels.len(), 2304);

        let (w, h, pixels) = decode_png_to_icon(ICON_128).expect("Failed to decode 128x128 icon");
        assert_eq!(w, 128);
        assert_eq!(h, 128);
        assert_eq!(pixels.len(), 16384);
    }

    #[test]
    fn test_net_wm_icon_data_format() {
        let data = get_net_wm_icon_data();
        assert!(!data.is_empty());

        let mut idx = 0;
        let mut icon_count = 0;
        while idx < data.len() {
            let w = data[idx] as usize;
            let h = data[idx + 1] as usize;
            let pixel_count = w * h;
            assert!(w > 0 && h > 0);
            idx += 2 + pixel_count;
            icon_count += 1;
        }
        assert_eq!(idx, data.len());
        assert_eq!(icon_count, 4);
    }
}
