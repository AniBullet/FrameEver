pub fn parse_number(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

pub fn format_number(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.6}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_the_way_tostring_does() {
        assert_eq!(format_number(0.), "0");
        assert_eq!(format_number(-100.), "-100");
        assert_eq!(format_number(0.05), "0.05");
        assert_eq!(format_number(1.5), "1.5");
    }

    #[test]
    fn parsing_rejects_the_work_in_progress_strings() {
        assert_eq!(parse_number("  12 "), Some(12.));
        assert_eq!(parse_number("-3.5"), Some(-3.5));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
