//! Every string that crosses between Desktop and a companion app passes through here when it is
//! written, and is checked against it again when it is read, however carefully it was made on the
//! other side.

/// The characters that change how text is laid out without being seen: the bidirectional
/// embeddings, overrides and isolates, the invisible separators and operators, and the byte-order
/// mark. A name holding one can read differently in a companion app than in Desktop. Zero-width
/// joiners and non-joiners stay, since scripts and emoji need them to be spelled at all.
fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{206A}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// `input` as it may cross: control and invisible formatting characters dropped, every run of
/// whitespace made one space, nothing at either end, and at most `max_chars` characters.
pub fn sanitize_text(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    let mut space = false;
    for c in input.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() || is_invisible_format(c) {
            continue;
        }
        let needed = if space { 2 } else { 1 };
        if count + needed > max_chars {
            break;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
        count += needed;
    }
    out
}

/// Whether `input` is already exactly what [`sanitize_text`] would make of it, and not empty.
pub fn is_sanitized(input: &str, max_chars: usize) -> bool {
    !input.is_empty() && sanitize_text(input, max_chars) == input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_names_cross_unchanged() {
        for name in [
            "Mallow",
            "Biscuit",
            "Dr. Pickles",
            "Zoë",
            "ミケ",
            "Ana-Lucía",
            "🐸 Frog",
        ] {
            assert_eq!(sanitize_text(name, 24), name);
            assert!(is_sanitized(name, 24));
        }
    }

    #[test]
    fn hostile_text_is_made_harmless() {
        assert_eq!(
            sanitize_text("  Mallow\n\tthe\r\nBrave  ", 24),
            "Mallow the Brave"
        );
        assert_eq!(sanitize_text("Mal\u{0}low\u{7}", 24), "Mallow");
        assert_eq!(sanitize_text("\u{202E}wollaM", 24), "wollaM");
        assert_eq!(
            sanitize_text("Ma\u{2066}l\u{2069}low\u{FEFF}", 24),
            "Mallow"
        );
        assert_eq!(sanitize_text("\u{200B}\u{200B}", 24), "");
        assert!(!is_sanitized("", 24));
        assert!(!is_sanitized("Mallow ", 24));
        assert!(!is_sanitized("\u{202E}Mallow", 24));
    }

    #[test]
    fn length_is_counted_in_characters_and_never_ends_on_a_space() {
        assert_eq!(sanitize_text(&"é".repeat(40), 24).chars().count(), 24);
        assert_eq!(sanitize_text("abc def", 4), "abc");
        assert_eq!(sanitize_text("abc def", 5), "abc d");
        assert!(is_sanitized(&sanitize_text(&"ab ".repeat(30), 24), 24));
    }
}
