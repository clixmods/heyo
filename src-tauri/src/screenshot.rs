//! "Attach a screenshot" profiles: a capture of the screen the user is working on, taken right
//! before the popup appears, sent to the engine with the question. Never written to the log;
//! for the Claude Code engine it goes to a temporary file that is deleted after the request.

/// Whether this OS can take the screenshot (Linux: not yet).
pub const AVAILABLE: bool = cfg!(any(windows, target_os = "macos"));

/// Longest side sent to the model: what Claude reads well without wasting tokens.
#[cfg(any(windows, target_os = "macos"))]
const MAX_SIDE: u32 = 1568;

/// JPEG of the monitor containing the point (physical pixels), or None if it can't be taken.
pub fn capture(x: i32, y: i32) -> Option<Vec<u8>> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        use image::DynamicImage;
        let monitor = xcap::Monitor::from_point(x, y).ok()?;
        let mut img = DynamicImage::ImageRgba8(monitor.capture_image().ok()?);
        if img.width().max(img.height()) > MAX_SIDE {
            img = img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle);
        }
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
            .encode_image(&img.to_rgb8())
            .ok()?;
        Some(jpeg)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (x, y);
        None
    }
}
