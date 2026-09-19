use base64::{Engine as _, engine::general_purpose::STANDARD};
use portable_pty::PtySize;

use crate::{
    domain::{TerminalInputEncoding, TerminalWriteRequest},
    error::{AoneError, AoneResult},
};

pub(super) const MAX_INPUT_BYTES: usize = 64 * 1024;
pub(super) const MIN_COLUMNS: u16 = 2;
pub(super) const MAX_COLUMNS: u16 = 500;
pub(super) const MIN_ROWS: u16 = 1;
pub(super) const MAX_ROWS: u16 = 300;

pub(super) fn pty_size(columns: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(MIN_ROWS, MAX_ROWS),
        cols: columns.clamp(MIN_COLUMNS, MAX_COLUMNS),
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub(super) fn decode_input(request: &TerminalWriteRequest) -> AoneResult<Vec<u8>> {
    if request.data.len() > encoded_input_limit(request.encoding) {
        return Err(input_too_large());
    }
    let bytes = match request.encoding {
        TerminalInputEncoding::Text => request.data.as_bytes().to_vec(),
        TerminalInputEncoding::Base64 => STANDARD
            .decode(request.data.as_bytes())
            .map_err(|_| AoneError::InvalidRequest("terminal input is not valid base64".into()))?,
    };
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(input_too_large());
    }
    Ok(bytes)
}

pub(super) fn validate_session_id(value: &str) -> AoneResult<()> {
    if value.len() > 96
        || !value.starts_with("terminal:")
        || value.len() == "terminal:".len()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | '-'))
    {
        return Err(AoneError::InvalidRequest(
            "invalid terminal session id".into(),
        ));
    }
    Ok(())
}

fn encoded_input_limit(encoding: TerminalInputEncoding) -> usize {
    match encoding {
        TerminalInputEncoding::Text => MAX_INPUT_BYTES,
        TerminalInputEncoding::Base64 => MAX_INPUT_BYTES.div_ceil(3) * 4,
    }
}

fn input_too_large() -> AoneError {
    AoneError::InvalidRequest(format!(
        "terminal input exceeds the {MAX_INPUT_BYTES}-byte limit"
    ))
}
