use frameever_project::{
    InputDisplayStyle,
    input_display::{
        BuiltinPresetId, DisplayState, INPUT_DISPLAY_IMPORTED_FRAME_SHARE, KeyLayout,
        MAX_PRESET_ATLAS_BYTES, MAX_PRESET_ATLAS_DIMENSION, Preset, PresetArt, PresetRefusal,
    },
};
use image::RgbaImage;

pub const MAX_ATLAS_SCALE: u32 = 4;

#[derive(Clone, Debug)]
pub struct LoadedPreset {
    pub preset: Preset,
    pub frame_share: f64,
    pub atlas_scale: u32,
    pub scalable: bool,
    pub atlas: RgbaImage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetShape {
    pub size: (u32, u32),
    pub frame_share: f64,
}

pub fn preset_shape(id: &str) -> Option<PresetShape> {
    let Some(builtin) = BuiltinPresetId::from_id(id) else {
        return crate::input_presets::shape(&crate::input_presets::presets_dir()?, id);
    };
    Some(PresetShape {
        size: builtin.size(),
        frame_share: builtin.frame_share(),
    })
}

pub fn atlas_scale_for(preset_width: u32, drawn_width: f64) -> u32 {
    let scale = (drawn_width / f64::from(preset_width.max(1))).ceil();
    if scale.is_finite() {
        (scale as u32).clamp(1, MAX_ATLAS_SCALE)
    } else {
        1
    }
}

pub fn builtin_preset(
    id: BuiltinPresetId,
    style: InputDisplayStyle,
    scale: u32,
) -> Result<LoadedPreset, PresetRefusal> {
    drawn_preset(&id.art(style), id.frame_share(), scale)
}

pub fn layout_preset(
    layout: &KeyLayout,
    style: InputDisplayStyle,
    frame_share: f64,
    scale: u32,
) -> Result<LoadedPreset, PresetRefusal> {
    drawn_preset(&layout.art(style), frame_share, scale)
}

pub fn resolve_preset(id: &str, style: InputDisplayStyle, scale: u32) -> Option<LoadedPreset> {
    match BuiltinPresetId::from_id(id) {
        Some(builtin) => builtin_preset(builtin, style, scale).ok(),
        None => crate::input_presets::load(&crate::input_presets::presets_dir()?, id, style, scale),
    }
}

fn drawn_preset(
    art: &PresetArt,
    frame_share: f64,
    scale: u32,
) -> Result<LoadedPreset, PresetRefusal> {
    let scale = scale.clamp(1, MAX_ATLAS_SCALE);
    let preset = Preset::parse_own(&art.json, (art.atlas_width, art.atlas_height))?;
    let unreadable = |detail: String| PresetRefusal::AtlasUnreadable { detail };
    let options = resvg::usvg::Options {
        fontdb: crate::frame_chrome::chrome_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&art.atlas_svg, &options)
        .map_err(|error| unreadable(error.to_string()))?;
    let (width, height) = (art.atlas_width * scale, art.atlas_height * scale);
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| unreadable("the image has no area".to_string()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut pixmap.as_mut(),
    );
    let atlas = RgbaImage::from_raw(width, height, pixmap.take())
        .ok_or_else(|| unreadable("the image has no pixels".to_string()))?;
    Ok(LoadedPreset {
        preset,
        frame_share,
        atlas_scale: scale,
        scalable: true,
        atlas,
    })
}

pub fn read_preset(json: &[u8], atlas_png: Option<&[u8]>) -> Result<LoadedPreset, PresetRefusal> {
    let json = std::str::from_utf8(json).map_err(|error| PresetRefusal::JsonUnreadable {
        detail: error.to_string(),
    })?;
    let png = atlas_png.ok_or(PresetRefusal::MissingAtlas)?;
    if png.len() > MAX_PRESET_ATLAS_BYTES {
        return Err(PresetRefusal::AtlasFileTooLarge { bytes: png.len() });
    }
    let unreadable = |error: image::ImageError| PresetRefusal::AtlasUnreadable {
        detail: error.to_string(),
    };
    let reader =
        || image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let (width, height) = reader().into_dimensions().map_err(unreadable)?;
    if width > MAX_PRESET_ATLAS_DIMENSION || height > MAX_PRESET_ATLAS_DIMENSION {
        return Err(PresetRefusal::AtlasTooLarge { width, height });
    }
    let preset = Preset::parse(json, (width, height))?;
    let mut atlas = reader().decode().map_err(unreadable)?.into_rgba8();
    for pixel in atlas.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(LoadedPreset {
        preset,
        frame_share: INPUT_DISPLAY_IMPORTED_FRAME_SHARE,
        atlas_scale: 1,
        scalable: false,
        atlas,
    })
}

pub fn compose(loaded: &LoadedPreset, state: &DisplayState) -> RgbaImage {
    let scale = loaded.atlas_scale.max(1);
    let mut output = RgbaImage::new(
        (loaded.preset.width * scale).max(1),
        (loaded.preset.height * scale).max(1),
    );
    for sprite in loaded.preset.sprites(state) {
        for row in 0..sprite.source.height * scale {
            for column in 0..sprite.source.width * scale {
                let (source_x, source_y) = (
                    sprite.source.u * scale + column,
                    sprite.source.v * scale + row,
                );
                let (x, y) = (sprite.x * scale + column, sprite.y * scale + row);
                if source_x >= loaded.atlas.width()
                    || source_y >= loaded.atlas.height()
                    || x >= output.width()
                    || y >= output.height()
                {
                    continue;
                }
                let source = loaded.atlas.get_pixel(source_x, source_y).0;
                let keep = 255 - u32::from(source[3]);
                let target = output.get_pixel_mut(x, y);
                for (target, source) in target.0.iter_mut().zip(source) {
                    *target = (u32::from(source) + (u32::from(*target) * keep + 127) / 255).min(255)
                        as u8;
                }
            }
        }
    }
    output
}

pub fn straighten_alpha(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 {
            pixel.0 = [0, 0, 0, 0];
            continue;
        }
        for channel in &mut pixel.0[..3] {
            *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_project::input_display::preset_for_controller;
    use frameever_project::input_display::{
        GamepadAxes, GamepadKind, GamepadSnapshot, Input, gamepad_buttons, key_code,
    };

    const FIXTURE: &str = r#"{
        "overlay_width": 20,
        "overlay_height": 12,
        "elements": [
            { "type": 0, "id": "plate", "pos": [0, 0], "mapping": [1, 1, 20, 12], "z_level": 0 },
            { "type": 1, "id": "w", "pos": [2, 2], "mapping": [24, 1, 6, 5], "z_level": 1, "code": 87 }
        ]
    }"#;

    fn png(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::from_fn(width, height, |x, y| image::Rgba(paint(x, y)))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn fixture_atlas() -> Vec<u8> {
        png(32, 16, |x, y| match (x, y) {
            (1..=20, 1..=12) => [0, 0, 255, 255],
            (24..=29, 1..=5) => [0, 255, 0, 255],
            (24..=29, 9..=13) => [255, 0, 0, 128],
            _ => [0, 0, 0, 0],
        })
    }

    fn pressed(recorded: &str) -> DisplayState {
        let mut state = DisplayState::default();
        state.apply(Input::Key(key_code(recorded).unwrap()), true);
        state
    }

    #[test]
    fn a_preset_made_of_a_json_file_and_a_png_draws_its_pressed_key() {
        let loaded = read_preset(FIXTURE.as_bytes(), Some(&fixture_atlas())).unwrap();

        let released = compose(&loaded, &DisplayState::default());
        let held = compose(&loaded, &pressed("W"));

        assert_eq!((released.width(), released.height()), (20, 12));
        assert_eq!(released.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(released.get_pixel(4, 4).0, [0, 255, 0, 255]);
        assert_eq!(held.get_pixel(4, 4).0, [128, 0, 127, 255]);
        assert_eq!(held.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(compose(&loaded, &pressed("A")), released);
    }

    #[test]
    fn a_preset_without_a_readable_image_is_refused_with_the_reason() {
        assert_eq!(
            read_preset(FIXTURE.as_bytes(), None).unwrap_err(),
            PresetRefusal::MissingAtlas
        );
        assert!(matches!(
            read_preset(FIXTURE.as_bytes(), Some(b"not a png")).unwrap_err(),
            PresetRefusal::AtlasUnreadable { .. }
        ));
        let cut = fixture_atlas();
        assert!(matches!(
            read_preset(FIXTURE.as_bytes(), Some(&cut[..cut.len() / 2])).unwrap_err(),
            PresetRefusal::AtlasUnreadable { .. }
        ));
    }

    #[test]
    fn oversized_images_are_refused() {
        let wide = png(MAX_PRESET_ATLAS_DIMENSION + 1, 1, |_, _| [0, 0, 0, 0]);

        assert_eq!(
            read_preset(FIXTURE.as_bytes(), Some(&wide)).unwrap_err(),
            PresetRefusal::AtlasTooLarge {
                width: MAX_PRESET_ATLAS_DIMENSION + 1,
                height: 1
            }
        );
        assert_eq!(
            read_preset(
                FIXTURE.as_bytes(),
                Some(&vec![0; MAX_PRESET_ATLAS_BYTES + 1])
            )
            .unwrap_err(),
            PresetRefusal::AtlasFileTooLarge {
                bytes: MAX_PRESET_ATLAS_BYTES + 1
            }
        );
    }

    #[test]
    fn a_preset_whose_json_does_not_fit_its_image_is_refused() {
        let small = png(8, 8, |_, _| [0, 0, 0, 0]);

        assert!(matches!(
            read_preset(FIXTURE.as_bytes(), Some(&small)).unwrap_err(),
            PresetRefusal::MappingOutsideAtlas { .. }
        ));
        assert!(matches!(
            read_preset(&FIXTURE.as_bytes()[..40], Some(&fixture_atlas())).unwrap_err(),
            PresetRefusal::JsonUnreadable { .. }
        ));
        assert!(matches!(
            read_preset(&[0xff, 0xfe], Some(&fixture_atlas())).unwrap_err(),
            PresetRefusal::JsonUnreadable { .. }
        ));
    }

    #[test]
    fn every_built_in_preset_draws_in_every_style_and_lights_a_pressed_key() {
        for id in BuiltinPresetId::ALL {
            for style in InputDisplayStyle::ALL {
                let loaded = builtin_preset(id, style, 1).unwrap();
                let released = compose(&loaded, &DisplayState::default());
                let held = compose(
                    &loaded,
                    &match id.controller() {
                        Some(_) => gamepad(
                            GamepadAxes {
                                left_trigger: 255,
                                ..Default::default()
                            },
                            &[gamepad_buttons::SOUTH],
                        ),
                        None => pressed("W"),
                    },
                );
                let lit = |image: &RgbaImage| {
                    image
                        .pixels()
                        .filter(|pixel| pixel.0 == [255, 255, 255, 255])
                        .count()
                };

                assert_eq!(
                    (released.width(), released.height()),
                    (loaded.preset.width, loaded.preset.height)
                );
                assert!(
                    released.pixels().any(|pixel| pixel[3] > 0),
                    "{id:?} {style:?}"
                );
                assert!(lit(&held) > lit(&released) + 500, "{id:?} {style:?}");
            }
        }
    }

    fn gamepad(axes: GamepadAxes, buttons: &[u16]) -> DisplayState {
        let mut state = DisplayState::default();
        state.show_gamepad(&buttons.iter().fold(
            GamepadSnapshot {
                axes,
                ..Default::default()
            },
            |snapshot, button| snapshot.with_button(*button),
        ));
        state
    }

    fn white(image: &RgbaImage, left: u32, top: u32, right: u32, bottom: u32) -> usize {
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| (x, y)))
            .filter(|(x, y)| image.get_pixel(*x, *y).0 == [255, 255, 255, 255])
            .count()
    }

    #[test]
    fn a_trigger_fills_from_the_left_by_how_far_it_is_pulled() {
        let loaded =
            builtin_preset(BuiltinPresetId::Gamepad, InputDisplayStyle::Keycaps, 1).unwrap();
        let pulled = |right_trigger| {
            compose(
                &loaded,
                &gamepad(
                    GamepadAxes {
                        right_trigger,
                        ..Default::default()
                    },
                    &[],
                ),
            )
        };
        let (left_half, right_half) = ((285, 5, 307, 25), (309, 5, 331, 25));
        let lit = |image: &RgbaImage, (left, top, right, bottom): (u32, u32, u32, u32)| {
            white(image, left, top, right, bottom)
        };

        assert_eq!(lit(&pulled(0), left_half), 0);
        assert!(lit(&pulled(128), left_half) > 250);
        assert_eq!(lit(&pulled(128), right_half), 0);
        assert!(lit(&pulled(255), right_half) > 250);
        assert_eq!(white(&pulled(255), 0, 0, 200, 30), 0);
    }

    #[test]
    fn a_pushed_stick_is_drawn_away_from_its_centre() {
        let loaded =
            builtin_preset(BuiltinPresetId::Gamepad, InputDisplayStyle::Keycaps, 1).unwrap();
        let knob = [24, 24, 24, 255];
        let knob_columns = |image: &RgbaImage| {
            let columns: Vec<u32> = (90..152)
                .filter(|x| image.get_pixel(*x, 109).0 == knob)
                .collect();
            (*columns.first().unwrap(), *columns.last().unwrap())
        };
        let pushed = |left_stick| {
            compose(
                &loaded,
                &gamepad(
                    GamepadAxes {
                        left_stick,
                        ..Default::default()
                    },
                    &[],
                ),
            )
        };

        let (rest_left, rest_right) = knob_columns(&pushed([0, 0]));
        assert_eq!(
            knob_columns(&pushed([127, 0])),
            (rest_left + 9, rest_right + 9)
        );
        assert_eq!(
            knob_columns(&pushed([-127, 0])),
            (rest_left - 9, rest_right - 9)
        );
        assert_eq!(pushed([0, 0]), compose(&loaded, &DisplayState::default()));
    }

    #[test]
    fn each_controller_draws_its_own_picture() {
        let picture = |id: &str| {
            compose(
                &resolve_preset(id, InputDisplayStyle::Outline, 1).unwrap(),
                &DisplayState::default(),
            )
        };
        let xbox = picture(preset_for_controller("gamepad", Some(GamepadKind::Xbox)));
        let playstation = picture(preset_for_controller(
            "gamepad",
            Some(GamepadKind::PlayStation),
        ));
        let switch = picture(preset_for_controller("gamepad", Some(GamepadKind::Switch)));

        assert_eq!(xbox.dimensions(), playstation.dimensions());
        assert_ne!(xbox, playstation);
        assert_ne!(xbox, switch);
        assert_ne!(playstation, switch);
        assert_eq!(xbox, picture("gamepad"));
    }

    #[test]
    fn a_preset_tells_its_shape_without_being_drawn() {
        for id in BuiltinPresetId::ALL {
            let loaded = builtin_preset(id, InputDisplayStyle::Ghost, 1).unwrap();

            assert_eq!(
                preset_shape(id.id()),
                Some(PresetShape {
                    size: (loaded.preset.width, loaded.preset.height),
                    frame_share: loaded.frame_share,
                })
            );
        }
        assert_eq!(preset_shape("missing"), None);
    }

    #[test]
    fn straightened_pixels_carry_their_full_colour() {
        let mut image = RgbaImage::from_fn(3, 1, |x, _| {
            image::Rgba([[128, 0, 64, 128], [0, 0, 0, 0], [9, 200, 255, 255]][x as usize])
        });

        straighten_alpha(&mut image);

        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 128, 128]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 0]);
        assert_eq!(image.get_pixel(2, 0).0, [9, 200, 255, 255]);
    }

    #[test]
    fn a_larger_drawing_gets_a_sharper_atlas() {
        let loaded = resolve_preset("wasd-mouse", InputDisplayStyle::Keycaps, 3).unwrap();

        assert_eq!(loaded.atlas_scale, 3);
        assert_eq!(
            compose(&loaded, &DisplayState::default()).width(),
            loaded.preset.width * 3
        );
        assert_eq!(atlas_scale_for(342, 300.0), 1);
        assert_eq!(atlas_scale_for(342, 700.0), 3);
        assert_eq!(atlas_scale_for(342, 9000.0), MAX_ATLAS_SCALE);
        assert!(resolve_preset("missing", InputDisplayStyle::Keycaps, 1).is_none());
    }
}
