use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use frameever_project::{AnnotationType, ProjectConfiguration};
use image::ImageDecoder;

use crate::i18n::t;

pub const IMAGES_DIR: &str = "images";
pub const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];
const PREVIEW_MAX_DIMENSION: u32 = 2048;
const MAX_IMAGE_DIMENSION: u32 = 16384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedImage {
    pub file: String,
    pub size: (u32, u32),
}

pub struct ExportImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub type ExportImages = HashMap<String, ExportImage>;

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

pub fn import_file(bundle: &Path, source: &Path) -> Result<ImportedImage, String> {
    let bytes = std::fs::read(source).map_err(|error| {
        tracing::error!(path = %source.display(), "image could not be read: {error}");
        t!("Couldn't read this image. Check that the file still exists.").to_string()
    })?;
    import_bytes(bundle, &bytes)
}

pub fn import_bytes(bundle: &Path, bytes: &[u8]) -> Result<ImportedImage, String> {
    let unreadable =
        || t!("Couldn't add this file. FrameEver can't read it as an image.").to_string();
    let format = image::guess_format(bytes).map_err(|_| unreadable())?;
    let (decoded, upright) = decode_upright(bytes, format).map_err(|error| {
        tracing::error!("image could not be decoded: {error}");
        unreadable()
    })?;
    let size = (decoded.width(), decoded.height());
    if size.0 == 0 || size.1 == 0 {
        return Err(unreadable());
    }
    let kept = match format {
        _ if !upright => None,
        image::ImageFormat::Png => Some("png"),
        image::ImageFormat::Jpeg => Some("jpg"),
        image::ImageFormat::WebP => Some("webp"),
        _ => None,
    };
    let unsaved = |error: std::io::Error| {
        tracing::error!(path = %bundle.display(), "image could not be copied: {error}");
        t!("Couldn't copy the image into the project. Check free disk space.").to_string()
    };
    let dir = bundle.join(IMAGES_DIR);
    std::fs::create_dir_all(&dir).map_err(unsaved)?;
    let name = format!("{}.{}", crate::store::new_uuid_v4(), kept.unwrap_or("png"));
    let target = dir.join(&name);
    match kept {
        Some(_) => std::fs::write(&target, bytes).map_err(unsaved)?,
        None => decoded
            .to_rgba8()
            .save_with_format(&target, image::ImageFormat::Png)
            .map_err(|error| unsaved(std::io::Error::other(error)))?,
    }
    frameever_utils::storage::stamp_format_version(bundle).map_err(unsaved)?;
    Ok(ImportedImage {
        file: format!("{IMAGES_DIR}/{name}"),
        size,
    })
}

fn decode_upright(
    bytes: &[u8],
    format: image::ImageFormat,
) -> image::ImageResult<(image::DynamicImage, bool)> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = image::DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    Ok((
        decoded,
        orientation == image::metadata::Orientation::NoTransforms,
    ))
}

pub fn image_path(bundle: &Path, file: &str) -> Option<PathBuf> {
    let relative = Path::new(file);
    relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then(|| bundle.join(relative))
}

pub fn load_rgba(bundle: &Path, file: &str) -> Option<image::RgbaImage> {
    let bytes = std::fs::read(image_path(bundle, file)?).ok()?;
    let format = image::guess_format(&bytes).ok()?;
    Some(
        image::load_from_memory_with_format(&bytes, format)
            .ok()?
            .into_rgba8(),
    )
}

pub fn load_preview(bundle: &Path, file: &str) -> Option<image::RgbaImage> {
    let full = load_rgba(bundle, file)?;
    let longest = full.width().max(full.height());
    if longest <= PREVIEW_MAX_DIMENSION {
        return Some(full);
    }
    let scale = f64::from(PREVIEW_MAX_DIMENSION) / f64::from(longest);
    Some(image::imageops::thumbnail(
        &full,
        ((f64::from(full.width()) * scale) as u32).max(1),
        ((f64::from(full.height()) * scale) as u32).max(1),
    ))
}

pub fn image_files(config: &ProjectConfiguration) -> Vec<String> {
    let mut files: Vec<String> = config
        .annotations
        .iter()
        .filter(|annotation| annotation.annotation_type == AnnotationType::Image)
        .filter_map(|annotation| annotation.image.clone())
        .collect();
    files.sort();
    files.dedup();
    files
}

pub fn export_images(bundle: &Path, config: &ProjectConfiguration) -> ExportImages {
    image_files(config)
        .into_iter()
        .filter_map(|file| {
            let rgba = load_rgba(bundle, &file)?;
            let (width, height) = (rgba.width(), rgba.height());
            Some((
                file,
                ExportImage {
                    rgba: rgba.into_raw(),
                    width,
                    height,
                },
            ))
        })
        .collect()
}

