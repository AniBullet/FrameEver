use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use frameever_project::{
    InputDisplayStyle,
    input_display::{
        GamepadKind, KeyLayout, MAKER_NAME_LENGTH, MAX_PRESET_ATLAS_BYTES, MAX_PRESET_JSON_BYTES,
        PresetRefusal,
    },
};
use serde::{Deserialize, Serialize};

use crate::input_display::{LoadedPreset, PresetShape, layout_preset_for, read_preset};

pub const USER_PRESET_PREFIX: &str = "user:";
pub const MAX_IMPORT_PRESETS: usize = 32;

const MANIFEST_FILE: &str = "frameever-preset.json";
const PRESET_FILE: &str = "preset.json";
const ATLAS_FILE: &str = "atlas.png";

static PRESETS_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub fn use_presets_dir(dir: PathBuf) {
    *PRESETS_DIR
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

pub fn presets_dir() -> Option<PathBuf> {
    PRESETS_DIR
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Manifest {
    Layout {
        name: String,
        #[serde(flatten)]
        layout: KeyLayout,
    },
    Imported {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserPreset {
    pub id: String,
    pub name: String,
    pub layout: Option<KeyLayout>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportRefusal {
    FolderUnreadable {
        detail: String,
    },
    NoPresetFile,
    TooManyPresetFiles {
        count: usize,
    },
    File {
        file: String,
        refusal: PresetRefusal,
    },
    NotSaved {
        detail: String,
    },
}

impl std::fmt::Display for ImportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FolderUnreadable { detail } => {
                write!(f, "the folder could not be read: {detail}")
            }
            Self::NoPresetFile => write!(f, "the folder has no .json preset file"),
            Self::TooManyPresetFiles { count } => write!(
                f,
                "the folder has {count} .json files, more than the {MAX_IMPORT_PRESETS} allowed"
            ),
            Self::File { file, refusal } => write!(f, "{file}: {refusal}"),
            Self::NotSaved { detail } => write!(f, "the preset could not be saved: {detail}"),
        }
    }
}

impl std::error::Error for ImportRefusal {}

const CONTROLLERS: [(GamepadKind, &str); 3] = [
    (GamepadKind::Xbox, "xbox"),
    (GamepadKind::PlayStation, "playstation"),
    (GamepadKind::Switch, "switch"),
];

pub fn for_controller(id: &str, controller: GamepadKind) -> String {
    let (id, _) = drawn_for(id);
    let name = CONTROLLERS
        .iter()
        .find(|(kind, _)| *kind == controller)
        .map_or("xbox", |(_, name)| name);
    format!("{id}@{name}")
}

fn drawn_for(id: &str) -> (&str, GamepadKind) {
    id.rsplit_once('@')
        .and_then(|(preset, name)| {
            let (kind, _) = CONTROLLERS.iter().find(|(_, known)| *known == name)?;
            Some((preset, *kind))
        })
        .unwrap_or((id, GamepadKind::Xbox))
}

pub fn draws_a_gamepad(dir: &Path, id: &str) -> bool {
    let (id, _) = drawn_for(id);
    folder_of(dir, id)
        .and_then(|folder| read_manifest(&folder))
        .is_some_and(|manifest| {
            matches!(manifest, Manifest::Layout { layout, .. } if layout.gamepad.is_some())
        })
}

fn folder_of(dir: &Path, id: &str) -> Option<PathBuf> {
    let folder = id.strip_prefix(USER_PRESET_PREFIX)?;
    let plain = !folder.is_empty()
        && folder.len() <= 64
        && folder
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    plain.then(|| dir.join(folder))
}

fn new_folder_name() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(
        "p-{now:x}-{:x}-{:x}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let length = fs::metadata(path)?.len();
    if length > limit as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is {length} bytes", path.display()),
        ));
    }
    fs::read(path)
}

