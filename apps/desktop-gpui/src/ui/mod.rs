//! The shared component library.
//!
//! Every window in this app was hand-rolling its own buttons, toggles, selects,
//! sliders and cards. The Tauri app does not: Tauri `ui-solid/src/Button.tsx`,
//! `components/Toggle.tsx` and `routes/editor/ui.tsx` are imported by the
//! editor, the settings pages, the teleprompter, the main window and the
//! screenshot editor alike. This module is that set, transcribed once.
//!
//! ## How a component is themed
//!
//! `theme.css` re-skins controls per window through attribute selectors --
//! `[data-macos-native-material="settings"] .cap-toggle { background:
//! var(--macos-settings-control-fill) }` and friends. There is no general
//! mechanism here to match it, because the remaps are not general: only three
//! surfaces exist in the shipping CSS and each one re-points a specific,
//! enumerable list of properties. So every component carries **named
//! constructors, one per surface**, and the quoted CSS rule lives in the
//! constructor:
//!
//! | constructor | window | token set |
//! |---|---|---|
//! | `::body(theme, ..)` | main window | Radix, with `Theme::body_*`'s panel-material remaps |
//! | `::settings(theme, ..)` | settings | `--macos-settings-*` (`Theme::settings_*`) |
//! | `::glass(theme, ..)` | teleprompter | bare glass: `gray-12` at 5/7/8/10 % |
//! | `::plain(theme, ..)` | editor, mode select | Radix, no material |
//!
//! A call site that needs a colour no surface provides sets it explicitly --
//! the builders all expose the individual colours -- so consolidation never
//! costs fidelity.
//!
//! ## Event handlers
//!
//! Handlers are `impl Fn(&ClickEvent, &mut Window, &mut App)`, which is exactly
//! what `cx.listener(..)` produces for any window type. Components are
//! therefore window-agnostic without a generic parameter.

mod button;
mod collapsible;
mod color_picker;
mod editor_button;
mod field;
mod kbd;
mod menu;
mod microphone_level;
mod number_text;
mod position_pad;
mod progress;
mod radio_cards;
mod record_mark;
mod segmented;
mod select;
mod selection_header;
mod slider;
mod surface;
// `pub` where its siblings are private: the app menu's Edit items are built
// from this module's actions by name (`crate::menus::build_menus`), and a
// re-export would not give the menu a path to `text_input::Undo` as a type.
pub mod text_input;
mod toggle;
mod tooltip;
mod windows_caption;

pub use button::{Button, ButtonSize, ButtonVariant, ClickHandler, IconButton};
pub use collapsible::{Collapsible, CollapsibleState};
pub use color_picker::{
    ColorPicker, ColorPickerSnapshot, hsv_to_rgb, hue_from_point, rgb_to_hsv, sv_from_point,
};
pub use editor_button::EditorButton;
pub use field::{Field, Subfield};
pub use kbd::{KbdChip, kbd_symbol};
pub use menu::{Menu, MenuItem, MenuKey, MenuState};
pub(crate) use microphone_level::{MicrophoneLevel, MicrophoneLevelKind};
pub use number_text::{format_number, parse_number};
pub use position_pad::{PositionPad, pad_position};
pub use progress::CircularProgress;
pub use radio_cards::{RadioCard, RadioCards};
pub use record_mark::{record_mark, stop_mark};
pub use segmented::{SegmentOption, SegmentedControl, option_at};
pub use select::Select;
pub use selection_header::{SelectionHeader, selection_label, zoom_selection_label};
pub use slider::{
    Slider, SliderDrag, SliderTrack, fraction_from_x, keyboard_slider_value, snap_to_step,
    value_at as slider_value_at, value_from_fraction,
};
pub use surface::{Card, Section, SettingRow};
pub use text_input::{
    TextInput, TextInputEvent, TextInputState, bind_keys as bind_text_input_keys,
    text_input_has_focus,
};
pub use toggle::Toggle;
pub use tooltip::{TOOLTIP_SHOW_DELAY, Tooltip, file_name};
#[cfg(target_os = "windows")]
pub use windows_caption::windows_caption_controls;
