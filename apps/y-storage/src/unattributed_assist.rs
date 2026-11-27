use std::fmt::Write as _;

/// Normalizes slashes and surrounding whitespace/quotes while preserving drive root (`C:\`).
#[must_use]
pub fn normalize_unattributed_target_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim().replace('/', "\\");
    if trimmed.len() == 3
        && trimmed.as_bytes()[0].is_ascii_alphabetic()
        && trimmed.as_bytes()[1] == b':'
        && trimmed.as_bytes()[2] == b'\\'
    {
        return trimmed;
    }
    let stripped = trimmed.trim_end_matches('\\');
    if stripped.len() == 2
        && stripped.as_bytes()[0].is_ascii_alphabetic()
        && stripped.as_bytes()[1] == b':'
    {
        return format!("{stripped}\\");
    }
    stripped.to_string()
}

/// Builds the exact raw Windows argument string passed to `explorer.exe` (`/select,"<full path>"`).
#[must_use]
pub fn build_explorer_select_raw_arg(path: &str) -> String {
    let normalized = normalize_unattributed_target_path(path);
    format!("/select,\"{normalized}\"")
}

/// Human-readable command line shown on `Locate` hover tooltips.
#[must_use]
pub fn build_explorer_select_command_line(path: &str) -> String {
    format!("explorer.exe {}", build_explorer_select_raw_arg(path))
}

fn sanitize_query_token(segment: &str) -> String {
    segment
        .chars()
        .map(|ch| if ch == '"' || ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_drive_root_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Constructs a natural search query (`"what is the {folder} folder in {parent}"`, or
/// `"what is the {folder} folder"` at a drive root) from an unattributed folder path.
#[must_use]
pub fn build_unattributed_search_query(path: &str) -> String {
    let normalized = normalize_unattributed_target_path(path);
    let segments: Vec<&str> = normalized
        .split('\\')
        .filter(|seg| !seg.is_empty() && !is_drive_root_segment(seg))
        .collect();

    let name = segments
        .last()
        .map(|s| sanitize_query_token(s))
        .unwrap_or_default();
    let parent = if segments.len() >= 2 {
        sanitize_query_token(segments[segments.len() - 2])
    } else {
        String::new()
    };

    if !name.is_empty() && !parent.is_empty() {
        format!("what is the {name} folder in {parent}")
    } else if !name.is_empty() {
        format!("what is the {name} folder")
    } else {
        "what is this folder".to_string()
    }
}

/// RFC-3986 UTF-8 byte percent encoding safe for search query values.
#[must_use]
pub fn percent_encode_rfc3986(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Constructs the `(query, url)` pair targeting Google Search without performing any network request.
#[must_use]
pub fn build_unattributed_search_url(path: &str) -> (String, String) {
    let query = build_unattributed_search_query(path);
    let encoded = percent_encode_rfc3986(&query);
    let url = format!("https://www.google.com/search?q={encoded}");
    (query, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_uses_natural_question_template_without_quote_fragments() {
        assert_eq!(
            build_unattributed_search_query(r"C:\Users\Win\AppData\Local\Firaxis Games"),
            "what is the Firaxis Games folder in Local"
        );
        assert_eq!(
            build_unattributed_search_query(
                r"C:\Users\Win\AppData\Local\Microsoft\onecoremsvsmon"
            ),
            "what is the onecoremsvsmon folder in Microsoft"
        );
        assert_eq!(
            build_unattributed_search_query(r"C:\Users\Win\onecoremsvsmon\"),
            "what is the onecoremsvsmon folder in Win"
        );
        assert_eq!(
            build_unattributed_search_query(r"C:\SWSetup"),
            "what is the SWSetup folder"
        );
        assert_eq!(
            build_unattributed_search_query(r"D:\Media"),
            "what is the Media folder"
        );
        assert!(!build_unattributed_search_query(r"D:\Media").contains('"'));
    }

    #[test]
    fn search_query_and_url_handle_special_characters_safely() {
        let path = r#"C:\ProgramData\Vendor & Co\C++ #1 (100%)"#;
        let (query, url) = build_unattributed_search_url(path);
        assert_eq!(
            query,
            "what is the C++ #1 (100%) folder in Vendor & Co"
        );
        assert!(!query.contains('"'));
        assert!(url.starts_with("https://www.google.com/search?q=what%20is%20the%20"));
        assert!(url.contains("%2B%2B"));
        assert!(url.contains("%231"));
        assert!(url.contains("%25"));
        assert!(url.contains("%26"));
        assert!(
            !url["https://www.google.com/search?q=".len()..].contains('&'),
            "ampersands inside folder names must be percent-encoded: {url}"
        );
        assert!(
            !url.contains('#'),
            "hash characters must be percent-encoded so they do not truncate as URL fragments: {url}"
        );

        let quoted_name = r#"C:\Cache\Weird"Sub\Folder"Name"#;
        assert_eq!(
            build_unattributed_search_query(quoted_name),
            "what is the Folder Name folder in Weird Sub"
        );
    }

    #[test]
    fn locate_command_quotes_paths_with_spaces_commas_and_unicode() {
        let complex = r"C:\Program Files (x86)\Acme, Inc. & Sons\Cache Data\";
        assert_eq!(
            build_explorer_select_raw_arg(complex),
            "/select,\"C:\\Program Files (x86)\\Acme, Inc. & Sons\\Cache Data\""
        );
        assert_eq!(
            build_explorer_select_command_line(complex),
            "explorer.exe /select,\"C:\\Program Files (x86)\\Acme, Inc. & Sons\\Cache Data\""
        );
        assert_eq!(
            build_explorer_select_raw_arg("  \"D:/Tools/日本語 フォルダ/\"  "),
            "/select,\"D:\\Tools\\日本語 フォルダ\""
        );
        assert_eq!(
            build_explorer_select_raw_arg(r"C:\"),
            "/select,\"C:\\\""
        );
    }
}