fn read_manifest(folder: &Path) -> Option<Manifest> {
    let bytes = read_limited(&folder.join(MANIFEST_FILE), MAX_PRESET_JSON_BYTES).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn short_name(name: &str) -> String {
    name.trim().chars().take(MAKER_NAME_LENGTH).collect()
}

fn publish(dir: &Path, write: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<String> {
    fs::create_dir_all(dir)?;
    let name = new_folder_name();
    let staging = dir.join(format!("{name}.tmp"));
    fs::create_dir_all(&staging)?;
    let result = write(&staging).and_then(|()| fs::rename(&staging, dir.join(&name)));
    if let Err(error) = result {
        fs::remove_dir_all(&staging).ok();
        return Err(error);
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(format!("{USER_PRESET_PREFIX}{name}"))
}

pub fn list(dir: &Path) -> Vec<UserPreset> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut presets: Vec<UserPreset> = entries
        .flatten()
        .filter_map(|entry| {
            let id = format!("{USER_PRESET_PREFIX}{}", entry.file_name().to_str()?);
            let folder = folder_of(dir, &id)?;
            Some(match read_manifest(&folder)? {
                Manifest::Layout { name, layout } => UserPreset {
                    id,
                    name: short_name(&name),
                    layout: Some(layout.fitted()),
                },
                Manifest::Imported { name } => UserPreset {
                    id,
                    name: short_name(&name),
                    layout: None,
                },
            })
        })
        .collect();
    presets.sort_by(|a, b| a.id.cmp(&b.id));
    presets
}

pub fn save_layout(
    dir: &Path,
    id: Option<&str>,
    name: &str,
    layout: &KeyLayout,
) -> io::Result<String> {
    let manifest = serde_json::to_vec_pretty(&Manifest::Layout {
        name: short_name(name),
        layout: layout.clone().fitted(),
    })?;
    if let Some(id) = id
        && let Some(folder) = folder_of(dir, id)
        && matches!(read_manifest(&folder), Some(Manifest::Layout { .. }))
    {
        replace_manifest(&folder, manifest)?;
        return Ok(id.to_string());
    }
    publish(dir, |folder| {
        fs::write(folder.join(MANIFEST_FILE), manifest)
    })
}

fn replace_manifest(folder: &Path, manifest: Vec<u8>) -> io::Result<()> {
    let staged = folder.join(format!("{MANIFEST_FILE}.tmp"));
    fs::write(&staged, manifest)?;
    fs::rename(&staged, folder.join(MANIFEST_FILE))?;
    GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

pub fn rename(dir: &Path, id: &str, name: &str) -> io::Result<bool> {
    let folder = folder_of(dir, id).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "not a preset the user added")
    })?;
    let mut manifest = read_manifest(&folder).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the preset is not in the app data folder",
        )
    })?;
    let (Manifest::Layout { name: stored, .. } | Manifest::Imported { name: stored }) =
        &mut manifest;
    let name = short_name(name);
    if name.is_empty() || name == *stored {
        return Ok(false);
    }
    *stored = name;
    replace_manifest(&folder, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(true)
}

pub fn remove(dir: &Path, id: &str) -> io::Result<()> {
    let folder = folder_of(dir, id).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "not a preset the user added")
    })?;
    if read_manifest(&folder).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the preset is not in the app data folder",
        ));
    }
    fs::remove_dir_all(folder)?;
    GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

fn with_extension(folder: &Path, extension: &str) -> io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|found| found.to_str())
                    .is_some_and(|found| found.eq_ignore_ascii_case(extension))
        })
        .collect();
    files.sort();
    Ok(files)
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

fn atlas_for<'a>(preset: &Path, images: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let name = stem(preset).to_lowercase();
    images
        .iter()
        .filter(|image| name.starts_with(&stem(image).to_lowercase()))
        .max_by_key(|image| stem(image).len())
        .or(match images {
            [only] => Some(only),
            _ => None,
        })
}

