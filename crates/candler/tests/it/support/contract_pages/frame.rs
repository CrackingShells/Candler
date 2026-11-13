//! The frame every contract page keeps: five `##` sections in order, and Notes that are short and
//! stamped.

use regex::Regex;

use super::hand::{level_two_headings, section};

/// The `##` sections of a page, in order.
pub const SECTIONS: [&str; 5] = ["Surface", "Version", "Fields", "Refusals", "Notes"];

/// The most words a page's Notes hold, not counting bullet markers and stamps.
pub const MAX_NOTE_WORDS: usize = 120;

const STAMP: &str = r"\[checked: (?:schemas|v|candler-evaluator)@\d+\.\d+\.\d+[^\]\s]*\]";

/// Checks the five sections and their order, then the Notes.
pub fn check_frame(text: &str) -> Result<(), String> {
    let headings = level_two_headings(text);
    if headings != SECTIONS {
        return Err(format!(
            "the `##` sections are {headings:?}, they must be exactly {SECTIONS:?}"
        ));
    }
    check_notes(&section(text, "Notes").unwrap_or_default())
}

/// Every note is a bullet that carries a stamp `[checked: <line>@<version>]`, and the words of the
/// notes stay within [`MAX_NOTE_WORDS`].
pub fn check_notes(notes: &str) -> Result<(), String> {
    let stamp = Regex::new(STAMP).map_err(|e| e.to_string())?;
    let mut bullets: Vec<String> = Vec::new();
    for line in notes.lines().filter(|line| !line.trim().is_empty()) {
        if let Some(first) = line.strip_prefix("- ") {
            bullets.push(first.to_owned());
        } else if line.starts_with(' ') || line.starts_with('\t') {
            match bullets.last_mut() {
                Some(last) => {
                    last.push(' ');
                    last.push_str(line.trim());
                }
                None => return Err(format!("a note is not a bullet: `{line}`")),
            }
        } else {
            return Err(format!("a note is not a bullet: `{line}`"));
        }
    }
    let mut words = 0;
    for bullet in &bullets {
        if !stamp.is_match(bullet) {
            return Err(format!(
                "a note has no `[checked: <line>@<version>]` stamp: `{bullet}`"
            ));
        }
        words += stamp.replace_all(bullet, "").split_whitespace().count();
    }
    if words > MAX_NOTE_WORDS {
        return Err(format!(
            "the Notes hold {words} words, at most {MAX_NOTE_WORDS}"
        ));
    }
    Ok(())
}
