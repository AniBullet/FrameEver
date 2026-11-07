use std::path::Path;

use frameever_video_decode::MediaFoundationDecoder;

#[test]
fn media_foundation_eof_is_clean_and_seek_restores_cpu_readback() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/video-decode/tests/fixtures/h264-decoder-lifecycle.mp4");
    let mut decoder = MediaFoundationDecoder::new(path).unwrap();
    let first = decoder.read_nv12().unwrap().unwrap();
    for _ in 1..60 {
        assert!(decoder.read_nv12().unwrap().is_some());
    }
    assert!(decoder.read_nv12().unwrap().is_none());
    assert!(decoder.read_nv12().unwrap().is_none());
    decoder.seek(0).unwrap();
    let repeated = decoder.read_nv12().unwrap().unwrap();
    assert_eq!(repeated.pts, first.pts);
}

#[test]
fn media_foundation_eof_is_clean_for_gpu_samples() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/video-decode/tests/fixtures/h264-decoder-lifecycle.mp4");
    let mut decoder = MediaFoundationDecoder::new(path).unwrap();
    for _ in 0..60 {
        assert!(decoder.read_sample().unwrap().is_some());
    }
    assert!(decoder.read_sample().unwrap().is_none());
    assert!(decoder.read_sample().unwrap().is_none());
    decoder.seek(0).unwrap();
    assert!(decoder.read_sample().unwrap().is_some());
}
