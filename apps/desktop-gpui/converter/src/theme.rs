use gpui::{Rgba, WindowAppearance};

#[path = "../../src/theme/tokens.rs"]
pub mod tokens;

use tokens::{Brand, Component, Semantic};

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
    pub step_ahead: Rgba,
    pub step_separator: Rgba,
    pub off_text: Rgba,
    pub warning_tint: Rgba,
}

impl Palette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        let appearance = Appearance::from_window(appearance);
        let brand = Brand::of(appearance);
        let semantic = Semantic::new(appearance);
        Self {
            semantic,
            component: Component::new(appearance),
            step_ahead: brand.neutral[8].to_rgba(),
            step_separator: brand.neutral[7].to_rgba(),
            off_text: brand.neutral[8].to_rgba(),
            warning_tint: brand.amber.tint.to_rgba(),
        }
    }
}

impl std::ops::Deref for Palette {
    type Target = Semantic;

    fn deref(&self) -> &Semantic {
        &self.semantic
    }
}
