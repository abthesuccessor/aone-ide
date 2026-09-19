use std::{collections::BTreeSet, io::Write};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::runner::{is_sensitive_name, redact_text};

const REDACTED: &[u8] = b"[REDACTED]";
const CONTENT_HIDDEN: &str = "[CONTENT HIDDEN: REDACTION LIMIT]";
pub(super) const MAX_SECRET_PATTERNS: usize = 256;
pub(super) const MAX_SECRET_PATTERN_BYTES: usize = 256 * 1024;

pub(super) struct SecretRedactor {
    matcher: Option<AhoCorasick>,
    fail_closed: bool,
}

impl SecretRedactor {
    pub(super) fn from_values(values: &[Zeroizing<String>]) -> Self {
        Self::from_iter(values.iter().map(|value| value.as_str()))
    }

    fn from_iter<'a>(values: impl Iterator<Item = &'a str>) -> Self {
        let mut patterns = BTreeSet::new();
        let mut bytes = 0_usize;
        for value in values.filter(|value| !value.is_empty()) {
            if patterns.contains(value) {
                continue;
            }
            bytes = match bytes.checked_add(value.len()) {
                Some(bytes) => bytes,
                None => return Self::fail_closed(),
            };
            if patterns.len() >= MAX_SECRET_PATTERNS || bytes > MAX_SECRET_PATTERN_BYTES {
                return Self::fail_closed();
            }
            patterns.insert(value.to_owned());
        }
        if patterns.is_empty() {
            return Self {
                matcher: None,
                fail_closed: false,
            };
        }
        let matcher = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns)
            .ok();
        Self {
            fail_closed: matcher.is_none(),
            matcher,
        }
    }

    fn fail_closed() -> Self {
        Self {
            matcher: None,
            fail_closed: true,
        }
    }

    pub(super) fn text_preview(&self, value: &str, max_bytes: usize) -> (String, bool) {
        if self.fail_closed {
            return bounded_text(CONTENT_HIDDEN, max_bytes, true);
        }
        let (exact_safe, mut truncated) = self.replace_text(value, max_bytes);
        let assignment_safe = redact_text(&exact_safe, &[]);
        let (assignment_safe, assignment_truncated) =
            bounded_text(&assignment_safe, max_bytes, false);
        truncated |= assignment_truncated;

        if let Ok(mut json) = serde_json::from_str::<Value>(&assignment_safe) {
            let nested_truncated = self.redact_json_value(&mut json, max_bytes);
            let mut output = BoundedBuffer::new(max_bytes);
            if serde_json::to_writer(&mut output, &json).is_err() {
                return bounded_text(CONTENT_HIDDEN, max_bytes, true);
            }
            output.truncated |= truncated || nested_truncated;
            return output.into_text();
        }

        // A known value can be a JSON key or a non-string scalar. Replacing it
        // before parsing is required, but a scalar replacement may make the
        // document invalid JSON. If the original was structured JSON, hide the
        // preview rather than bypassing sensitive-field traversal.
        if serde_json::from_str::<Value>(value).is_ok() {
            return bounded_text(CONTENT_HIDDEN, max_bytes, true);
        }
        (assignment_safe, truncated)
    }

    pub(super) fn binary_preview(&self, value: &[u8], max_bytes: usize) -> (Vec<u8>, bool) {
        if self.fail_closed {
            let mut output = BoundedBuffer::new(max_bytes);
            output.push(CONTENT_HIDDEN.as_bytes());
            output.truncated = true;
            return (output.bytes, true);
        }
        self.replace_bytes(value, max_bytes)
    }

    fn redact_json_value(&self, value: &mut Value, max_bytes: usize) -> bool {
        match value {
            Value::Object(fields) => {
                let mut truncated = false;
                for (name, value) in fields {
                    if is_sensitive_name(name) {
                        *value = Value::String("[REDACTED]".into());
                    } else {
                        truncated |= self.redact_json_value(value, max_bytes);
                    }
                }
                truncated
            }
            Value::Array(values) => {
                let mut truncated = false;
                for value in values {
                    truncated |= self.redact_json_value(value, max_bytes);
                }
                truncated
            }
            Value::String(text) => {
                let (redacted, truncated) = self.replace_text(text, max_bytes);
                *text = redacted;
                truncated
            }
            _ => false,
        }
    }

    fn replace_text(&self, value: &str, max_bytes: usize) -> (String, bool) {
        let (bytes, truncated) = self.replace_bytes(value.as_bytes(), max_bytes);
        let mut bytes = bytes;
        while std::str::from_utf8(&bytes).is_err() {
            bytes.pop();
        }
        (String::from_utf8(bytes).unwrap_or_default(), truncated)
    }

    fn replace_bytes(&self, value: &[u8], max_bytes: usize) -> (Vec<u8>, bool) {
        let mut output = BoundedBuffer::new(max_bytes);
        let Some(matcher) = &self.matcher else {
            output.push(value);
            return (output.bytes, output.truncated);
        };
        let mut offset = 0_usize;
        for matched in matcher.find_iter(value) {
            if !output.push(&value[offset..matched.start()]) || !output.push(REDACTED) {
                break;
            }
            offset = matched.end();
        }
        if !output.truncated {
            output.push(&value[offset..]);
        }
        (output.bytes, output.truncated)
    }
}

pub(super) struct RedactionCache {
    persistent: Vec<Zeroizing<String>>,
    fingerprint: [u8; 32],
    redactor: SecretRedactor,
}

impl RedactionCache {
    pub(super) fn new(persistent: Vec<Zeroizing<String>>) -> Self {
        let fingerprint = fingerprint(persistent.iter().map(|value| value.as_str()));
        let redactor = SecretRedactor::from_values(&persistent);
        Self {
            persistent,
            fingerprint,
            redactor,
        }
    }

    pub(super) fn refresh(&mut self, dynamic: &[Zeroizing<String>]) -> &SecretRedactor {
        let values = self
            .persistent
            .iter()
            .chain(dynamic)
            .map(|value| value.as_str());
        let fingerprint = fingerprint(values);
        if fingerprint != self.fingerprint {
            self.redactor = SecretRedactor::from_iter(
                self.persistent
                    .iter()
                    .chain(dynamic)
                    .map(|value| value.as_str()),
            );
            self.fingerprint = fingerprint;
        }
        &self.redactor
    }
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    max_bytes: usize,
    truncated: bool,
}

impl BoundedBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max_bytes.min(16 * 1024)),
            max_bytes,
            truncated: false,
        }
    }

    fn push(&mut self, value: &[u8]) -> bool {
        if value.is_empty() {
            return !self.truncated;
        }
        let remaining = self.max_bytes.saturating_sub(self.bytes.len());
        let retained = remaining.min(value.len());
        self.bytes.extend_from_slice(&value[..retained]);
        if retained < value.len() {
            self.truncated = true;
        }
        !self.truncated
    }

    fn into_text(mut self) -> (String, bool) {
        while std::str::from_utf8(&self.bytes).is_err() {
            self.bytes.pop();
            self.truncated = true;
        }
        (
            String::from_utf8(self.bytes).unwrap_or_default(),
            self.truncated,
        )
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.push(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn bounded_text(value: &str, max_bytes: usize, force_truncated: bool) -> (String, bool) {
    let mut output = BoundedBuffer::new(max_bytes);
    output.push(value.as_bytes());
    output.truncated |= force_truncated;
    output.into_text()
}

fn fingerprint<'a>(values: impl Iterator<Item = &'a str>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for value in values {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    *hasher.finalize().as_bytes()
}
