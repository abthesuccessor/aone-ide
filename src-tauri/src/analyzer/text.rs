use tree_sitter::Node;

pub(super) const MAX_LABEL_BYTES: usize = 240;
const MAX_COMPACT_INPUT_BYTES: usize = 4_096;

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

pub(super) fn compact_text(node: Node<'_>, source: &str) -> String {
    let text = node_text(node, source);
    let mut input_end = text.len().min(MAX_COMPACT_INPUT_BYTES);
    while !text.is_char_boundary(input_end) {
        input_end -= 1;
    }
    let mut output = String::with_capacity(MAX_LABEL_BYTES + '…'.len_utf8());
    let mut pending_space = false;
    let mut truncated = input_end < text.len();

    for character in text[..input_end].chars() {
        if character.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        let required = character.len_utf8() + usize::from(pending_space);
        if output.len().saturating_add(required) > MAX_LABEL_BYTES {
            truncated = true;
            break;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }

    if truncated && !output.is_empty() {
        output.push('…');
    }
    output
}
