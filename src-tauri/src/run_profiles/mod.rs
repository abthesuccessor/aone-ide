mod discovery;
mod package_json;
mod profile;

#[cfg(test)]
mod tests;

pub use discovery::detect_profiles;
