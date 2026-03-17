use markright_document::paragraph;

/// Default indent per list level in pixels.
pub const DEFAULT_LIST_INDENT: f32 = 20.0;

/// Compute the total left margin for a paragraph with a given style.
///
/// For list items an extra `list_indent` is added so the bullet/number marker
/// has room to hang before the text (the "hanging indent" in Word/PPT terms).
///
/// - Level 1 bullet: margin = `list_indent + 1 * list_indent` = text starts at
///   2×indent, marker drawn at 1×indent.
/// - Level 2 bullet: margin = `list_indent + 2 * list_indent` = text at 3×indent,
///   marker at 2×indent.
/// - Plain indented text (no list) level 1: margin = `1 * list_indent`, no marker.
pub fn compute_margin(style: &paragraph::Style, list_indent: f32) -> f32 {
    let hanging = if style.list.is_some() {
        list_indent
    } else {
        0.0
    };
    style.indent.left + hanging + style.level as f32 * list_indent
}

/// Returns the marker string for a list item.
pub fn marker_text(list: &paragraph::List, ordinal: usize) -> String {
    match list {
        paragraph::List::Bullet(bullet) => match bullet {
            paragraph::Bullet::Disc => "\u{2022}".into(),
            paragraph::Bullet::Circle => "\u{25E6}".into(),
            paragraph::Bullet::Square => "\u{25AA}".into(),
            paragraph::Bullet::Custom(c) => c.to_string(),
        },
        paragraph::List::Ordered(number) => format_number(ordinal, number),
    }
}

/// Count the ordinal position of `line` among consecutive list items at the
/// same level.
pub fn count_ordinal(paragraph_styles: &[paragraph::Style], line: usize) -> usize {
    let style = &paragraph_styles[line];
    let level = style.level;
    let mut ordinal = 1;
    let mut i = line;
    while i > 0 {
        i -= 1;
        let prev = &paragraph_styles[i];
        if prev.level < level || prev.list.is_none() {
            break;
        }
        if prev.level == level {
            ordinal += 1;
        }
    }
    ordinal
}

/// Returns the bullet variant appropriate for a given nesting level.
///
/// Follows the Google Docs convention: disc → circle → square, cycling.
pub fn bullet_for_level(level: u8) -> paragraph::Bullet {
    match level % 3 {
        0 => paragraph::Bullet::Disc,
        1 => paragraph::Bullet::Circle,
        _ => paragraph::Bullet::Square,
    }
}

/// Returns the number variant appropriate for a given nesting level.
///
/// Follows the convention: arabic → lower alpha → lower roman, cycling.
pub fn number_for_level(level: u8) -> paragraph::Number {
    match level % 3 {
        0 => paragraph::Number::Arabic,
        1 => paragraph::Number::LowerAlpha,
        _ => paragraph::Number::LowerRoman,
    }
}

fn format_number(n: usize, style: &paragraph::Number) -> String {
    match style {
        paragraph::Number::Arabic => format!("{n}."),
        paragraph::Number::LowerAlpha => format!("{}.", nth_alpha(n, false)),
        paragraph::Number::UpperAlpha => format!("{}.", nth_alpha(n, true)),
        paragraph::Number::LowerRoman => format!("{}.", to_roman(n, false)),
        paragraph::Number::UpperRoman => format!("{}.", to_roman(n, true)),
    }
}

fn nth_alpha(n: usize, upper: bool) -> String {
    if n == 0 {
        return String::new();
    }
    let base = if upper { b'A' } else { b'a' };
    let mut result = String::new();
    let mut remaining = n;
    while remaining > 0 {
        remaining -= 1;
        result.push((base + (remaining % 26) as u8) as char);
        remaining /= 26;
    }
    result.chars().rev().collect()
}

fn to_roman(n: usize, upper: bool) -> String {
    const VALS: &[(usize, &str, &str)] = &[
        (1000, "m", "M"),
        (900, "cm", "CM"),
        (500, "d", "D"),
        (400, "cd", "CD"),
        (100, "c", "C"),
        (90, "xc", "XC"),
        (50, "l", "L"),
        (40, "xl", "XL"),
        (10, "x", "X"),
        (9, "ix", "IX"),
        (5, "v", "V"),
        (4, "iv", "IV"),
        (1, "i", "I"),
    ];

    let mut result = String::new();
    let mut remaining = n;
    for &(val, lower, up) in VALS {
        while remaining >= val {
            result.push_str(if upper { up } else { lower });
            remaining -= val;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_numbering() {
        assert_eq!(nth_alpha(1, false), "a");
        assert_eq!(nth_alpha(26, false), "z");
        assert_eq!(nth_alpha(27, false), "aa");
        assert_eq!(nth_alpha(28, false), "ab");
        assert_eq!(nth_alpha(1, true), "A");
    }

    #[test]
    fn roman_numbering() {
        assert_eq!(to_roman(1, false), "i");
        assert_eq!(to_roman(4, false), "iv");
        assert_eq!(to_roman(9, false), "ix");
        assert_eq!(to_roman(14, false), "xiv");
        assert_eq!(to_roman(42, true), "XLII");
        assert_eq!(to_roman(2024, true), "MMXXIV");
    }

    #[test]
    fn ordinal_counting() {
        // Lists start at level 1; level 2 is nested.
        let styles = vec![
            paragraph::Style {
                list: Some(paragraph::List::Bullet(paragraph::Bullet::Disc)),
                level: 1,
                ..Default::default()
            },
            paragraph::Style {
                list: Some(paragraph::List::Bullet(paragraph::Bullet::Disc)),
                level: 1,
                ..Default::default()
            },
            paragraph::Style {
                list: Some(paragraph::List::Bullet(paragraph::Bullet::Circle)),
                level: 2,
                ..Default::default()
            },
            paragraph::Style {
                list: Some(paragraph::List::Bullet(paragraph::Bullet::Disc)),
                level: 1,
                ..Default::default()
            },
        ];

        assert_eq!(count_ordinal(&styles, 0), 1);
        assert_eq!(count_ordinal(&styles, 1), 2);
        assert_eq!(count_ordinal(&styles, 2), 1); // nested, resets
        assert_eq!(count_ordinal(&styles, 3), 3); // back to level 1
    }

    #[test]
    fn margin_computation() {
        let indent = DEFAULT_LIST_INDENT;

        let default = paragraph::Style::default();
        assert_eq!(compute_margin(&default, indent), 0.0);

        // Base list level = 1 → margin = hanging + 1*indent = 2*indent.
        let bullet = paragraph::Style {
            list: Some(paragraph::List::Bullet(paragraph::Bullet::Disc)),
            level: 1,
            ..Default::default()
        };
        assert_eq!(compute_margin(&bullet, indent), 2.0 * indent);

        // Nested list level = 2 → margin = hanging + 2*indent = 3*indent.
        let nested = paragraph::Style {
            list: Some(paragraph::List::Bullet(paragraph::Bullet::Circle)),
            level: 2,
            ..Default::default()
        };
        assert_eq!(compute_margin(&nested, indent), 3.0 * indent);

        // Plain indented text at level 2 → no hanging, just 2*indent.
        let indented_no_list = paragraph::Style {
            level: 2,
            ..Default::default()
        };
        assert_eq!(compute_margin(&indented_no_list, indent), 2.0 * indent);
    }
}