fn checked(preset: &Path, atlas: Option<&Path>) -> Result<(), PresetRefusal> {
    let size = |path: &Path| fs::metadata(path).map_or(0, |found| found.len() as usize);
    let unreadable = |error: io::Error| PresetRefusal::JsonUnreadable {
        detail: error.to_string(),
    };
    let bytes = size(preset);
    if bytes > MAX_PRESET_JSON_BYTES {
        return Err(PresetRefusal::JsonTooLarge { bytes });
    }
    let json = fs::read(preset).map_err(unreadable)?;
    let Some(atlas) = atlas else {
        return read_preset(&json, None).map(|_| ());
    };
    let bytes = size(atlas);
    if bytes > MAX_PRESET_ATLAS_BYTES {
        return Err(PresetRefusal::AtlasFileTooLarge { bytes });
    }
    let image = fs::read(atlas).map_err(|error| PresetRefusal::AtlasUnreadable {
        detail: error.to_string(),
    })?;
    read_preset(&json, Some(&image)).map(|_| ())
}

pub fn import_folder(dir: &Path, source: &Path) -> Result<Vec<String>, ImportRefusal> {
    let unreadable = |error: io::Error| ImportRefusal::FolderUnreadable {
        detail: error.to_string(),
    };
    let presets = with_extension(source, "json").map_err(unreadable)?;
    let images = with_extension(source, "png").map_err(unreadable)?;
    if presets.is_empty() {
        return Err(ImportRefusal::NoPresetFile);
    }
    if presets.len() > MAX_IMPORT_PRESETS {
        return Err(ImportRefusal::TooManyPresetFiles {
            count: presets.len(),
        });
    }
    let mut pairs = Vec::with_capacity(presets.len());
    for preset in &presets {
        let atlas = atlas_for(preset, &images);
        checked(preset, atlas.map(PathBuf::as_path)).map_err(|refusal| ImportRefusal::File {
            file: preset
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            refusal,
        })?;
        if let Some(atlas) = atlas {
            pairs.push((preset, atlas));
        }
    }
    let mut imported = Vec::with_capacity(pairs.len());
    for (preset, atlas) in pairs {
        let saved = serde_json::to_vec_pretty(&Manifest::Imported {
            name: short_name(&stem(preset)),
        })
        .map_err(io::Error::from)
        .and_then(|manifest| {
            publish(dir, |folder| {
                fs::copy(preset, folder.join(PRESET_FILE))?;
                fs::copy(atlas, folder.join(ATLAS_FILE))?;
                fs::write(folder.join(MANIFEST_FILE), manifest)
            })
        });
        match saved {
            Ok(id) => imported.push(id),
            Err(error) => {
                for id in &imported {
                    remove(dir, id).ok();
                }
                return Err(ImportRefusal::NotSaved {
                    detail: error.to_string(),
                });
            }
        }
    }
    Ok(imported)
}

pub fn load(dir: &Path, id: &str, style: InputDisplayStyle, scale: u32) -> Option<LoadedPreset> {
    let (id, controller) = drawn_for(id);
    let folder = folder_of(dir, id)?;
    match read_manifest(&folder)? {
        Manifest::Layout { layout, .. } => {
            layout_preset_for(&layout.fitted(), style, controller, scale).ok()
        }
        Manifest::Imported { .. } => {
            let json = read_limited(&folder.join(PRESET_FILE), MAX_PRESET_JSON_BYTES).ok()?;
            let atlas = read_limited(&folder.join(ATLAS_FILE), MAX_PRESET_ATLAS_BYTES).ok()?;
            read_preset(&json, Some(&atlas)).ok()
        }
    }
}

