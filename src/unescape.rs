//! Decoding the escape codes a JavaScript string or name can be written in.
//!
//! One loader family in this campaign is ordinary JavaScript with every
//! character written as `\uXXXX`. The file does not contain the indicator
//! strings, so a fixed-string match against its bytes finds nothing, whatever
//! is in `ioc/`. This makes a decoded copy for the matcher to read as well.
//! See ADR-0040.
//!
//! Three forms are decoded: `\uXXXX` (with a surrogate pair joined into one
//! character), `\u{X...}` and `\xNN`. Anything else, an invalid sequence
//! included, is copied as it is.
//!
//! A backslash in front of an escape is not taken to cancel it. In JavaScript
//! `"\\u0041"` is a backslash and five letters, not an `A`. It is decoded here
//! anyway, because that is how a loader nested one string deep is written (a
//! script inside a JSON string, as in a `tasks.json`), and the copy is only
//! ever matched against the indicator set. It is never written back, shown or
//! cut from, so decoding too much costs nothing but a match the indicator set
//! would have to ask for.
//!
//! Linear in the length of the text: each backslash looks at most twelve bytes
//! ahead. No regex.

/// `text` with its escape codes decoded, or `None` when it has none to decode.
/// A file without a `\u` or `\x` in it costs one pass and no copy.
pub fn decode(text: &str) -> Option<String> {
    if !has_escape(text.as_bytes()) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0usize;
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes.get(at) != Some(&b'\\') {
            at += 1;
            continue;
        }
        match escape_at(bytes, at) {
            Some((c, len)) => {
                // Every escape is ASCII, so `at` and `at + len` are both
                // character boundaries.
                out.push_str(text.get(copied..at).unwrap_or_default());
                out.push(c);
                at += len;
                copied = at;
            }
            None => at += 1,
        }
    }
    if copied == 0 {
        // Something that looked like an escape and was not one.
        return None;
    }
    out.push_str(text.get(copied..).unwrap_or_default());
    Some(out)
}

/// Whether `text` contains `needle`, as written or once decoded. For a word
/// that decides whether a file is looked at further, such as `folderOpen` in
/// a `tasks.json`, which JSON lets be written in escapes and still means the
/// same to the editor that reads it.
pub fn contains_decoded(text: &str, needle: &str) -> bool {
    text.contains(needle) || decode(text).is_some_and(|d| d.contains(needle))
}

/// Whether there is anything here that could be an escape. Cheap enough to
/// ask of every file before deciding to make a copy.
pub fn has_escape(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w == b"\\u" || w == b"\\x")
}

fn hex(byte: u8) -> Option<u32> {
    char::from(byte).to_digit(16)
}

/// `count` hex digits starting at `from`, as a number.
fn hex_run(bytes: &[u8], from: usize, count: usize) -> Option<u32> {
    let digits = bytes.get(from..from.checked_add(count)?)?;
    digits
        .iter()
        .try_fold(0u32, |acc, b| Some(acc * 16 + hex(*b)?))
}

