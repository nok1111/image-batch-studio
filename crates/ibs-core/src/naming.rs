//! Output file name templates for the Bulk Renamer.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RenameSpec {
    /// Tokens: `{name}` (after find/replace), `{n}` / `{n:03}` (counter,
    /// zero-padded), `{date}` (file modified date, YYYY-MM-DD, UTC).
    pub template: String,
    pub prefix: String,
    pub suffix: String,
    /// Text removed from or replaced in the original name; empty = none.
    pub find: String,
    pub replace: String,
    /// Counter value of the first input.
    pub start: u32,
}

enum Token {
    Text(String),
    Name,
    Counter(usize),
    Date,
}

const MAX_PADDING: usize = 9;

fn parse(template: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if open > 0 {
            out.push(Token::Text(rest[..open].into()));
        }
        let close = rest[open..]
            .find('}')
            .ok_or_else(|| format!("unclosed `{{` in template `{template}`"))?
            + open;
        let tag = &rest[open + 1..close];
        out.push(match tag {
            "name" => Token::Name,
            "date" => Token::Date,
            "n" => Token::Counter(0),
            _ => match tag.strip_prefix("n:") {
                Some(w) if w.len() <= 2 && w.chars().all(|c| c.is_ascii_digit()) => {
                    let w: usize = w.parse().map_err(|_| format!("bad counter `{tag}`"))?;
                    Token::Counter(w.min(MAX_PADDING))
                }
                _ => return Err(format!("unknown token `{{{tag}}}`")),
            },
        });
        rest = &rest[close + 1..];
    }
    if !rest.is_empty() {
        out.push(Token::Text(rest.into()));
    }
    Ok(out)
}

impl RenameSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.template.trim().is_empty() {
            return Err("rename template is empty".into());
        }
        parse(&self.template).map(|_| ())
    }

    /// File stem (without extension) for the input at `index`.
    pub fn render(&self, stem: &str, index: u32, modified: Option<SystemTime>) -> String {
        let Ok(tokens) = parse(&self.template) else {
            return sanitize(stem);
        };
        let name = if self.find.is_empty() {
            stem.to_string()
        } else {
            stem.replace(&self.find, &self.replace)
        };
        let n = u64::from(self.start) + u64::from(index);
        let mut out = self.prefix.clone();
        for t in tokens {
            match t {
                Token::Text(s) => out.push_str(&s),
                Token::Name => out.push_str(&name),
                Token::Counter(w) => out.push_str(&format!("{n:0w$}")),
                Token::Date => out.push_str(&modified.map(utc_date).unwrap_or_default()),
            }
        }
        out.push_str(&self.suffix);
        sanitize(&out)
    }
}

fn utc_date(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Makes a stem valid on Windows, macOS and Linux.
pub fn sanitize(stem: &str) -> String {
    let mut s: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    while s.ends_with(['.', ' ']) {
        s.pop();
    }
    let s = s.trim_start().to_string();
    if s.is_empty() {
        return "image".into();
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&base.as_str()) {
        format!("_{s}")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn spec(template: &str) -> RenameSpec {
        RenameSpec {
            template: template.into(),
            prefix: String::new(),
            suffix: String::new(),
            find: String::new(),
            replace: String::new(),
            start: 1,
        }
    }

    #[test]
    fn renders_tokens() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let s = RenameSpec {
            prefix: "web_".into(),
            suffix: "_final".into(),
            find: "IMG".into(),
            replace: "photo".into(),
            start: 7,
            ..spec("{name}_{n:03}_{date}")
        };
        assert_eq!(
            s.render("IMG_42", 2, Some(t)),
            "web_photo_42_009_2023-11-14_final"
        );
        assert_eq!(spec("{n}").render("x", 0, None), "1");
        assert_eq!(spec("a{date}b").render("x", 0, None), "ab");
    }

    #[test]
    fn rejects_bad_templates() {
        assert!(spec("{nme}").validate().is_err());
        assert!(spec("{name").validate().is_err());
        assert!(spec("  ").validate().is_err());
        assert!(spec("{n:123}").validate().is_err());
        assert!(spec("{name}-{n:4}").validate().is_ok());
    }

    #[test]
    fn sanitizes_for_every_os() {
        assert_eq!(sanitize("a/b\\c:d*?"), "a_b_c_d__");
        assert_eq!(sanitize("name. . "), "name");
        assert_eq!(sanitize("con"), "_con");
        assert_eq!(sanitize(""), "image");
        assert_eq!(sanitize("..."), "image");
    }
}
