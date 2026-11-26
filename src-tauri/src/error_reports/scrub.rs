//! Scrubbing (D3): what makes a log line safe to put in a report.

use std::path::PathBuf;

/// Rewrites a log line so it can go into a report.
///
/// The log tail is the one field that could smuggle in something a report may
/// never contain: log lines carry recording directory paths, which embed both
/// the home directory and the guide title, and server error bodies, which can
/// carry a token. Every rule here removes one of those.
///
/// Scrubbing runs at report creation, before the file is written, so the copy
/// on disk is already clean and the content the dialog shows is exactly the
/// content that is sent.
///
/// What deliberately survives: cursor coordinates, the audio device name, and
/// HTTP status lines. They are not personal data and they are frequently the
/// diagnosis.
///
/// Adding a log line that names a file means adding a case here and a test for
/// it. That is the review rule, not a suggestion -- a line whose shape no rule
/// matches goes out verbatim.
pub struct Scrubber {
    home: Option<String>,
    output_dir: Option<String>,
}

impl Scrubber {
    pub fn new(home: Option<PathBuf>, output_dir: Option<String>) -> Self {
        Self {
            home: home
                .map(|p| {
                    p.to_string_lossy()
                        .trim_end_matches(['/', '\\'])
                        .to_string()
                })
                .filter(|s| !s.is_empty()),
            output_dir: output_dir
                .map(|s| s.trim_end_matches(['/', '\\']).to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    pub fn scrub(&self, line: &str) -> String {
        // Output directory first: it usually sits under the home directory, so
        // rewriting the home prefix first would leave nothing for this rule to
        // match and the guide title would survive.
        let mut out = self.scrub_output_dir(line);
        if let Some(home) = &self.home {
            out = out.replace(home, "~");
        }
        out = scrub_emails(&out);
        out = scrub_secrets(&out);
        out
    }

    pub fn scrub_all(&self, lines: &[String]) -> Vec<String> {
        lines.iter().map(|l| self.scrub(l)).collect()
    }

    /// `<output dir>/<recording>/...` becomes
    /// `<Anleitungsverzeichnis>/<Aufnahme>/...`. The path component after the
    /// output directory is the recording folder, and its name is the guide
    /// title the user typed.
    fn scrub_output_dir(&self, line: &str) -> String {
        let Some(dir) = &self.output_dir else {
            return line.to_string();
        };
        let mut out = String::with_capacity(line.len());
        let mut rest = line;
        while let Some(at) = rest.find(dir.as_str()) {
            out.push_str(&rest[..at]);
            out.push_str("<Anleitungsverzeichnis>");
            let after = &rest[at + dir.len()..];
            let mut chars = after.char_indices();
            match chars.next() {
                // A separator means a path component follows, and that
                // component is the recording name.
                Some((_, sep)) if sep == '/' || sep == '\\' => {
                    out.push(sep);
                    let tail = &after[sep.len_utf8()..];
                    let end = tail.find(['/', '\\', ' ', '"', '\'']).unwrap_or(tail.len());
                    if end > 0 {
                        out.push_str("<Aufnahme>");
                    }
                    rest = &tail[end..];
                }
                _ => rest = after,
            }
        }
        out.push_str(rest);
        out
    }
}

/// Anything shaped like an address. Kept deliberately loose: a false positive
/// costs a placeholder in a log line, a false negative costs an address.
fn scrub_emails(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for token in split_keeping_separators(line) {
        if looks_like_email(token) {
            out.push_str("<E-Mail>");
        } else {
            out.push_str(token);
        }
    }
    out
}

fn looks_like_email(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_alphanumeric());
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

fn split_keeping_separators(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in line.char_indices() {
        if c.is_whitespace() || c == '"' || c == ',' || c == '<' || c == '>' {
            if i > start {
                parts.push(&line[start..i]);
            }
            parts.push(&line[i..i + c.len_utf8()]);
            start = i + c.len_utf8();
        }
    }
    if start < line.len() {
        parts.push(&line[start..]);
    }
    parts
}

/// `Bearer <token>`, and the values of the JSON keys that carry credentials.
fn scrub_secrets(line: &str) -> String {
    let mut out = replace_after_marker(line, "Bearer ", |c| c.is_whitespace() || c == '"');
    for key in ["access_token", "refresh_token", "api_key"] {
        for marker in [format!("\"{key}\":\""), format!("\"{key}\": \"")] {
            out = replace_after_marker(&out, &marker, |c| c == '"');
        }
    }
    out
}

/// Replace everything between `marker` and the first character `ends` accepts
/// with `<entfernt>`, for every occurrence.
fn replace_after_marker(line: &str, marker: &str, ends: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(marker) {
        out.push_str(&rest[..at + marker.len()]);
        let value = &rest[at + marker.len()..];
        let end = value.find(&ends).unwrap_or(value.len());
        if end > 0 {
            out.push_str("<entfernt>");
        }
        rest = &value[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
