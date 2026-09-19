use crate::domain::{WorkspaceSearchMatch, WorkspaceSearchMatchKind};

use super::limits::{MAX_MATCH_TEXT_CHARS, MAX_PREVIEW_CHARS};

pub(super) fn literal_line_matches(
    relative_path: &str,
    source: &str,
    query: &str,
    maximum: usize,
) -> Vec<WorkspaceSearchMatch> {
    let mut matches = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        for (byte_start, byte_end) in ascii_insensitive_ranges(line, query) {
            let start_column = line[..byte_start].encode_utf16().count() + 1;
            let end_column = line[..byte_end].encode_utf16().count() + 1;
            let match_text = bounded_chars(&line[byte_start..byte_end], MAX_MATCH_TEXT_CHARS);
            matches.push(WorkspaceSearchMatch {
                key: match_key(
                    WorkspaceSearchMatchKind::Content,
                    relative_path,
                    line_index + 1,
                    start_column,
                    matches.len(),
                ),
                relative_path: relative_path.into(),
                start_line: line_index + 1,
                start_column,
                end_line: line_index + 1,
                end_column,
                preview: preview_around(line, byte_start, byte_end),
                match_text,
                kind: WorkspaceSearchMatchKind::Content,
            });
            if matches.len() == maximum {
                return matches;
            }
        }
    }
    matches
}

pub(super) fn path_match(relative_path: &str, query: &str) -> Option<WorkspaceSearchMatch> {
    let (byte_start, byte_end) = first_literal_range(relative_path, query)?;
    Some(WorkspaceSearchMatch {
        key: match_key(WorkspaceSearchMatchKind::Path, relative_path, 1, 1, 0),
        relative_path: relative_path.into(),
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 1,
        preview: bounded_chars(relative_path, MAX_PREVIEW_CHARS),
        match_text: bounded_chars(&relative_path[byte_start..byte_end], MAX_MATCH_TEXT_CHARS),
        kind: WorkspaceSearchMatchKind::Path,
    })
}

pub(super) fn match_key(
    kind: WorkspaceSearchMatchKind,
    relative_path: &str,
    line: usize,
    column: usize,
    ordinal: usize,
) -> String {
    let kind = match kind {
        WorkspaceSearchMatchKind::Content => "content",
        WorkspaceSearchMatchKind::Path => "path",
        WorkspaceSearchMatchKind::Symbol => "symbol",
        WorkspaceSearchMatchKind::Endpoint => "endpoint",
        WorkspaceSearchMatchKind::Event => "event",
        WorkspaceSearchMatchKind::Heading => "heading",
        WorkspaceSearchMatchKind::Sentence => "sentence",
    };
    format!("{kind}:{relative_path}:{line}:{column}:{ordinal}")
}

pub(super) fn preview_line(source: &str, one_based_line: usize) -> String {
    source
        .lines()
        .nth(one_based_line.saturating_sub(1))
        .map(|line| bounded_chars(line, MAX_PREVIEW_CHARS))
        .unwrap_or_default()
}

pub(super) fn bounded_match_text(value: &str) -> String {
    bounded_chars(value, MAX_MATCH_TEXT_CHARS)
}

pub(super) fn first_literal_range(value: &str, query: &str) -> Option<(usize, usize)> {
    ascii_insensitive_ranges(value, query).next()
}

fn ascii_insensitive_ranges(value: &str, query: &str) -> std::vec::IntoIter<(usize, usize)> {
    let folded_value = value.to_ascii_lowercase();
    let folded_query = query.to_ascii_lowercase();
    folded_value
        .match_indices(&folded_query)
        .map(|(start, matched)| (start, start + matched.len()))
        .collect::<Vec<_>>()
        .into_iter()
}

fn preview_around(line: &str, byte_start: usize, byte_end: usize) -> String {
    let characters = line.chars().collect::<Vec<_>>();
    if characters.len() <= MAX_PREVIEW_CHARS {
        return line.into();
    }
    let match_start = line[..byte_start].chars().count();
    let match_end = line[..byte_end].chars().count();
    let match_length = match_end.saturating_sub(match_start);
    let available_context = MAX_PREVIEW_CHARS
        .saturating_sub(match_length)
        .saturating_sub(2);
    let before = available_context / 2;
    let mut window_start = match_start.saturating_sub(before);
    let window_end = (window_start + MAX_PREVIEW_CHARS.saturating_sub(2)).min(characters.len());
    if window_end == characters.len() {
        window_start = window_end.saturating_sub(MAX_PREVIEW_CHARS.saturating_sub(2));
    }
    let mut preview = String::new();
    if window_start > 0 {
        preview.push('…');
    }
    preview.extend(characters[window_start..window_end].iter());
    if window_end < characters.len() {
        preview.push('…');
    }
    preview
}

fn bounded_chars(value: &str, maximum: usize) -> String {
    let mut characters = value.chars();
    let bounded = characters.by_ref().take(maximum).collect::<String>();
    if characters.next().is_some() {
        format!(
            "{}…",
            bounded
                .chars()
                .take(maximum.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        bounded
    }
}

#[cfg(test)]
mod tests {
    use super::{first_literal_range, literal_line_matches, path_match};

    #[test]
    fn literal_matches_are_case_insensitive_and_one_based() {
        let matches = literal_line_matches("src/lib.rs", "first\nListenReady();\n", "listen", 10);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_line, 2);
        assert_eq!(matches[0].start_column, 1);
        assert_eq!(matches[0].end_column, 7);
        assert_eq!(matches[0].match_text, "Listen");
    }

    #[test]
    fn path_match_jumps_to_the_file_start() {
        let found = path_match("src/userService.ts", "service").unwrap();
        assert_eq!(found.start_line, 1);
        assert_eq!(found.start_column, 1);
        assert_eq!(found.match_text, "Service");
    }

    #[test]
    fn non_ascii_matching_keeps_exact_case() {
        assert!(literal_line_matches("notes.md", "Tokyo 東京", "東京", 10).len() == 1);
        assert!(literal_line_matches("notes.md", "Éclair", "éclair", 10).is_empty());
        assert!(first_literal_range("Äpfel", "äpfel").is_none());
        assert_eq!(first_literal_range("Äpfel", "ÄPFEL"), Some((0, 6)));
    }

    #[test]
    fn literal_columns_use_utf16_units_before_and_inside_astral_matches() {
        let source = "😀東京 target 😀target";
        let after_emoji = literal_line_matches("notes.md", source, "target", 10);
        assert_eq!(
            (after_emoji[0].start_column, after_emoji[0].end_column),
            (6, 12)
        );

        let bearing_emoji = literal_line_matches("notes.md", source, "😀target", 10);
        assert_eq!(
            (bearing_emoji[0].start_column, bearing_emoji[0].end_column),
            (13, 21)
        );
    }
}