pub fn with_opacity(image: &image::RgbaImage, alpha: f32) -> image::RgbaImage {
    let mut faded = image.clone();
    if alpha < 1. {
        for pixel in faded.pixels_mut() {
            pixel.0[3] = (f32::from(pixel.0[3]) * alpha.max(0.)).round() as u8;
        }
    }
    faded
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use frameever_utils::storage::{FORMAT_VERSION, format_version};

    pub(crate) fn png_bytes(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba(color));
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn old_project(dir: &Path) -> PathBuf {
        let bundle = dir.join("Old.ever");
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(bundle.join("frameever.json"), br#"{"formatVersion":1}"#).unwrap();
        bundle
    }

    #[test]
    fn an_added_image_is_copied_into_the_project_and_outlives_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        let source = dir.path().join("logo.png");
        std::fs::write(&source, png_bytes(6, 4, [10, 20, 30, 255])).unwrap();

        let imported = import_file(&bundle, &source).unwrap();
        assert_eq!(imported.size, (6, 4));
        assert!(imported.file.starts_with("images/"));
        std::fs::remove_file(&source).unwrap();

        let loaded = load_rgba(&bundle, &imported.file).unwrap();
        assert_eq!(loaded.dimensions(), (6, 4));
        assert_eq!(loaded.get_pixel(5, 3).0, [10, 20, 30, 255]);
    }

    #[test]
    fn adding_an_image_moves_an_older_project_to_the_current_format() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        assert_eq!(format_version(&bundle), Some(1));
        import_bytes(&bundle, &png_bytes(2, 2, [0, 0, 0, 255])).unwrap();
        assert_eq!(format_version(&bundle), Some(FORMAT_VERSION));
        assert_eq!(FORMAT_VERSION, 2);
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused_and_nothing_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        assert!(import_bytes(&bundle, b"not an image").is_err());
        assert!(!bundle.join(IMAGES_DIR).exists());
        assert_eq!(format_version(&bundle), Some(1));
    }

    #[test]
    fn a_clipboard_bitmap_is_stored_as_png() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        let mut bmp = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([200, 100, 50, 255]))
            .write_to(&mut std::io::Cursor::new(&mut bmp), image::ImageFormat::Bmp)
            .unwrap();
        let imported = import_bytes(&bundle, &bmp).unwrap();
        assert!(imported.file.ends_with(".png"));
        let loaded = load_rgba(&bundle, &imported.file).unwrap();
        assert_eq!(loaded.get_pixel(0, 0).0, [200, 100, 50, 255]);
    }

    #[test]
    fn a_photo_is_turned_upright_from_its_exif_orientation() {
        use image::ImageEncoder;
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        let rotate_90 = vec![
            0x49, 0x49, 0x2A, 0x00, 0x08, 0, 0, 0, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0, 0,
            0, 0x06, 0x00, 0, 0, 0, 0, 0, 0,
        ];
        let pixels = image::RgbImage::from_pixel(8, 4, image::Rgb([30, 60, 90]));
        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg);
        encoder.set_exif_metadata(rotate_90).unwrap();
        encoder
            .write_image(&pixels, 8, 4, image::ExtendedColorType::Rgb8)
            .unwrap();

        let imported = import_bytes(&bundle, &jpeg).unwrap();
        assert_eq!(imported.size, (4, 8));
        let loaded = load_rgba(&bundle, &imported.file).unwrap();
        assert_eq!(loaded.dimensions(), (4, 8));
    }

    #[test]
    fn an_image_too_large_to_hold_is_refused_and_nothing_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = old_project(dir.path());
        let wide = png_bytes(MAX_IMAGE_DIMENSION + 1, 1, [0, 0, 0, 255]);
        assert!(import_bytes(&bundle, &wide).is_err());
        assert!(!bundle.join(IMAGES_DIR).exists());
    }

    #[test]
    fn an_image_name_cannot_point_outside_the_project() {
        let bundle = Path::new("Shot.ever");
        assert!(image_path(bundle, "images/a.png").is_some());
        assert!(image_path(bundle, "../a.png").is_none());
        assert!(image_path(bundle, "C:\\a.png").is_none());
    }

    #[test]
    fn opacity_scales_the_alpha_channel_only() {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([9, 8, 7, 200]));
        assert_eq!(with_opacity(&image, 0.5).get_pixel(0, 0).0, [9, 8, 7, 100]);
        assert_eq!(with_opacity(&image, 1.).get_pixel(0, 0).0, [9, 8, 7, 200]);
    }
}
