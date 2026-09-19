use std::path::Path;

pub(super) const MAX_DOCUMENT_LINES_PER_FILE: usize = 250_000;

#[derive(Clone, Copy)]
pub(super) enum DocumentFormat {
    Markdown,
    PlainText,
    ReStructuredText,
    AsciiDoc,
}

impl DocumentFormat {
    pub(super) fn from_path(path: &str) -> Self {
        match Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "md" | "mdx" => Self::Markdown,
            "rst" => Self::ReStructuredText,
            "adoc" | "asciidoc" => Self::AsciiDoc,
            _ => Self::PlainText,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::PlainText => "plainText",
            Self::ReStructuredText => "reStructuredText",
            Self::AsciiDoc => "asciiDoc",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct LineSpan {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) excluded: bool,
}

#[derive(Clone)]
pub(super) struct HeadingSpan {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) level: usize,
    pub(super) style: &'static str,
}

pub(super) fn line_spans(source: &str) -> (Vec<LineSpan>, bool) {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut truncated = false;
    for part in source.split_inclusive('\n') {
        if lines.len() >= MAX_DOCUMENT_LINES_PER_FILE {
            truncated = true;
            break;
        }
        let raw_end = start + part.len();
        let mut end = raw_end.saturating_sub(usize::from(part.ends_with('\n')));
        if source
            .get(start..end)
            .is_some_and(|line| line.ends_with('\r'))
        {
            end = end.saturating_sub(1);
        }
        lines.push(LineSpan {
            start,
            end,
            excluded: false,
        });
        start = raw_end;
    }
    if source.is_empty() {
        lines.push(LineSpan {
            start: 0,
            end: 0,
            excluded: false,
        });
    }
    (lines, truncated)
}

pub(super) fn mark_fenced_regions(source: &str, lines: &mut [LineSpan], format: DocumentFormat) {
    match format {
        DocumentFormat::Markdown | DocumentFormat::PlainText | DocumentFormat::ReStructuredText => {
            mark_markdown_fences(source, lines)
        }
        DocumentFormat::AsciiDoc => mark_asciidoc_fences(source, lines),
    }
}

fn mark_markdown_fences(source: &str, lines: &mut [LineSpan]) {
    let mut active = None::<(u8, usize)>;
    for line in lines {
        let text = &source[line.start..line.end];
        if let Some((marker, length)) = active {
            line.excluded = true;
            if markdown_fence(text).is_some_and(|candidate| {
                candidate.0 == marker && candidate.1 >= length && candidate.2
            }) {
                active = None;
            }
        } else if let Some((marker, length, _)) = markdown_fence(text) {
            line.excluded = true;
            active = Some((marker, length));
        }
    }
}