pub fn shape(dir: &Path, id: &str) -> Option<PresetShape> {
    let (id, _) = drawn_for(id);
    let folder = folder_of(dir, id)?;
    match read_manifest(&folder)? {
        Manifest::Layout { layout, .. } => {
            let layout = layout.fitted();
            Some(PresetShape {
                size: layout.size(),
                frame_share: layout.frame_share(),
            })
        }
        Manifest::Imported { .. } => {
            let loaded = load(dir, id, InputDisplayStyle::default(), 1)?;
            Some(PresetShape {
                size: (loaded.preset.width, loaded.preset.height),
                frame_share: loaded.frame_share,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_display::compose;
    use frameever_project::input_display::{
        BuiltinPresetId, DisplayState, Input, MAX_PRESET_ATLAS_DIMENSION, MAX_PRESET_ELEMENTS,
        key_codes::letter,
    };
    use image::RgbaImage;

    const PRESET: &str = r#"{
        "default_height": 0, "default_width": 0, "flags": 0, "space_h": 0, "space_v": 0,
        "overlay_width": 20, "overlay_height": 12, "version": 508,
        "elements": [
            { "type": 0, "id": "plate", "pos": [0, 0], "mapping": [1, 1, 20, 12], "z_level": 0 },
            { "type": 1, "id": "w", "pos": [2, 2], "mapping": [24, 1, 6, 5], "z_level": 1, "code": 70 }
        ]
    }"#;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::from_fn(width, height, |x, y| match (x, y) {
            (1..=20, 1..=12) => image::Rgba([0, 0, 255, 255]),
            (24..=29, 1..=5) => image::Rgba([0, 255, 0, 255]),
            (24..=29, 9..=13) => image::Rgba([255, 0, 0, 255]),
            _ => image::Rgba([0, 0, 0, 0]),
        })
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    struct Folders {
        _root: tempfile::TempDir,
        presets: PathBuf,
        source: PathBuf,
    }

    fn folders(files: &[(&str, &[u8])]) -> Folders {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("downloaded preset");
        fs::create_dir_all(&source).unwrap();
        for (name, bytes) in files {
            fs::write(source.join(name), bytes).unwrap();
        }
        Folders {
            presets: root.path().join("input-presets"),
            source,
            _root: root,
        }
    }

    fn refused(files: &[(&str, &[u8])]) -> ImportRefusal {
        let folders = folders(files);
        let refusal = import_folder(&folders.presets, &folders.source).unwrap_err();
        assert_eq!(list(&folders.presets), Vec::new());
        assert!(
            !folders.presets.exists() || fs::read_dir(&folders.presets).unwrap().next().is_none()
        );
        refusal
    }

    fn holding_w() -> DisplayState {
        let mut state = DisplayState::default();
        state.apply(Input::Key(letter('W')), true);
        state
    }

    #[test]
    fn a_preset_folder_is_copied_listed_and_drawn() {
        let folders = folders(&[
            ("wasd.json", PRESET.as_bytes()),
            ("wasd.png", &png(32, 16)),
            ("readme.txt", b"hello"),
        ]);

        let imported = import_folder(&folders.presets, &folders.source).unwrap();

        let listed = list(&folders.presets);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, imported[0]);
        assert_eq!(
            (listed[0].name.as_str(), &listed[0].layout),
            ("wasd", &None)
        );
        fs::remove_dir_all(&folders.source).unwrap();
        let loaded = load(&folders.presets, &imported[0], InputDisplayStyle::Ghost, 3).unwrap();
        assert_eq!(
            compose(&loaded, &holding_w()).get_pixel(4, 4).0,
            [255, 0, 0, 255]
        );
        assert_eq!(
            compose(&loaded, &DisplayState::default()).get_pixel(4, 4).0,
            [0, 255, 0, 255]
        );
        assert_eq!(
            shape(&folders.presets, &imported[0]),
            Some(PresetShape {
                size: (20, 12),
                frame_share: loaded.frame_share
            })
        );
    }

    #[test]
    fn presets_that_share_an_image_are_each_imported_with_it() {
        let folders = folders(&[
            ("wasd-minimal.json", PRESET.as_bytes()),
            ("wasd-full.json", PRESET.as_bytes()),
            ("wasd.png", &png(32, 16)),
            ("wasd-full-arrows.png", &png(8, 8)),
            ("other.png", &png(8, 8)),
        ]);

        let imported = import_folder(&folders.presets, &folders.source).unwrap();

        let mut names: Vec<String> = list(&folders.presets)
            .into_iter()
            .map(|preset| preset.name)
            .collect();
        names.sort();
        assert_eq!(names, ["wasd-full", "wasd-minimal"]);
        for id in imported {
            assert!(load(&folders.presets, &id, InputDisplayStyle::Keycaps, 1).is_some());
        }
    }

    #[test]
    fn a_broken_preset_is_refused_and_nothing_is_added() {
        assert_eq!(
            refused(&[("readme.txt", b"hello"), ("wasd.png", &png(32, 16))]),
            ImportRefusal::NoPresetFile
        );
        assert!(matches!(
            refused(&[
                ("wasd.json", &PRESET.as_bytes()[..PRESET.len() / 2]),
                ("wasd.png", &png(32, 16))
            ]),
            ImportRefusal::File {
                refusal: PresetRefusal::JsonUnreadable { .. },
                ..
            }
        ));
        assert_eq!(
            refused(&[("wasd.json", PRESET.as_bytes())]),
            ImportRefusal::File {
                file: "wasd.json".to_string(),
                refusal: PresetRefusal::MissingAtlas
            }
        );
        assert!(matches!(
            refused(&[("wasd.json", PRESET.as_bytes()), ("wasd.png", b"not a png")]),
            ImportRefusal::File {
                refusal: PresetRefusal::AtlasUnreadable { .. },
                ..
            }
        ));
        assert!(matches!(
            refused(&[("wasd.json", PRESET.as_bytes()), ("wasd.png", &png(8, 8))]),
            ImportRefusal::File {
                refusal: PresetRefusal::MappingOutsideAtlas { .. },
                ..
            }
        ));
        let unknown = PRESET.replace(r#""type": 0"#, r#""type": 42"#);
        assert!(matches!(
            refused(&[
                ("wasd.json", unknown.as_bytes()),
                ("wasd.png", &png(32, 16))
            ]),
            ImportRefusal::File {
                refusal: PresetRefusal::UnknownElementType { type_id: 42, .. },
                ..
            }
        ));
    }

    #[test]
    fn one_broken_file_keeps_the_whole_folder_out() {
        let refusal = refused(&[
            ("a-good.json", PRESET.as_bytes()),
            ("b-bad.json", b"{"),
            ("a.png", &png(32, 16)),
        ]);

        assert!(matches!(
            refusal,
            ImportRefusal::File { ref file, .. } if file == "b-bad.json"
        ));
    }

    #[test]
    fn oversized_presets_are_refused_and_nothing_is_added() {
        let padded = format!("{PRESET}{}", " ".repeat(MAX_PRESET_JSON_BYTES));
        assert_eq!(
            refused(&[("wasd.json", padded.as_bytes()), ("wasd.png", &png(32, 16))]),
            ImportRefusal::File {
                file: "wasd.json".to_string(),
                refusal: PresetRefusal::JsonTooLarge {
                    bytes: padded.len()
                }
            }
        );
        assert_eq!(
            refused(&[
                ("wasd.json", PRESET.as_bytes()),
                ("wasd.png", &vec![0; MAX_PRESET_ATLAS_BYTES + 1])
            ]),
            ImportRefusal::File {
                file: "wasd.json".to_string(),
                refusal: PresetRefusal::AtlasFileTooLarge {
                    bytes: MAX_PRESET_ATLAS_BYTES + 1
                }
            }
        );
        assert_eq!(
            refused(&[
                ("wasd.json", PRESET.as_bytes()),
                ("wasd.png", &png(MAX_PRESET_ATLAS_DIMENSION + 1, 16))
            ]),
            ImportRefusal::File {
                file: "wasd.json".to_string(),
                refusal: PresetRefusal::AtlasTooLarge {
                    width: MAX_PRESET_ATLAS_DIMENSION + 1,
                    height: 16
                }
            }
        );
        let element = r#"{ "type": 0, "pos": [0, 0], "mapping": [1, 1, 2, 2] }"#;
        let crowded = format!(
            r#"{{ "elements": [{}] }}"#,
            vec![element; MAX_PRESET_ELEMENTS + 1].join(",")
        );
        assert_eq!(
            refused(&[
                ("wasd.json", crowded.as_bytes()),
                ("wasd.png", &png(32, 16))
            ]),
            ImportRefusal::File {
                file: "wasd.json".to_string(),
                refusal: PresetRefusal::TooManyElements {
                    count: MAX_PRESET_ELEMENTS + 1
                }
            }
        );
        let many: Vec<(String, &[u8])> = (0..=MAX_IMPORT_PRESETS)
            .map(|index| (format!("p{index}.json"), PRESET.as_bytes()))
            .collect();
        let many: Vec<(&str, &[u8])> = many
            .iter()
            .map(|(name, bytes)| (name.as_str(), *bytes))
            .collect();
        assert_eq!(
            refused(&many),
            ImportRefusal::TooManyPresetFiles {
                count: MAX_IMPORT_PRESETS + 1
            }
        );
    }

    #[test]
    fn a_made_preset_is_drawn_at_its_own_size_and_opacity() {
        let folders = folders(&[]);
        let plain = BuiltinPresetId::WasdMouse.layout();
        let made = KeyLayout {
            size: Some(150.0),
            opacity: Some(40.0),
            ..plain.clone()
        };
        let plain_id = save_layout(&folders.presets, None, "Plain", &plain).unwrap();
        let made_id = save_layout(&folders.presets, None, "Made", &made).unwrap();
        let style = InputDisplayStyle::Keycaps;
        let plain_loaded = load(&folders.presets, &plain_id, style, 1).unwrap();
        let made_loaded = load(&folders.presets, &made_id, style, 1).unwrap();

        assert_eq!(plain_loaded.opacity, 1.0);
        assert_eq!(plain_loaded.opacity_under(0.5), 0.5);
        assert_eq!(made_loaded.opacity, 0.4);
        assert!((made_loaded.opacity_under(0.5) - 0.2).abs() < 1e-9);
        assert!((made_loaded.frame_share - plain_loaded.frame_share * 1.5).abs() < 1e-9);
        assert_eq!(
            compose(&made_loaded, &holding_w()),
            compose(&plain_loaded, &holding_w())
        );
        assert_eq!(
            shape(&folders.presets, &made_id).map(|shape| shape.frame_share),
            Some(made.frame_share())
        );
    }

    #[test]
    fn a_made_preset_is_saved_edited_in_place_and_removed() {
        let folders = folders(&[]);
        let mut layout = BuiltinPresetId::WasdMouse.layout();
        let before = generation();

        let id = save_layout(&folders.presets, None, "  Mine  ", &layout).unwrap();

        assert!(generation() > before);
        assert_eq!(
            list(&folders.presets),
            vec![UserPreset {
                id: id.clone(),
                name: "Mine".to_string(),
                layout: Some(layout.clone())
            }]
        );
        for style in InputDisplayStyle::ALL {
            let loaded = load(&folders.presets, &id, style, 1).unwrap();
            assert!(loaded.scalable, "{style:?}");
            assert_ne!(
                compose(&loaded, &holding_w()),
                compose(&loaded, &DisplayState::default()),
                "{style:?}"
            );
        }
        assert_eq!(
            shape(&folders.presets, &id),
            Some(PresetShape {
                size: layout.size(),
                frame_share: layout.frame_share()
            })
        );

        layout.mouse = None;
        layout.keys.truncate(3);
        let edited = save_layout(&folders.presets, Some(&id), "Mine too", &layout).unwrap();
        assert_eq!(edited, id);
        let listed = list(&folders.presets);
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (listed[0].name.as_str(), listed[0].layout.as_ref()),
            ("Mine too", Some(&layout))
        );

        remove(&folders.presets, &id).unwrap();
        assert_eq!(list(&folders.presets), Vec::new());
        assert!(load(&folders.presets, &id, InputDisplayStyle::Keycaps, 1).is_none());
        assert!(remove(&folders.presets, &id).is_err());
    }

    #[test]
    fn saving_under_an_imported_preset_s_id_makes_a_new_one() {
        let folders = folders(&[("wasd.json", PRESET.as_bytes()), ("wasd.png", &png(32, 16))]);
        let imported = import_folder(&folders.presets, &folders.source).unwrap();

        let made = save_layout(
            &folders.presets,
            Some(&imported[0]),
            "Mine",
            &BuiltinPresetId::WasdMouse.layout(),
        )
        .unwrap();

        assert_ne!(made, imported[0]);
        assert_eq!(list(&folders.presets).len(), 2);
    }

    #[test]
    fn renaming_a_preset_keeps_its_id_and_files_and_writes_only_a_new_name() {
        let folders = folders(&[("wasd.json", PRESET.as_bytes()), ("wasd.png", &png(32, 16))]);
        let id = import_folder(&folders.presets, &folders.source)
            .unwrap()
            .remove(0);
        let folder = folder_of(&folders.presets, &id).unwrap();
        let files = || {
            let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(&folder)
                .unwrap()
                .flatten()
                .map(|entry| {
                    (
                        entry.file_name().to_string_lossy().into_owned(),
                        fs::read(entry.path()).unwrap(),
                    )
                })
                .collect();
            files.sort();
            files
        };
        let drawn = || {
            load(&folders.presets, &id, InputDisplayStyle::Keycaps, 1)
                .unwrap()
                .atlas
        };
        let (before, picture, generation_before) = (files(), drawn(), generation());

        for refused in ["", "   ", "wasd", "  wasd  "] {
            assert!(!rename(&folders.presets, &id, refused).unwrap());
        }
        assert!(files() == before);

        assert!(rename(&folders.presets, &id, "  Fight stick  ").unwrap());

        assert!(generation() > generation_before);
        assert_eq!(
            list(&folders.presets),
            vec![UserPreset {
                id: id.clone(),
                name: "Fight stick".to_string(),
                layout: None
            }]
        );
        let after = files();
        assert_eq!(after.len(), before.len());
        for ((name, bytes), (name_before, bytes_before)) in after.iter().zip(&before) {
            assert_eq!(name, name_before);
            assert_eq!(bytes == bytes_before, name != MANIFEST_FILE, "{name}");
        }
        assert!(drawn() == picture);
        let long = "x".repeat(MAKER_NAME_LENGTH + 5);
        assert!(rename(&folders.presets, &id, &long).unwrap());
        assert_eq!(list(&folders.presets)[0].name.len(), MAKER_NAME_LENGTH);

        let layout = BuiltinPresetId::WasdMouse.layout();
        let made = save_layout(&folders.presets, None, "Mine", &layout).unwrap();
        assert!(rename(&folders.presets, &made, "Mine too").unwrap());
        assert_eq!(
            list(&folders.presets)
                .into_iter()
                .find(|preset| preset.id == made),
            Some(UserPreset {
                id: made,
                name: "Mine too".to_string(),
                layout: Some(layout)
            })
        );
        assert!(rename(&folders.presets, "user:p-missing", "Gone").is_err());
        assert!(rename(&folders.presets, "wasd-mouse", "Built in").is_err());
    }

    #[test]
    fn a_saved_preset_is_drawn_by_its_id_once_the_folder_is_known() {
        let folders = folders(&[]);
        let layout = BuiltinPresetId::WasdMouse.layout();
        let id = save_layout(&folders.presets, None, "Mine", &layout).unwrap();

        use_presets_dir(folders.presets.clone());

        let drawn = crate::input_display::resolve_preset(&id, InputDisplayStyle::Ghost, 2).unwrap();
        assert_eq!(drawn.atlas_scale, 2);
        assert_eq!(
            crate::input_display::preset_shape(&id),
            Some(PresetShape {
                size: layout.size(),
                frame_share: layout.frame_share()
            })
        );
        assert!(
            crate::input_display::resolve_preset("user:p-missing", InputDisplayStyle::Ghost, 1)
                .is_none()
        );
    }

    #[test]
    fn an_id_that_is_not_a_plain_folder_name_reaches_nothing() {
        let folders = folders(&[("wasd.json", PRESET.as_bytes()), ("wasd.png", &png(32, 16))]);
        import_folder(&folders.presets, &folders.source).unwrap();

        for id in [
            "user:../downloaded preset",
            "user:..",
            "user:",
            "user:a/b",
            "user:C:\\x",
            "wasd-mouse",
        ] {
            assert!(load(&folders.presets, id, InputDisplayStyle::Keycaps, 1).is_none());
            assert!(shape(&folders.presets, id).is_none());
            assert!(remove(&folders.presets, id).is_err());
        }
        assert!(folders.source.exists());
    }

    #[test]
    fn a_made_gamepad_preset_is_drawn_for_the_controller_in_use() {
        use crate::input_display::{builtin_preset, preset_for_controller};
        use frameever_project::input_display::{GamepadKind, GamepadParts};
        let folders = folders(&[]);
        let whole = KeyLayout {
            gamepad: Some(GamepadParts::default()),
            ..Default::default()
        };
        let id = save_layout(&folders.presets, None, "Pad", &whole).unwrap();
        let keys = save_layout(
            &folders.presets,
            None,
            "Keys",
            &BuiltinPresetId::WasdMouse.layout(),
        )
        .unwrap();
        let style = InputDisplayStyle::Keycaps;
        let drawn = |id: &str| load(&folders.presets, id, style, 1).unwrap().atlas;
        let built_in = |preset| builtin_preset(preset, style, 1).unwrap().atlas;

        assert!(draws_a_gamepad(&folders.presets, &id));
        assert!(!draws_a_gamepad(&folders.presets, &keys));
        assert!(!draws_a_gamepad(&folders.presets, "user:p-missing"));
        assert!(drawn(&id) == built_in(BuiltinPresetId::Gamepad));

        let playstation = preset_for_controller(&id, Some(GamepadKind::PlayStation));
        assert_eq!(playstation, format!("{id}@playstation"));
        assert!(drawn(&playstation) == built_in(BuiltinPresetId::GamepadPlayStation));
        assert!(draws_a_gamepad(&folders.presets, &playstation));
        assert_eq!(
            preset_for_controller(&playstation, Some(GamepadKind::Switch)),
            format!("{id}@switch")
        );
        assert_eq!(preset_for_controller(&id, None), id);
        assert_eq!(
            preset_for_controller("gamepad", Some(GamepadKind::Switch)),
            "gamepad-switch"
        );
        assert_eq!(
            shape(&folders.presets, &playstation),
            shape(&folders.presets, &id)
        );
        assert_eq!(
            shape(&folders.presets, &id).map(|shape| shape.size),
            Some(BuiltinPresetId::Gamepad.size())
        );
        assert!(drawn(&preset_for_controller(&keys, Some(GamepadKind::Switch))) == drawn(&keys));

        let fewer = KeyLayout {
            gamepad: Some(GamepadParts {
                triggers: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        save_layout(&folders.presets, Some(&id), "Pad", &fewer).unwrap();
        let listed = list(&folders.presets);
        assert_eq!(
            listed
                .iter()
                .find(|preset| preset.id == id)
                .and_then(|preset| preset.layout.clone()),
            Some(fewer)
        );
        assert!(drawn(&id) != built_in(BuiltinPresetId::Gamepad));
    }
}
