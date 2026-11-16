pub mod key_codes {
    pub const BACKSPACE: u16 = 0x0008;
    pub const TAB: u16 = 0x0009;
    pub const ENTER: u16 = 0x000A;
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
    pub const BACK_SLASH: u16 = 0x005C;
    pub const CLOSE_BRACKET: u16 = 0x005D;
    pub const NUMPAD_0: u16 = 0x0060;
    pub const F1: u16 = 0x0070;
    pub const DELETE: u16 = 0x007F;
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
    fn names_the_recorder_does_not_write_have_no_code() {
        for recorded in ["", "w", "Key10", "F13", "F0", "NumpadAdd", "VolumeUp", "Ä"] {
            assert_eq!(key_code(recorded), None, "{recorded}");
        }
    }
}
