mod discovery;
mod package_json;
mod profile;

#[cfg(test)]
mod tests;

pub use discovery::detect_profiles;
pub(crate) use profile::attach_required_env;
