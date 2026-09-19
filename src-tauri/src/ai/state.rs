use std::{
    collections::HashMap,
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
};

use parking_lot::RwLock;
use reqwest::Url;
use zeroize::Zeroizing;

use super::cli_adapter::{CliAdapter, CliKind};
use super::graph_projection::WorkspaceGraphProjection;
use crate::{
    domain::{AiConfigurationStatus, GraphEdge, GraphNode, GraphProjection, GraphSnapshot},
    error::{AoneError, AoneResult},
};

pub(super) const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-luna";
pub(super) const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-5";
const MAX_API_KEY_BYTES: usize = 16 * 1024;
const MAX_OLLAMA_ENDPOINT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AiProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    Codex,
    Claude,
}

impl AiProviderKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Ollama => "Ollama",
            Self::Codex => "Codex CLI",
            Self::Claude => "Claude Code",
        }
    }

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Ollama => "ollama",
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

pub(super) struct ProviderConfiguration {
    pub(super) kind: AiProviderKind,
    pub(super) model: String,
    pub(super) transport: ProviderTransport,
}

pub(super) enum ProviderTransport {
    HostedApi { api_key: Zeroizing<String> },
    LocalHttp { chat_endpoint: Url },
    Cli { adapter: CliAdapter },
}

struct ProviderState {
    selected: AiProviderKind,
    model: String,
    transport: Option<StoredProviderTransport>,
}

enum StoredProviderTransport {
    HostedApi { api_key: Zeroizing<String> },
    LocalHttp { chat_endpoint: Url },
    Cli { adapter: CliAdapter },
}

pub(super) struct PreparedOllamaConfiguration {
    pub(super) model: String,
    pub(super) chat_endpoint: Url,
    pub(super) tags_endpoint: Url,
}

/// Backend-only AI state. It stores only the provider configuration required
/// for AI and a bounded projection of graph facts used as citable evidence.
pub struct SecretState {
    provider: RwLock<ProviderState>,
    graph_projection: RwLock<Option<WorkspaceGraphProjection>>,
    ai_call_in_flight: AtomicBool,
}

impl SecretState {
    pub fn new() -> Self {
        Self {
            provider: RwLock::new(ProviderState {
                selected: AiProviderKind::OpenAi,
                model: DEFAULT_OPENAI_MODEL.into(),
                transport: None,
            }),
            graph_projection: RwLock::new(None),
            ai_call_in_flight: AtomicBool::new(false),
        }
    }

