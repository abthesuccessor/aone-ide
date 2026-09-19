use std::{
    fs::{File, Metadata, OpenOptions},
    io::Read,
    path::Path,
};

use crate::error::{AoneError, AoneResult};

use super::limits::MAX_PREVIEW_FILE_BYTES;

pub fn read_source_file(path: &Path) -> AoneResult<String> {
    read_source_file_bounded(path, MAX_PREVIEW_FILE_BYTES)
}

pub(crate) fn read_source_file_bounded(path: &Path, maximum: u64) -> AoneResult<String> {
    let maximum = maximum.min(MAX_PREVIEW_FILE_BYTES);
    let path_metadata = path.symlink_metadata()?;
    let canonical = path.canonicalize()?;
    let canonical_metadata = canonical.metadata()?;
    let (file, opened_metadata) =
        open_verified_regular_file(path, &path_metadata, &canonical_metadata)?;
    if opened_metadata.len() > maximum {
        return Err(AoneError::FileTooLarge);
    }
    let (bytes, _) = read_bounded(file, &opened_metadata, path, maximum)?;
    if is_binary(&bytes) {
        return Err(AoneError::BinaryFile);
    }
    String::from_utf8(bytes)
        .map_err(|_| AoneError::InvalidRequest("file is not valid UTF-8".into()))
}

pub(super) fn open_verified_regular_file(
    path: &Path,
    before: &Metadata,
    canonical: &Metadata,
) -> AoneResult<(File, Metadata)> {
    if before.file_type().is_symlink() || !before.is_file() || !canonical.is_file() {
        return Err(AoneError::InvalidRequest(
            "refusing to read a symbolic link or non-regular file".into(),
        ));
    }

    let file = open_read_only_no_follow(path).map_err(secure_open_error)?;
    let opened = file.metadata()?;
    let after = path.symlink_metadata()?;
    if after.file_type().is_symlink()
        || !opened.is_file()
        || !after.is_file()
        || !same_file_identity(before, &opened)
        || !same_file_identity(canonical, &opened)
        || !same_file_identity(&after, &opened)
    {
        return Err(AoneError::InvalidRequest(
            "workspace file changed during secure open".into(),
        ));
    }
    Ok((file, opened))
}

#[cfg(unix)]
fn open_read_only_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_read_only_no_follow(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn secure_open_error(error: std::io::Error) -> AoneError {
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::ELOOP) {
        return AoneError::SensitivePath;
    }
    AoneError::Io(error)
}

#[cfg(unix)]
fn same_file_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

pub(super) fn read_bounded(
    mut file: File,
    opened_metadata: &Metadata,
    path: &Path,
    maximum: u64,
) -> AoneResult<(Vec<u8>, Metadata)> {
    let mut bytes = Vec::with_capacity(opened_metadata.len().min(maximum) as usize);
    file.by_ref().take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(AoneError::FileTooLarge);
    }
    let final_metadata = file.metadata()?;
    let final_path_metadata = path.symlink_metadata()?;
    if final_path_metadata.file_type().is_symlink()
        || !same_file_identity(opened_metadata, &final_metadata)
        || !same_file_identity(&final_path_metadata, &final_metadata)
        || final_metadata.len() != bytes.len() as u64
        || opened_metadata.modified().ok() != final_metadata.modified().ok()
    {
        return Err(AoneError::InvalidRequest(
            "workspace file changed while it was being read".into(),
        ));
    }
    Ok((bytes, final_metadata))
}

pub(super) fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8_192).any(|byte| *byte == 0)
}