/// The character the escape at `at` stands for, and how many bytes it takes.
fn escape_at(bytes: &[u8], at: usize) -> Option<(char, usize)> {
    match bytes.get(at + 1)? {
        b'x' => {
            let value = hex_run(bytes, at + 2, 2)?;
            Some((char::from_u32(value)?, 4))
        }
        b'u' if bytes.get(at + 2) == Some(&b'{') => {
            // At most six digits: 10FFFF is the largest character there is.
            let start = at + 3;
            let digits = bytes
                .get(start..)?
                .iter()
                .take(7)
                .take_while(|b| b.is_ascii_hexdigit())
                .count();
            if digits == 0 || digits > 6 || bytes.get(start + digits) != Some(&b'}') {
                return None;
            }
            let value = hex_run(bytes, start, digits)?;
            Some((char::from_u32(value)?, 3 + digits + 1))
        }
        b'u' => {
            let unit = hex_run(bytes, at + 2, 4)?;
            if (0xD800..=0xDBFF).contains(&unit) {
                // A high surrogate is half a character. Joined with the low
                // half that follows it, or left alone.
                if bytes.get(at + 6) != Some(&b'\\') || bytes.get(at + 7) != Some(&b'u') {
                    return None;
                }
                let low = hex_run(bytes, at + 8, 4)?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return None;
                }
                let value = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                return Some((char::from_u32(value)?, 12));
            }
            // A low surrogate on its own is not a character.
            Some((char::from_u32(unit)?, 6))
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn decoded(text: &str) -> Option<String> {
        decode(text)
    }

    #[test]
    fn text_without_escapes_is_not_copied() {
        assert_eq!(decoded("export default { plugins: {} }\n"), None);
        assert_eq!(decoded(""), None);
        assert_eq!(decoded("C:\\Users\\x"), None);
    }

    #[test]
    fn each_form_decodes_to_its_character() {
        assert_eq!(decoded("\\u0067lobal").as_deref(), Some("global"));
        assert_eq!(decoded("\\x67lobal").as_deref(), Some("global"));
        assert_eq!(decoded("\\u{67}lobal").as_deref(), Some("global"));
        assert_eq!(decoded("\\u{1F600}").as_deref(), Some("\u{1F600}"));
        assert_eq!(decoded("caf\\u00E9").as_deref(), Some("caf\u{e9}"));
        // Escapes mixed with plain text, the way a loader is usually written.
        assert_eq!(
            decoded("var a='\\u0041'+\"B\\x43\";").as_deref(),
            Some("var a='A'+\"BC\";")
        );
    }

    #[test]
    fn a_surrogate_pair_is_one_character_and_half_of_one_is_left_alone() {
        assert_eq!(decoded("\\uD83D\\uDE00").as_deref(), Some("\u{1F600}"));
        assert_eq!(decoded("\\uD83Dx"), None);
        assert_eq!(decoded("\\uDE00"), None);
        assert_eq!(decoded("\\uD83D\\u0041").as_deref(), Some("\\uD83DA"));
    }

    #[test]
    fn what_is_not_an_escape_is_copied_as_it_is() {
        assert_eq!(decoded("\\u00"), None);
        assert_eq!(decoded("\\u00zz"), None);
        assert_eq!(decoded("\\x4"), None);
        assert_eq!(decoded("\\u{}"), None);
        assert_eq!(decoded("\\u{1234567}"), None);
        assert_eq!(decoded("\\u{110000}"), None);
        assert_eq!(decoded("\\u{41"), None);
        assert_eq!(
            decoded("\\u00zz \\u0041 \\x").as_deref(),
            Some("\\u00zz A \\x")
        );
    }

    #[test]
    fn a_backslash_before_an_escape_does_not_stop_it_being_decoded() {
        // A loader inside a JSON string is written one level deeper. The copy
        // is only matched, so decoding it is the safe direction.
        assert_eq!(decoded("\\\\u0041").as_deref(), Some("\\A"));
    }

    #[test]
    fn non_ascii_text_around_an_escape_survives() {
        assert_eq!(decoded("ünï \\u0041 çødé").as_deref(), Some("ünï A çødé"));
    }

    #[test]
    fn a_word_written_in_escapes_is_still_that_word() {
        assert!(contains_decoded(r#"{"runOn":"folderOpen"}"#, "folderOpen"));
        assert!(contains_decoded(r#"{"runOn":"folderOpen"}"#, "folderOpen"));
        assert!(!contains_decoded(r#"{"runOn":"default"}"#, "folderOpen"));
    }

    #[test]
    fn work_is_linear_in_the_length() {
        // A long run of backslashes and near-misses has to finish quickly.
        let text = "\\u12".repeat(200_000);
        assert_eq!(decoded(&text), None);
    }
}
