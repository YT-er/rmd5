use std::path::{Path, PathBuf};

pub fn format_digest_line(hex: &str, path: &Path, binary: bool) -> String {
    let name = path_to_display(path);
    let escaped = needs_escape(&name);
    let mut line = String::new();
    if escaped {
        line.push('\\');
    }
    line.push_str(hex);
    line.push(' ');
    line.push(if binary { '*' } else { ' ' });
    if escaped {
        line.push_str(&escape_name(&name));
    } else {
        line.push_str(&name);
    }
    line
}

pub fn path_to_display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn parse_check_line(line: &str) -> Option<CheckLine> {
    if line.is_empty() {
        return None;
    }

    let (escaped, rest) = if let Some(stripped) = line.strip_prefix('\\') {
        (true, stripped)
    } else {
        (false, line)
    };

    if rest.len() < 35 {
        return None;
    }

    let digest = &rest[..32];
    let bytes = rest.as_bytes();
    if bytes[32] != b' ' || (bytes[33] != b' ' && bytes[33] != b'*') {
        return None;
    }

    let raw_name = &rest[34..];
    let name = if escaped {
        unescape_name(raw_name)?
    } else {
        raw_name.to_string()
    };

    Some(CheckLine {
        digest: digest.to_string(),
        path: PathBuf::from(name),
    })
}

pub struct CheckLine {
    pub digest: String,
    pub path: PathBuf,
}

fn needs_escape(name: &str) -> bool {
    name.bytes().any(|b| b == b'\\' || b == b'\n')
}

fn escape_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn unescape_name(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next()? {
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{format_digest_line, parse_check_line};
    use std::path::Path;

    #[test]
    fn round_trips_escaped_name() {
        let line = format_digest_line(
            "d41d8cd98f00b204e9800998ecf8427e",
            Path::new("a\\b\nc"),
            false,
        );
        let parsed = parse_check_line(&line).expect("check line");
        assert_eq!(parsed.digest, "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(parsed.path, Path::new("a\\b\nc"));
    }
}
