use gpui::{Rgba, WindowAppearance, rgb};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub background: Rgba,
    pub card: Rgba,
    pub card_hover: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub primary: Rgba,
    pub primary_hover: Rgba,
    pub on_primary: Rgba,
    pub warning: Rgba,
}

impl Palette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }

    fn light() -> Self {
        Self {
            background: rgb(0xfcfcfc),
            card: rgb(0xf9f9f9),
            card_hover: rgb(0xf0f0f0),
            border: rgb(0xd9d9d9),
            text: rgb(0x202020),
            muted: rgb(0x646464),
            primary: rgb(0x202020),
            primary_hover: rgb(0x646464),
            on_primary: rgb(0xfcfcfc),
            warning: rgb(0xab6400),
        }
    }

    fn dark() -> Self {
        Self {
            background: rgb(0x111111),
            card: rgb(0x1c1c1c),
            card_hover: rgb(0x282828),
            border: rgb(0x444444),
            text: rgb(0xeeeeee),
            muted: rgb(0xa1a1a1),
            primary: rgb(0xeeeeee),
            primary_hover: rgb(0xa1a1a1),
            on_primary: rgb(0x111111),
            warning: rgb(0xffca16),
        }
    }
}
