use unicode_script::{Script, UnicodeScript};

fn is_han(character: char) -> bool {
    character.script() == Script::Han && character.is_alphabetic()
}

pub fn is_chinese_script(text: &str) -> bool {
    let count = text.chars().filter(|character| is_han(*character)).count();
    count > text.chars().count() / 3
}

pub fn count_words(text: &str) -> usize {
    let han_count = text.chars().filter(|character| is_han(*character)).count();
    if han_count == 0 {
        return text.split_whitespace().count();
    }
    han_count
        + text
            .split(is_han)
            .flat_map(str::split_whitespace)
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .count()
}
