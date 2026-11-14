use gpui::WindowAppearance;

#[path = "../../src/theme/tokens.rs"]
pub mod tokens;

use tokens::{Component, Semantic};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn from_window(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub semantic: Semantic,
    pub component: Component,
}

impl Palette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        let appearance = Appearance::from_window(appearance);
        Self {
            semantic: Semantic::new(appearance),
            component: Component::new(appearance),
        }
    }
}

impl std::ops::Deref for Palette {
    type Target = Semantic;

    fn deref(&self) -> &Semantic {
        &self.semantic
    }
}
