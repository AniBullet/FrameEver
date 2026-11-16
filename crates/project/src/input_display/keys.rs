pub mod key_codes {
    pub const BACKSPACE: u16 = 0x0008;
    pub const TAB: u16 = 0x0009;
    pub const ENTER: u16 = 0x000A;
    pub const PAUSE: u16 = 0x0013;
    pub const CAPS_LOCK: u16 = 0x0014;
    pub const ESCAPE: u16 = 0x001B;
    pub const SPACE: u16 = 0x0020;
    pub const PAGE_UP: u16 = 0x0021;
    pub const PAGE_DOWN: u16 = 0x0022;
    pub const END: u16 = 0x0023;
    pub const HOME: u16 = 0x0024;
    pub const LEFT: u16 = 0x0025;
    pub const UP: u16 = 0x0026;
    pub const RIGHT: u16 = 0x0027;
    pub const DOWN: u16 = 0x0028;
    pub const COMMA: u16 = 0x002C;
    pub const MINUS: u16 = 0x002D;
    pub const PERIOD: u16 = 0x002E;
    pub const SLASH: u16 = 0x002F;
    pub const DIGIT_0: u16 = 0x0030;
    pub const SEMICOLON: u16 = 0x003B;
    pub const EQUALS: u16 = 0x003D;
    pub const LETTER_A: u16 = 0x0041;
    pub const OPEN_BRACKET: u16 = 0x005B;
    pub const CLOSE_BRACKET: u16 = 0x005C;
    pub const BACK_SLASH: u16 = 0x005D;
    pub const NUMPAD_0: u16 = 0x0060;
    pub const NUMPAD_MULTIPLY: u16 = 0x006A;
    pub const NUMPAD_ADD: u16 = 0x006B;
    pub const NUMPAD_SUBTRACT: u16 = 0x006D;
    pub const NUMPAD_DECIMAL: u16 = 0x006E;
    pub const NUMPAD_DIVIDE: u16 = 0x006F;
    pub const F1: u16 = 0x0070;
    pub const NUMPAD_ENTER: u16 = 0x007D;
    pub const DELETE: u16 = 0x007F;
    pub const NUM_LOCK: u16 = 0x0090;
    pub const SCROLL_LOCK: u16 = 0x0091;
    pub const PRINT_SCREEN: u16 = 0x009A;
    pub const INSERT: u16 = 0x009B;
    pub const BACKQUOTE: u16 = 0x00C0;
    pub const QUOTE: u16 = 0x00DE;
    pub const CONTEXT_MENU: u16 = 0x020D;
    pub const SHIFT_LEFT: u16 = 0xA010;
    pub const CONTROL_LEFT: u16 = 0xA011;
    pub const ALT_LEFT: u16 = 0xA012;
    pub const META_LEFT: u16 = 0xA09D;
    pub const SHIFT_RIGHT: u16 = 0xB010;
    pub const CONTROL_RIGHT: u16 = 0xB011;
    pub const ALT_RIGHT: u16 = 0xB012;
    pub const META_RIGHT: u16 = 0xB09D;

    pub const fn letter(letter: char) -> u16 {
        LETTER_A + (letter as u16 - 'A' as u16)
    }

    pub const fn digit(digit: char) -> u16 {
        DIGIT_0 + (digit as u16 - '0' as u16)
    }
}

