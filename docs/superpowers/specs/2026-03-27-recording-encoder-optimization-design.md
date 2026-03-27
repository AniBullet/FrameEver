# Recording Encoder Optimization Design

## Problem

1. First few seconds of recording show mosaic/pixelation artifacts
2. File sizes are larger than necessary for the quality produced
3. No user control over codec selection

## Root Cause (Mosaic)

- VBR rate control cold start: NVENC uses `rc vbr` without VBV buffer constraints
- MF encoder uses only `MF_MT_AVG_BITRATE` with no buffer model
- libx264 uses `ultrafast` + `zerolatency` (worst compression efficiency)
- No lookahead in any encoder path

## Design

### 1. Fix Mosaic (All Encoders)

**NVENC**: Add VBV constraints and lookahead
- `rc-lookahead: 32` (look ahead 32 frames for better bit allocation)
- `bf: 3` (B frames for better compression)
- `b_adapt: 1`
- Preset: `p4` → `p6` (better quality)
- Tune: `ll` → `hq` (high quality instead of low latency)

**MF Encoder**: Increase initial bitrate allocation margin

**libx264**: Configurable preset based on quality setting

### 2. User-Selectable Encoding

New settings in `GeneralSettings`:
- `recording_codec`: H264 | H265 (default: H264)
- `recording_quality`: Low | Standard | High | Ultra (default: Standard)

Quality mapping:

| Quality | H.264 BPP | H.265 BPP | x264 preset | NVENC preset |
|---------|-----------|-----------|-------------|-------------|
| Low     | 0.08      | 0.05      | medium      | p4 + ll     |
| Standard| 0.15      | 0.10      | medium      | p6 + hq     |
| High    | 0.25      | 0.17      | slow        | p7 + hq     |
| Ultra   | 0.40      | 0.28      | slow        | p7 + hq     |

### 3. AAC Audio Fix

- 320kbps → 128kbps (60% audio size reduction, imperceptible for screen recording)

### 4. HEVC/H.265 Integration

- Wire existing `enc-ffmpeg/src/video/hevc.rs` into Windows pipeline
- NVENC HEVC (`hevc_nvenc`) configuration
- MF HEVC encoder support
- Container: MP4 (compatible)

### 5. Frontend Settings UI

- Codec selector: H.264 / H.265
- Quality selector: Low / Standard / High / Ultra
- Brief descriptions per option

## Files to Modify

### Rust Backend
- `crates/enc-ffmpeg/src/video/h264.rs` - NVENC/libx264 params
- `crates/enc-ffmpeg/src/video/hevc.rs` - HEVC params
- `crates/enc-ffmpeg/src/audio/aac.rs` - AAC bitrate fix
- `crates/enc-mediafoundation/src/video/h264.rs` - MF encoder
- `crates/recording/src/output_pipeline/win.rs` - Encoder selection
- `crates/recording/src/capture_pipeline.rs` - Pipeline config
- `crates/recording/src/instant_recording.rs` - Instant mode
- `apps/desktop/src-tauri/src/general_settings.rs` - Settings types
- `apps/desktop/src-tauri/src/recording.rs` - Recording start

### Frontend
- `apps/desktop/src/routes/(window-chrome)/new-main/index.tsx` - Settings UI

## Expected Results

- Mosaic completely eliminated
- Standard quality: ~30-40% smaller files at same visual quality
- H.265: additional 30-40% reduction
- User can choose codec and quality level
