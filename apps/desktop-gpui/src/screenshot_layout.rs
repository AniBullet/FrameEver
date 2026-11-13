use crate::i18n::n;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InspectorPage {
    #[default]
    Background,
    Screen,
    Layers,
}

impl InspectorPage {
    pub(crate) const ALL: [Self; 3] = [Self::Background, Self::Screen, Self::Layers];

    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::Background => "icons/image.svg",
            Self::Screen => "icons/monitor.svg",
            Self::Layers => "icons/layers.svg",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Background => n!("Background"),
            Self::Screen => n!("Screen"),
            Self::Layers => n!("Layers"),
        }
    }

    pub(crate) fn for_key(key: &str) -> Option<Self> {
        match key {
            "b" => Some(Self::Background),
            "p" | "h" | "e" => Some(Self::Screen),
            "l" => Some(Self::Layers),
            _ => None,
        }
    }
}

impl<'de> serde::Deserialize<'de> for InspectorPage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::ALL
            .into_iter()
            .find(|page| serde_json::to_value(page).is_ok_and(|value| value == stored))
            .unwrap_or_default())
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    TitleBar,
    PreviewToolbar,
    Preview,
    StatusBar,
    Page(InspectorPage),
    Selection,
    Keyboard,
    Toast,
    Settings,
    Library,
    Ticket(u32),
    Removed,
}

#[cfg(test)]
pub(crate) const INVENTORY: &[(&str, &[Place])] = &[
    (
        "Select, draw, arrow, rectangle, mask, circle, text",
        &[Place::PreviewToolbar, Place::Keyboard],
    ),
    (
        "Drag to draw, Shift constrains, move, eight handles, arrow end points, double-click to edit text",
        &[Place::Preview],
    ),
    (
        "Stroke color, hex input, width, fill with transparent, opacity",
        &[Place::Selection],
    ),
    (
        "Mask: blur or pixelate, intensity, kept inside the image",
        &[Place::Selection],
    ),
    (
        "Text color, size, inline editing",
        &[Place::Selection, Place::Preview],
    ),
    (
        "Done, Delete or Backspace, Escape",
        &[Place::Selection, Place::Keyboard],
    ),
    ("Copy and paste an annotation", &[Place::Keyboard]),
    (
        "Layers: list, reorder, delete, hover highlight, empty state, top is in front",
        &[Place::Page(InspectorPage::Layers), Place::Keyboard],
    ),
    (
        "Remember the layers panel",
        &[Place::Page(InspectorPage::Layers)],
    ),
    (
        "Background: wallpaper with themes, image with drag and clear, color with hex and transparent, gradient with from, to and presets",
        &[Place::Page(InspectorPage::Background), Place::Keyboard],
    ),
    (
        "Background blur; animated gradient shown as a still with its note",
        &[Place::Page(InspectorPage::Background)],
    ),
    (
        "Padding, rounding, corner style",
        &[Place::Page(InspectorPage::Screen), Place::Keyboard],
    ),
    (
        "Shadow with advanced size, opacity, blur",
        &[Place::Page(InspectorPage::Screen), Place::Keyboard],
    ),
    (
        "Border: toggle, width, color, opacity",
        &[Place::Page(InspectorPage::Screen), Place::Keyboard],
    ),
    (
        "Aspect ratio: Auto, 16:9, 9:16, 1:1, 4:3, 3:4",
        &[Place::PreviewToolbar],
    ),
    (
        "Crop dialog: size and position fields, ratios, snap, Full, Reset, Save, arrow nudge, Shift and Alt, Escape",
        &[Place::PreviewToolbar],
    ),
    (
        "Zoom 10 to 300%, Ctrl+wheel, pinch, Ctrl + and −, wheel pans",
        &[Place::StatusBar, Place::Preview, Place::Keyboard],
    ),
    ("Undo, redo, autosave", &[Place::TitleBar, Place::Keyboard]),
    (
        "Copy, flattened on white unless transparent",
        &[Place::TitleBar, Place::Keyboard],
    ),
    (
        "Save as PNG, annotations past the frame kept",
        &[Place::TitleBar, Place::Keyboard],
    ),
    ("Open folder, delete with confirmation", &[Place::TitleBar]),
    (
        "Loading preview, load errors, toasts",
        &[Place::Preview, Place::Toast],
    ),
    (
        "After a screenshot: open, copy, or both",
        &[Place::Settings],
    ),
    (
        "Open from the library, tray and deep link",
        &[Place::Library],
    ),
    (
        "Step number, line, highlighter, speech bubble, spotlight, magnifier, image, cut out",
        &[
            Place::PreviewToolbar,
            Place::Preview,
            Place::Selection,
            Place::Keyboard,
            Place::Ticket(104),
        ],
    ),
    ("Ctrl+K command palette", &[Place::Ticket(98)]),
    (
        "Upstream share link, sign in to share, upgrade, upload",
        &[Place::Removed],
    ),
    ("Upstream export fixes and speed-ups", &[Place::Ticket(65)]),
];

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: &str = include_str!("../../../docs/design/surfaces/screenshot-editor.js");

    fn design_inventory() -> Vec<String> {
        let start = DESIGN.find("table(\"inv\"").expect("inventory table");
        let end = start + DESIGN[start..].find("]);").expect("inventory end");
        let literals: Vec<&str> = DESIGN[start..end].split('"').skip(1).step_by(2).collect();
        literals[2..]
            .chunks(4)
            .map(|row| row[0].to_string())
            .collect()
    }

    #[test]
    fn every_design_inventory_row_has_a_place() {
        let design = design_inventory();
        assert_eq!(design.len(), 28);
        let ours: Vec<&str> = INVENTORY
            .iter()
            .map(|(capability, _)| *capability)
            .collect();
        for capability in &design {
            assert!(
                ours.contains(&capability.as_str()),
                "no place for {capability:?}"
            );
        }
        assert_eq!(ours.len(), design.len());
        for (capability, places) in INVENTORY {
            assert!(!places.is_empty(), "{capability:?} has no place");
        }
    }

    #[test]
    fn styling_keys_open_their_inspector_page() {
        assert_eq!(InspectorPage::for_key("b"), Some(InspectorPage::Background));
        for key in ["p", "h", "e"] {
            assert_eq!(InspectorPage::for_key(key), Some(InspectorPage::Screen));
        }
        assert_eq!(InspectorPage::for_key("l"), Some(InspectorPage::Layers));
        assert_eq!(InspectorPage::for_key("x"), None);
    }

    #[test]
    fn the_inspector_remembers_its_page_by_name() {
        for page in InspectorPage::ALL {
            let stored = serde_json::to_value(page).unwrap();
            assert_eq!(
                serde_json::from_value::<InspectorPage>(stored).unwrap(),
                page
            );
        }
        assert_eq!(
            serde_json::to_value(InspectorPage::Layers).unwrap(),
            serde_json::json!("layers")
        );
        for unknown in [serde_json::json!("nonsense"), serde_json::json!(3)] {
            assert_eq!(
                serde_json::from_value::<InspectorPage>(unknown).unwrap(),
                InspectorPage::default()
            );
        }
    }
}
