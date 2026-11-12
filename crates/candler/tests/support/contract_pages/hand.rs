//! The hand-written text of a page: the generator copies it from the file and writes it back.

/// The text under a `## <heading>` line, up to the next `## ` line, trimmed; `None` when the file
/// has no such section. Lines inside a code fence are never headings.
pub fn section(text: &str, heading: &str) -> Option<String> {
    let wanted = format!("## {heading}");
    let mut fenced = false;
    let mut inside = false;
    let mut found = false;
    let mut body = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && line.starts_with("## ") {
            if inside {
                break;
            }
            if line.trim_end() == wanted {
                inside = true;
                found = true;
            }
            continue;
        }
        if inside {
            body.push_str(line);
            body.push('\n');
        }
    }
    found.then(|| body.trim().to_owned())
}

/// The `## ` headings of the text in order, ignoring lines inside a code fence.
pub fn level_two_headings(text: &str) -> Vec<String> {
    let mut fenced = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && let Some(heading) = line.strip_prefix("## ") {
            out.push(heading.trim().to_owned());
        }
    }
    out
}