pub fn key_code(recorded: &str) -> Option<u16> {
    use key_codes::*;

    let mut characters = recorded.chars();
    if let (Some(only), None) = (characters.next(), characters.next())
        && only.is_ascii_uppercase()
    {
        return Some(letter(only));
    }
    if let Some(number) = suffix_number(recorded, "Key")
        && number <= 9
    {
        return Some(DIGIT_0 + number);
    }
    if let Some(number) = suffix_number(recorded, "Numpad")
        && number <= 9
    {
        return Some(NUMPAD_0 + number);
    }
    if let Some(number) = suffix_number(recorded, "F")
        && (1..=12).contains(&number)
    {
        return Some(F1 + number - 1);
    }
    Some(match recorded {
        "Escape" => ESCAPE,
        "Space" => SPACE,
        "LControl" => CONTROL_LEFT,
        "RControl" => CONTROL_RIGHT,
        "LShift" => SHIFT_LEFT,
        "RShift" => SHIFT_RIGHT,
        "LAlt" => ALT_LEFT,
        "RAlt" => ALT_RIGHT,
        "Meta" | "LMeta" | "Command" => META_LEFT,
        "RMeta" => META_RIGHT,
        "Enter" => ENTER,
        "Up" => UP,
        "Down" => DOWN,
        "Left" => LEFT,
        "Right" => RIGHT,
        "Backspace" => BACKSPACE,
        "CapsLock" => CAPS_LOCK,
        "Tab" => TAB,
        "Home" => HOME,
        "End" => END,
        "PageUp" => PAGE_UP,
        "PageDown" => PAGE_DOWN,
        "Insert" => INSERT,
        "Delete" => DELETE,
        "Grave" => BACKQUOTE,
        "Minus" => MINUS,
        "Equal" => EQUALS,
        "LeftBracket" => OPEN_BRACKET,
        "RightBracket" => CLOSE_BRACKET,
        "BackSlash" => BACK_SLASH,
        "Semicolon" => SEMICOLON,
        "Apostrophe" => QUOTE,
        "Comma" => COMMA,
        "Dot" => PERIOD,
        "Slash" => SLASH,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCodeTable {
    Sequential,
    Uiohook13,
    Uiohook12,
}

impl KeyCodeTable {
    pub const ALL: [Self; 3] = [Self::Sequential, Self::Uiohook13, Self::Uiohook12];

    pub fn detect(codes: &[u32]) -> Self {
        let mut best = (Self::Sequential, 0);
        for table in Self::ALL {
            let known = codes
                .iter()
                .filter(|code| table.key(**code).is_some())
                .count();
            if known > best.1 {
                best = (table, known);
            }
        }
        best.0
    }

    pub fn key(self, code: u32) -> Option<u16> {
        let code = u16::try_from(code).ok()?;
        match self {
            Self::Sequential => sequential_key(code),
            Self::Uiohook13 => known_key(code).then_some(code),
            Self::Uiohook12 => scancode_key(code),
        }
    }
}

fn known_key(code: u16) -> bool {
    use key_codes::*;
    matches!(code, 0x0041..=0x005A | 0x0030..=0x0039 | 0x0060..=0x0069 | 0x0070..=0x007B)
        || [
            BACKSPACE,
            TAB,
            ENTER,
            PAUSE,
            CAPS_LOCK,
            ESCAPE,
            SPACE,
            PAGE_UP,
            PAGE_DOWN,
            END,
            HOME,
            LEFT,
            UP,
            RIGHT,
            DOWN,
            COMMA,
            MINUS,
            PERIOD,
            SLASH,
            SEMICOLON,
            EQUALS,
            OPEN_BRACKET,
            CLOSE_BRACKET,
            BACK_SLASH,
            NUMPAD_MULTIPLY,
            NUMPAD_ADD,
            NUMPAD_SUBTRACT,
            NUMPAD_DECIMAL,
            NUMPAD_DIVIDE,
            NUMPAD_ENTER,
            DELETE,
            NUM_LOCK,
            SCROLL_LOCK,
            PRINT_SCREEN,
            INSERT,
            BACKQUOTE,
            QUOTE,
            CONTEXT_MENU,
            SHIFT_LEFT,
            CONTROL_LEFT,
            ALT_LEFT,
            META_LEFT,
            SHIFT_RIGHT,
            CONTROL_RIGHT,
            ALT_RIGHT,
            META_RIGHT,
        ]
        .contains(&code)
}

fn sequential_key(code: u16) -> Option<u16> {
    use key_codes::*;
    Some(match code {
        1 => ESCAPE,
        2..=13 => F1 + code - 2,
        32 => BACKQUOTE,
        33..=41 => DIGIT_0 + code - 32,
        42 => DIGIT_0,
        43 => MINUS,
        44 => EQUALS,
        45 => BACKSPACE,
        46 => TAB,
        47 => CAPS_LOCK,
        48..=73 => LETTER_A + code - 48,
        74 => OPEN_BRACKET,
        75 => CLOSE_BRACKET,
        76 => BACK_SLASH,
        77 => SEMICOLON,
        78 => QUOTE,
        79 => ENTER,
        80 => COMMA,
        81 => PERIOD,
        82 => SLASH,
        83 => SPACE,
        96 => PRINT_SCREEN,
        97 => SCROLL_LOCK,
        98 => PAUSE,
        101 => INSERT,
        102 => DELETE,
        103 => HOME,
        104 => END,
        105 => PAGE_UP,
        106 => PAGE_DOWN,
        107 => UP,
        108 => LEFT,
        109 => RIGHT,
        110 => DOWN,
        112 => NUM_LOCK,
        113..=121 => NUMPAD_0 + code - 112,
        122 => NUMPAD_0,
        124 => NUMPAD_DIVIDE,
        125 => NUMPAD_MULTIPLY,
        126 => NUMPAD_SUBTRACT,
        128 => NUMPAD_ADD,
        129 => NUMPAD_ENTER,
        130 => NUMPAD_DECIMAL,
        144 => SHIFT_LEFT,
        145 => SHIFT_RIGHT,
        146 => CONTROL_LEFT,
        147 => CONTROL_RIGHT,
        148 => ALT_LEFT,
        149 => ALT_RIGHT,
        150 => META_LEFT,
        151 => META_RIGHT,
        152 => CONTEXT_MENU,
        _ => return None,
    })
}

fn scancode_key(code: u16) -> Option<u16> {
    use key_codes::*;
    let row = |first: u16, keys: &str| {
        keys.chars()
            .nth(usize::from(code - first))
            .map(|key| match key {
                'A'..='Z' => letter(key),
                _ => digit(key),
            })
    };
    Some(match code {
        0x0001 => ESCAPE,
        0x0002..=0x000B => return row(0x0002, "1234567890"),
        0x000C => MINUS,
        0x000D => EQUALS,
        0x000E => BACKSPACE,
        0x000F => TAB,
        0x0010..=0x0019 => return row(0x0010, "QWERTYUIOP"),
        0x001A => OPEN_BRACKET,
        0x001B => CLOSE_BRACKET,
        0x001C => ENTER,
        0x001D => CONTROL_LEFT,
        0x001E..=0x0026 => return row(0x001E, "ASDFGHJKL"),
        0x0027 => SEMICOLON,
        0x0028 => QUOTE,
        0x0029 => BACKQUOTE,
        0x002A => SHIFT_LEFT,
        0x002B => BACK_SLASH,
        0x002C..=0x0032 => return row(0x002C, "ZXCVBNM"),
        0x0033 => COMMA,
        0x0034 => PERIOD,
        0x0035 => SLASH,
        0x0036 => SHIFT_RIGHT,
        0x0037 => NUMPAD_MULTIPLY,
        0x0038 => ALT_LEFT,
        0x0039 => SPACE,
        0x003A => CAPS_LOCK,
        0x003B..=0x0044 => F1 + code - 0x003B,
        0x0045 => NUM_LOCK,
        0x0046 => SCROLL_LOCK,
        0x0047..=0x0049 => NUMPAD_0 + 7 + code - 0x0047,
        0x004A => NUMPAD_SUBTRACT,
        0x004B..=0x004D => NUMPAD_0 + 4 + code - 0x004B,
        0x004E => NUMPAD_ADD,
        0x004F..=0x0051 => NUMPAD_0 + 1 + code - 0x004F,
        0x0052 => NUMPAD_0,
        0x0053 => NUMPAD_DECIMAL,
        0x0057 => F1 + 10,
        0x0058 => F1 + 11,
        0x0E1C => NUMPAD_ENTER,
        0x0E1D => CONTROL_RIGHT,
        0x0E38 => ALT_RIGHT,
        0x0E47 => HOME,
        0x0E49 => PAGE_UP,
        0x0E4F => END,
        0x0E51 => PAGE_DOWN,
        0x0E52 => INSERT,
        0x0E53 => DELETE,
        0x0E5B => META_LEFT,
        0x0E5C => META_RIGHT,
        0xE048 => UP,
        0xE04B => LEFT,
        0xE04D => RIGHT,
        0xE050 => DOWN,
        _ => return None,
    })
}

pub fn keystroke_code(key: &str) -> Option<u16> {
    use key_codes::*;

    let mut characters = key.chars();
    if let (Some(only), None) = (characters.next(), characters.next()) {
        return Some(match only {
            'a'..='z' => letter(only.to_ascii_uppercase()),
            'A'..='Z' => letter(only),
            '0'..='9' => digit(only),
            ',' => COMMA,
            '.' => PERIOD,
            '/' => SLASH,
            ';' => SEMICOLON,
            '\'' => QUOTE,
            '[' => OPEN_BRACKET,
            ']' => CLOSE_BRACKET,
            '\\' => BACK_SLASH,
            '-' => MINUS,
            '=' => EQUALS,
            '`' => BACKQUOTE,
            ' ' => SPACE,
            _ => return None,
        });
    }
    if let Some(number) = suffix_number(key, "f")
        && (1..=12).contains(&number)
    {
        return Some(F1 + number - 1);
    }
    Some(match key {
        "space" => SPACE,
        "enter" => ENTER,
        "tab" => TAB,
        "escape" => ESCAPE,
        "backspace" => BACKSPACE,
        "delete" => DELETE,
        "insert" => INSERT,
        "home" => HOME,
        "end" => END,
        "pageup" => PAGE_UP,
        "pagedown" => PAGE_DOWN,
        "up" => UP,
        "down" => DOWN,
        "left" => LEFT,
        "right" => RIGHT,
        "capslock" => CAPS_LOCK,
        "shift" => SHIFT_LEFT,
        "control" => CONTROL_LEFT,
        "alt" => ALT_LEFT,
        "platform" => META_LEFT,
        "menu" => CONTEXT_MENU,
        _ => return None,
    })
}

pub fn key_label(code: u16) -> String {
    use key_codes::*;
    let name = match code {
        0x0041..=0x005A | 0x0030..=0x0039 => {
            return char::from(code as u8).to_string();
        }
        0x0070..=0x007B => return format!("F{}", code - F1 + 1),
        0x0060..=0x0069 => return format!("Num {}", code - NUMPAD_0),
        ESCAPE => "Esc",
        SPACE => "Space",
        ENTER | NUMPAD_ENTER => "Enter",
        TAB => "Tab",
        BACKSPACE => "Back",
        CAPS_LOCK => "Caps",
        DELETE => "Del",
        INSERT => "Ins",
        HOME => "Home",
        END => "End",
        PAGE_UP => "PgUp",
        PAGE_DOWN => "PgDn",
        UP => "\u{2191}",
        DOWN => "\u{2193}",
        LEFT => "\u{2190}",
        RIGHT => "\u{2192}",
        SHIFT_LEFT | SHIFT_RIGHT => "Shift",
        CONTROL_LEFT | CONTROL_RIGHT => "Ctrl",
        ALT_LEFT | ALT_RIGHT => "Alt",
        META_LEFT | META_RIGHT => "Win",
        CONTEXT_MENU => "Menu",
        COMMA => ",",
        PERIOD => ".",
        SLASH => "/",
        SEMICOLON => ";",
        QUOTE => "'",
        OPEN_BRACKET => "[",
        CLOSE_BRACKET => "]",
        BACK_SLASH => "\\",
        MINUS => "-",
        EQUALS => "=",
        BACKQUOTE => "`",
        _ => "",
    };
    name.to_string()
}

fn suffix_number(recorded: &str, prefix: &str) -> Option<u16> {
    let digits = recorded.strip_prefix(prefix)?;
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_names_map_to_the_codes_presets_use() {
        for (recorded, code) in [
            ("W", 0x0057),
            ("A", 0x0041),
            ("Key1", 0x0031),
            ("Key0", 0x0030),
            ("Numpad1", 0x0061),
            ("F1", 0x0070),
            ("F12", 0x007B),
            ("Escape", 0x001B),
            ("Space", 0x0020),
            ("Enter", 0x000A),
            ("LShift", 0xA010),
            ("RControl", 0xB011),
            ("Meta", 0xA09D),
            ("Up", 0x0026),
            ("Grave", 0x00C0),
            ("Apostrophe", 0x00DE),
        ] {
            assert_eq!(key_code(recorded), Some(code), "{recorded}");
        }
    }

    #[test]
    fn the_codes_current_presets_write_map_to_their_keys() {
        use key_codes::*;
        for (code, key) in [
            (1, ESCAPE),
            (2, F1),
            (13, F1 + 11),
            (32, BACKQUOTE),
            (33, digit('1')),
            (41, digit('9')),
            (42, digit('0')),
            (46, TAB),
            (48, letter('A')),
            (64, letter('Q')),
            (70, letter('W')),
            (73, letter('Z')),
            (74, OPEN_BRACKET),
            (75, CLOSE_BRACKET),
            (76, BACK_SLASH),
            (79, ENTER),
            (83, SPACE),
            (107, UP),
            (110, DOWN),
            (113, NUMPAD_0 + 1),
            (122, NUMPAD_0),
            (144, SHIFT_LEFT),
            (147, CONTROL_RIGHT),
            (151, META_RIGHT),
            (152, CONTEXT_MENU),
        ] {
            assert_eq!(KeyCodeTable::Sequential.key(code), Some(key), "{code}");
        }
        assert_eq!(KeyCodeTable::Sequential.key(0), None);
        assert_eq!(KeyCodeTable::Sequential.key(20), None);
        assert_eq!(KeyCodeTable::Sequential.key(70_000), None);
    }

    #[test]
    fn the_two_older_code_tables_map_to_the_same_keys() {
        use key_codes::*;
        for (old, new, key) in [
            (0x0011, 0x0057, letter('W')),
            (0x001E, 0x0041, letter('A')),
            (0x0002, 0x0031, digit('1')),
            (0x000B, 0x0030, digit('0')),
            (0x0001, 0x001B, ESCAPE),
            (0x003B, 0x0070, F1),
            (0x0058, 0x007B, F1 + 11),
            (0x001A, 0x005B, OPEN_BRACKET),
            (0x001B, 0x005C, CLOSE_BRACKET),
            (0x002B, 0x005D, BACK_SLASH),
            (0x0039, 0x0020, SPACE),
            (0x002A, 0xA010, SHIFT_LEFT),
            (0x0E1D, 0xB011, CONTROL_RIGHT),
            (0xE048, 0x0026, UP),
            (0x004F, 0x0061, NUMPAD_0 + 1),
            (0x0E53, 0x007F, DELETE),
        ] {
            assert_eq!(KeyCodeTable::Uiohook12.key(old), Some(key), "{old:#x}");
            assert_eq!(KeyCodeTable::Uiohook13.key(new), Some(key), "{new:#x}");
        }
        assert_eq!(KeyCodeTable::Uiohook13.key(0x0092), None);
        assert_eq!(KeyCodeTable::Uiohook12.key(0x0090), None);
    }

    #[test]
    fn a_preset_s_codes_tell_which_table_wrote_them() {
        assert_eq!(
            KeyCodeTable::detect(&[64, 70, 52, 144, 48, 66, 51, 146, 83]),
            KeyCodeTable::Sequential
        );
        assert_eq!(
            KeyCodeTable::detect(&[0x57, 0x41, 0x53, 0x44]),
            KeyCodeTable::Uiohook13
        );
        assert_eq!(
            KeyCodeTable::detect(&[0x57, 0x41, 0x53, 0x44, 0xA010, 0x20]),
            KeyCodeTable::Uiohook13
        );
        assert_eq!(
            KeyCodeTable::detect(&[0x11, 0x1E, 0x1F, 0x20, 0x2A, 0x1D, 0x39]),
            KeyCodeTable::Uiohook12
        );
        assert_eq!(KeyCodeTable::detect(&[0, 0]), KeyCodeTable::Sequential);
    }

    #[test]
    fn keys_pressed_in_a_window_map_to_the_codes_presets_use() {
        use key_codes::*;
        for (key, code, label) in [
            ("w", letter('W'), "W"),
            ("7", digit('7'), "7"),
            ("space", SPACE, "Space"),
            ("escape", ESCAPE, "Esc"),
            ("backspace", BACKSPACE, "Back"),
            ("f5", F1 + 4, "F5"),
            ("up", UP, "\u{2191}"),
            ("shift", SHIFT_LEFT, "Shift"),
            ("control", CONTROL_LEFT, "Ctrl"),
            ("platform", META_LEFT, "Win"),
            ("[", OPEN_BRACKET, "["),
            ("\\", BACK_SLASH, "\\"),
            ("pagedown", PAGE_DOWN, "PgDn"),
        ] {
            assert_eq!(keystroke_code(key), Some(code), "{key}");
            assert_eq!(key_label(code), label, "{key}");
        }
        for key in ["", "f13", "f0", "ä", "volumeup"] {
            assert_eq!(keystroke_code(key), None, "{key}");
        }
    }

    #[test]
    fn names_the_recorder_does_not_write_have_no_code() {
        for recorded in ["", "w", "Key10", "F13", "F0", "NumpadAdd", "VolumeUp", "Ä"] {
            assert_eq!(key_code(recorded), None, "{recorded}");
        }
    }
}
