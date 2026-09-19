mod address;

#[cfg(test)]
mod tests;

pub(crate) use address::{DestinationScope, classify_destination, is_cloud_metadata_hostname};