fn markdown_fence(line: &str) -> Option<(u8, usize, bool)> {
    let bytes = line.as_bytes();
    let indent = bytes.iter().take_while(|byte| **byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let marker = *bytes.get(indent)?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = bytes[indent..]
        .iter()
        .take_while(|byte| **byte == marker)
        .count();
    if length < 3 {
        return None;
    }
    let closing = bytes[indent + length..]
        .iter()
        .all(|byte| byte.is_ascii_whitespace());
    Some((marker, length, closing))
}

fn mark_asciidoc_fences(source: &str, lines: &mut [LineSpan]) {
    let mut pending_source = false;
    let mut delimiter = None::<String>;
    for line in lines {
        let text = source[line.start..line.end].trim();
        if let Some(active) = delimiter.as_deref() {
            line.excluded = true;
            if text == active {
                delimiter = None;
            }
            continue;
        }
        if text.starts_with("[source") && text.ends_with(']') {
            line.excluded = true;
            pending_source = true;
            continue;
        }
        if pending_source && matches!(text, "----" | "....") {
            line.excluded = true;
            delimiter = Some(text.to_string());
            pending_source = false;
        } else if !text.is_empty() {
            pending_source = false;
        }
    }
}

pub(super) fn detect_headings(
    source: &str,
    lines: &mut [LineSpan],
    format: DocumentFormat,
) -> Vec<Option<HeadingSpan>> {
    let mut headings = lines
        .iter()
        .map(|line| {
            if line.excluded {
                None
            } else {
                direct_heading(source, *line, format)
            }
        })
        .collect::<Vec<_>>();

    if matches!(
        format,
        DocumentFormat::Markdown | DocumentFormat::ReStructuredText
    ) {
        for index in 0..lines.len() {
            if lines[index].excluded {
                continue;
            }
            let text = &source[lines[index].start..lines[index].end];
            let Some(level) = underline_heading_level(text, format) else {
                continue;
            };
            lines[index].excluded = true;
            let Some(previous) = index.checked_sub(1) else {
                continue;
            };
            if lines[previous].excluded || headings[previous].is_some() {
                continue;
            }
            if let Some((start, end)) =
                trimmed_span(source, lines[previous].start, lines[previous].end)
            {
                headings[previous] = Some(HeadingSpan {
                    start,
                    end,
                    level,
                    style: if matches!(format, DocumentFormat::Markdown) {
                        "setext"
                    } else {
                        "reStructuredTextUnderline"
                    },
                });
            }
        }
    }
    headings
}

fn direct_heading(source: &str, line: LineSpan, format: DocumentFormat) -> Option<HeadingSpan> {
    let raw = &source[line.start..line.end];
    let leading = raw.len().saturating_sub(raw.trim_start().len());
    let marker = match format {
        DocumentFormat::Markdown => b'#',
        DocumentFormat::AsciiDoc => b'=',
        DocumentFormat::PlainText | DocumentFormat::ReStructuredText => return None,
    };
    if matches!(format, DocumentFormat::Markdown) && leading > 3 {
        return None;
    }
    let bytes = raw.as_bytes();
    let level = bytes[leading..]
        .iter()
        .take_while(|byte| **byte == marker)
        .count();
    if !(1..=6).contains(&level)
        || bytes
            .get(leading + level)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
    {
        return None;
    }
    let (start, mut end) = trimmed_span(source, line.start + leading + level, line.end)?;
    if matches!(format, DocumentFormat::Markdown) {
        let body = &source[start..end];
        let without_hashes = body.trim_end_matches('#');
        if without_hashes.len() < body.len()
            && without_hashes
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        {
            end = start + without_hashes.trim_end().len();
        }
    }
    (start < end).then_some(HeadingSpan {
        start,
        end,
        level,
        style: if matches!(format, DocumentFormat::Markdown) {
            "atx"
        } else {
            "asciiDoc"
        },
    })
}

fn underline_heading_level(line: &str, format: DocumentFormat) -> Option<usize> {
    let trimmed = line.trim();
    let marker = trimmed.as_bytes().first().copied()?;
    if trimmed.len() < 3 || !trimmed.as_bytes().iter().all(|byte| *byte == marker) {
        return None;
    }
    match format {
        DocumentFormat::Markdown => match marker {
            b'=' => Some(1),
            b'-' => Some(2),
            _ => None,
        },
        DocumentFormat::ReStructuredText => match marker {
            b'=' => Some(1),
            b'-' => Some(2),
            b'~' => Some(3),
            b'^' => Some(4),
            b'"' | b'\'' | b'`' | b':' | b'.' | b'_' | b'*' | b'+' | b'#' => Some(5),
            _ => None,
        },
        DocumentFormat::PlainText | DocumentFormat::AsciiDoc => None,
    }
}

fn trimmed_span(source: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let value = source.get(start..end)?;
    let trimmed_start = value.trim_start();
    let adjusted_start = start + value.len().saturating_sub(trimmed_start.len());
    let trimmed = trimmed_start.trim_end();
    let adjusted_end = adjusted_start + trimmed.len();
    (adjusted_start < adjusted_end).then_some((adjusted_start, adjusted_end))
}
