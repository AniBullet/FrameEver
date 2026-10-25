//! Before/after benchmark for the editor preview's zero-copy surface handoff.
//!
//! "Before" is the NV12 path this repo shipped until the BGRA change: a
//! compute pass converting the rendered RGBA frame into the Y/UV planes of a
//! 420f IOSurface, painted by gpui with a two-texture YCbCr matrix sample.
//! That code is gone from production, so this example carries a faithful
//! reimplementation (same bind groups, same dispatch geometry, same pool).
//!
//! "After" is the production `RgbaToBgraSurfaceConverter`: a fullscreen
//! triangle blit into a 32BGRA IOSurface, painted with a single passthrough
//! sample.
//!
//! Both paths run on the same device against the same source texture, with
//! identical submit-and-wait harnesses, alternating iterations so GPU clock
//! ramping hits them equally. Two phases are timed per path:
//!
//! * convert — encode + submit + wait for the conversion pass (the renderer
//!   side of the handoff, what `finish_encoder_bgra_surface` pays).
//! * paint — sampling the converted surface into a BGRA target, simulating
//!   gpui's `draw_surfaces` fragment work.
//!
//! Run: `cargo run -p cap-rendering --example preview-surface-benchmark --release`

fn main() {
    eprintln!("This benchmark compares IOSurface preview paths and is macOS-only.");
}
