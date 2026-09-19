use std::collections::{BTreeSet, HashMap};

use zeroize::Zeroizing;

use super::{
    RuntimeState,
    environment::{MAX_ENV_NAME_REQUESTS, MAX_SELECTED_ENV_NAMES, validate_run_env_name},
};
use crate::error::{AoneError, AoneResult};

impl RuntimeState {
    /// Replaces trusted backend environment values; old values are zeroized.
    pub fn replace_env(&self, values: HashMap<String, String>) -> AoneResult<()> {
        let mut replacement = HashMap::with_capacity(values.len());
        for (name, value) in values {
            validate_run_env_name(&name)?;
            if value.contains('\0') {
                return Err(AoneError::InvalidRequest(format!(
                    "environment variable {name} contains a NUL byte"
                )));
            }
            replacement.insert(name, Zeroizing::new(value));
        }
        *self.loaded_env.write() = replacement;
        Ok(())
    }

    /// Returns names only. Secret values must never cross IPC.
    #[cfg(test)]
    pub fn env_names(&self) -> Vec<String> {
        let mut names = self.loaded_env.read().keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn secret_values_for_redaction(&self) -> Vec<Zeroizing<String>> {
        self.loaded_env
            .read()
            .values()
            .map(|value| Zeroizing::new(value.to_string()))
            .collect()
    }

    pub(super) fn child_environment(
        &self,
        requested: &[String],
    ) -> AoneResult<Vec<(String, String)>> {
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        if requested.len() > MAX_ENV_NAME_REQUESTS {
            return Err(AoneError::InvalidRequest(format!(
                "too many environment name selections; maximum is {MAX_ENV_NAME_REQUESTS}"
            )));
        }
        let loaded = self.loaded_env.read();

        let mut unique_names = BTreeSet::new();
        for name in requested {
            validate_run_env_name(name)?;
            unique_names.insert(name.as_str());
            if unique_names.len() > MAX_SELECTED_ENV_NAMES {
                return Err(AoneError::InvalidRequest(format!(
                    "too many distinct environment names; maximum is {MAX_SELECTED_ENV_NAMES}"
                )));
            }
        }

        let mut selected = Vec::with_capacity(unique_names.len());
        for name in unique_names {
            let value = loaded.get(name).ok_or_else(|| {
                AoneError::InvalidRequest(format!(
                    "environment variable {name} has not been loaded"
                ))
            })?;
            selected.push((name.to_owned(), value.to_string()));
        }
        Ok(selected)
    }
}