    /// Accepts environment values from trusted backend loading only. The
    /// renderer never receives or supplies these values to ai_explain.
    pub fn replace_env(&self, mut values: HashMap<String, String>) -> Result<(), String> {
        let openai_api_key = take_secret(&mut values, "OPENAI_API_KEY");
        let anthropic_api_key = take_secret(&mut values, "ANTHROPIC_API_KEY");
        let openai_model = values
            .remove("OPENAI_MODEL")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into());
        let anthropic_model = values
            .remove("ANTHROPIC_MODEL")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ANTHROPIC_MODEL.into());
        validate_model_name(&openai_model)?;
        validate_model_name(&anthropic_model)?;

        let requested = values.remove("AONE_AI_PROVIDER");
        let selected = match requested.as_deref().map(str::trim) {
            Some("openai") => AiProviderKind::OpenAi,
            Some("anthropic") => AiProviderKind::Anthropic,
            Some("") | None if openai_api_key.is_some() && anthropic_api_key.is_some() => {
                return Err(
                    "AONE_AI_PROVIDER must be openai or anthropic when both provider keys are present"
                        .into(),
                );
            }
            Some("") | None if anthropic_api_key.is_some() => AiProviderKind::Anthropic,
            Some("") | None => AiProviderKind::OpenAi,
            Some(_) => return Err("AONE_AI_PROVIDER must be openai or anthropic".into()),
        };

        let (api_key, model) = match selected {
            AiProviderKind::OpenAi => (openai_api_key, openai_model),
            AiProviderKind::Anthropic => (anthropic_api_key, anthropic_model),
            AiProviderKind::Ollama => unreachable!("environment selection cannot choose Ollama"),
            AiProviderKind::Codex | AiProviderKind::Claude => {
                unreachable!("environment selection cannot choose a CLI adapter")
            }
        };
        let api_key = api_key.map(normalize_api_key).transpose()?;
        *self.provider.write() = ProviderState {
            selected,
            model,
            transport: api_key.map(|api_key| StoredProviderTransport::HostedApi { api_key }),
        };
        Ok(())
    }

    pub(super) fn configure_hosted(
        &self,
        provider: String,
        api_key: Zeroizing<String>,
        model: Option<String>,
    ) -> Result<(), String> {
        let provider = provider.trim();
        if provider.len() > 20 || provider.chars().any(char::is_control) {
            return Err("hosted AI provider must be openai or anthropic".into());
        }
        let selected = match provider {
            "openai" => AiProviderKind::OpenAi,
            "anthropic" => AiProviderKind::Anthropic,
            _ => return Err("hosted AI provider must be openai or anthropic".into()),
        };
        let api_key = normalize_api_key(api_key)?;
        let default_model = match selected {
            AiProviderKind::OpenAi => DEFAULT_OPENAI_MODEL,
            AiProviderKind::Anthropic => DEFAULT_ANTHROPIC_MODEL,
            AiProviderKind::Ollama => unreachable!("Ollama is not a hosted provider"),
            AiProviderKind::Codex | AiProviderKind::Claude => {
                unreachable!("a CLI adapter is not a hosted provider")
            }
        };
        let model = model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let model = model.unwrap_or(default_model);
        validate_model_name(model)?;
        let model = model.to_string();

        *self.provider.write() = ProviderState {
            selected,
            model,
            transport: Some(StoredProviderTransport::HostedApi { api_key }),
        };
        Ok(())
    }

    pub(super) fn prepare_ollama(
        endpoint: String,
        model: String,
    ) -> Result<PreparedOllamaConfiguration, String> {
        let model = model.trim();
        validate_model_name(model)?;
        let (chat_endpoint, tags_endpoint) = normalize_ollama_endpoint(&endpoint)?;
        Ok(PreparedOllamaConfiguration {
            model: model.to_string(),
            chat_endpoint,
            tags_endpoint,
        })
    }

    pub(super) fn install_ollama(&self, configuration: PreparedOllamaConfiguration) {
        *self.provider.write() = ProviderState {
            selected: AiProviderKind::Ollama,
            model: configuration.model,
            transport: Some(StoredProviderTransport::LocalHttp {
                chat_endpoint: configuration.chat_endpoint,
            }),
        };
    }

    pub(super) fn install_cli(&self, kind: CliKind, adapter: CliAdapter, version: String) {
        *self.provider.write() = ProviderState {
            selected: match kind {
                CliKind::Codex => AiProviderKind::Codex,
                CliKind::Claude => AiProviderKind::Claude,
            },
            model: format!("{} {version}", kind.id()),
            transport: Some(StoredProviderTransport::Cli { adapter }),
        };
    }

    pub fn replace_graph_snapshot(&self, workspace_id: &str, snapshot: &GraphSnapshot) {
        *self.graph_projection.write() = Some(WorkspaceGraphProjection::from_snapshot(
            workspace_id,
            snapshot,
        ));
    }

    pub fn replace_graph_nodes(&self, workspace_id: &str, nodes: Vec<GraphNode>) {
        *self.graph_projection.write() =
            Some(WorkspaceGraphProjection::from_nodes(workspace_id, nodes));
    }

    pub fn replace_query_snapshot(
        &self,
        workspace_id: &str,
        projection_kind: GraphProjection,
        snapshot: &GraphSnapshot,
    ) {
        if projection_kind == GraphProjection::ExecutionFlow {
            return;
        }
        let mut projection = self.graph_projection.write();
        if projection
            .as_ref()
            .is_none_or(|current| current.workspace_id != workspace_id)
        {
            *projection = Some(WorkspaceGraphProjection::empty(workspace_id));
        }
        let current = projection.as_mut().expect("projection was initialized");
        current.replace(projection_kind, snapshot);
    }

    pub(super) fn provider_configuration(&self) -> AoneResult<ProviderConfiguration> {
        let state = self.provider.read();
        let transport = state.transport.as_ref().ok_or_else(|| {
            AoneError::InvalidRequest(format!(
                "AI explanation is optional; configure {} to enable it",
                state.selected.label()
            ))
        })?;
        let transport = match transport {
            StoredProviderTransport::HostedApi { api_key } => ProviderTransport::HostedApi {
                api_key: Zeroizing::new(api_key.to_string()),
            },
            StoredProviderTransport::LocalHttp { chat_endpoint } => ProviderTransport::LocalHttp {
                chat_endpoint: chat_endpoint.clone(),
            },
            StoredProviderTransport::Cli { adapter } => ProviderTransport::Cli {
                adapter: adapter.clone(),
            },
        };
        Ok(ProviderConfiguration {
            kind: state.selected,
            model: state.model.clone(),
            transport,
        })
    }

    pub(crate) fn configuration_status(&self) -> AiConfigurationStatus {
        let state = self.provider.read();
        let configured = state.transport.is_some();
        let transport = state.transport.as_ref().map(|transport| match transport {
            StoredProviderTransport::HostedApi { .. } => "api",
            StoredProviderTransport::LocalHttp { .. } => "local_http",
            StoredProviderTransport::Cli { .. } => "cli",
        });

        AiConfigurationStatus {
            configured,
            provider: configured.then(|| state.selected.id().to_string()),
            model: configured.then(|| state.model.clone()),
            transport: transport.map(str::to_string),
            inference_available: configured,
        }
    }

    #[cfg(test)]
    pub(super) fn model(&self) -> String {
        self.provider.read().model.clone()
    }

    #[cfg(test)]
    pub(super) fn provider_kind(&self) -> AiProviderKind {
        self.provider.read().selected
    }

    pub(super) fn selected_nodes(
        &self,
        workspace_id: &str,
        ids: &[String],
        max: usize,
    ) -> AoneResult<Vec<GraphNode>> {
        let projection = self.graph_projection.read();
        let projection = projection
            .as_ref()
            .filter(|projection| projection.workspace_id == workspace_id)
            .ok_or_else(|| {
                AoneError::InvalidRequest(
                    "AI graph evidence is unavailable for the current workspace".into(),
                )
            })?;
        Ok(projection.selected_nodes(ids, max))
    }

    pub(super) fn selected_edges(
        &self,
        workspace_id: &str,
        node_ids: &[String],
        max: usize,
    ) -> AoneResult<Vec<GraphEdge>> {
        let projection = self.graph_projection.read();
        let projection = projection
            .as_ref()
            .filter(|projection| projection.workspace_id == workspace_id)
            .ok_or_else(|| {
                AoneError::InvalidRequest(
                    "AI graph evidence is unavailable for the current workspace".into(),
                )
            })?;
        Ok(projection.selected_edges(node_ids, max))
    }

    pub(super) fn reserve_ai_call(&self) -> AoneResult<AiCallReservation<'_>> {
        self.ai_call_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                AoneError::InvalidRequest(
                    "an AI explanation is already awaiting confirmation or in progress".into(),
                )
            })?;
        Ok(AiCallReservation {
            in_flight: &self.ai_call_in_flight,
        })
    }
}

