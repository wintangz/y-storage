use std::sync::OnceLock;

use crate::logo_png::{decode_png_rgba, downsample_trimmed_logo_rgba};

/// Original unmodified WT application logo artwork embedded at compile time.
pub const EMBEDDED_WT_LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo-mark.png");

fn decoded_wt_logo() -> &'static (usize, usize, Vec<u8>) {
    static DECODED: OnceLock<(usize, usize, Vec<u8>)> = OnceLock::new();
    DECODED.get_or_init(|| {
        decode_png_rgba(EMBEDDED_WT_LOGO_PNG)
            .expect("embedded assets/logo-mark.png must be valid 8-bit RGBA PNG")
    })
}

/// Rasterizes the original transparent WT logo artwork to `px_size x px_size` RGBA pixels.
#[must_use]
pub fn rasterize_wt_logo_rgba(px_size: usize) -> Vec<u8> {
    let (w, h, ref rgba) = *decoded_wt_logo();
    downsample_trimmed_logo_rgba(w, h, rgba, px_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logo_png::alpha_bbox;

    #[test]
    fn embedded_wt_logo_decodes_and_downsamples_cleanly() {
        assert_eq!(EMBEDDED_WT_LOGO_PNG.len(), 84_018);
        let (w, h, ref rgba) = *decoded_wt_logo();
        assert_eq!((w, h), (1200, 1200));
        assert_eq!(alpha_bbox(w, h, rgba), Some((283, 276, 952, 935)));

        for sz in [16usize, 24, 30, 32] {
            let out = rasterize_wt_logo_rgba(sz);
            assert_eq!(out.len(), sz * sz * 4);
            // Outer corners stay transparent (zero solid backdrop / frame added).
            assert_eq!(out[3], 0, "top-left corner must remain transparent at {sz}px");
            assert!(
                out.chunks_exact(4).any(|px| px[3] > 200),
                "downsampled mark at {sz}px must contain crisp foreground artwork"
            );
        }
    }
}