pub(super) struct AiCallReservation<'a> {
    in_flight: &'a AtomicBool,
}

impl Drop for AiCallReservation<'_> {
    fn drop(&mut self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

impl Default for SecretState {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_model_name(model: &str) -> Result<(), String> {
    if model.is_empty()
        || model.len() > 120
        || !model.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
    {
        return Err("AI provider model contains unsupported characters".into());
    }
    Ok(())
}

fn normalize_api_key(api_key: Zeroizing<String>) -> Result<Zeroizing<String>, String> {
    let trimmed = api_key.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_API_KEY_BYTES
        || trimmed.chars().any(char::is_control)
    {
        return Err("AI provider API key is empty or contains unsupported data".into());
    }
    Ok(Zeroizing::new(trimmed.to_string()))
}

fn normalize_ollama_endpoint(endpoint: &str) -> Result<(Url, Url), String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.len() > MAX_OLLAMA_ENDPOINT_BYTES {
        return Err("Ollama endpoint is empty or too long".into());
    }
    let mut base = Url::parse(endpoint).map_err(|_| "Ollama endpoint is not a valid URL")?;
    if base.scheme() != "http" {
        return Err("Ollama endpoint must use loopback HTTP".into());
    }
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err("Ollama endpoint cannot contain credentials, a query, or a fragment".into());
    }
    let host = base
        .host_str()
        .ok_or("Ollama endpoint must include a loopback host")?;
    let literal = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if literal.eq_ignore_ascii_case("localhost") {
        base.set_host(Some("127.0.0.1"))
            .map_err(|_| "Ollama endpoint contains an invalid host")?;
    } else if !literal
        .parse::<IpAddr>()
        .is_ok_and(|address| address.is_loopback())
    {
        return Err("Ollama endpoint must use a loopback IP address or localhost".into());
    }
    if base.port() == Some(0) {
        return Err("Ollama endpoint must use a valid port".into());
    }
    if !matches!(base.path(), "" | "/" | "/api/chat" | "/api/tags") {
        return Err("Ollama endpoint path must be empty, /api/chat, or /api/tags".into());
    }

    let mut chat_endpoint = base.clone();
    chat_endpoint.set_path("/api/chat");
    let mut tags_endpoint = base;
    tags_endpoint.set_path("/api/tags");
    Ok((chat_endpoint, tags_endpoint))
}

fn take_secret(values: &mut HashMap<String, String>, name: &str) -> Option<Zeroizing<String>> {
    values.remove(name).and_then(|value| {
        let value = Zeroizing::new(value);
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| Zeroizing::new(trimmed.to_string()))
    })
}
