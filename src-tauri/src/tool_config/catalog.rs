use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use directories::UserDirs;

use super::files::attest_target;
use crate::{
    domain::{
        ToolConfiguration, ToolConfigurationKind, ToolConfigurationScope, ToolInspectionState,
    },
    error::{AoneError, AoneResult},
    state::AppState,
};

const MAX_PATH_DIRECTORIES: usize = 64;
const CODEX_PATHS: &[&str] = &[
    "/Applications/Codex.app",
    "/opt/homebrew/bin/codex",
    "/usr/local/bin/codex",
    "~/.local/bin/codex",
];
const CLAUDE_PATHS: &[&str] = &[
    "/Applications/Claude.app",
    "~/Applications/Claude.app",
    "/opt/homebrew/bin/claude",
    "/usr/local/bin/claude",
    "~/.local/bin/claude",
];
const CURSOR_PATHS: &[&str] = &[
    "/Applications/Cursor.app",
    "~/Applications/Cursor.app",
    "/opt/homebrew/bin/cursor-agent",
    "/usr/local/bin/cursor-agent",
    "~/.local/bin/cursor-agent",
    "~/.cursor/bin/cursor-agent",
];
const GEMINI_PATHS: &[&str] = &[
    "/opt/homebrew/bin/gemini",
    "/usr/local/bin/gemini",
    "~/.local/bin/gemini",
    "~/.npm-global/bin/gemini",
];
const VSCODE_PATHS: &[&str] = &[
    "/Applications/Visual Studio Code.app",
    "~/Applications/Visual Studio Code.app",
    "/opt/homebrew/bin/code",
    "/usr/local/bin/code",
];

#[derive(Debug, Clone, Copy)]
pub(super) enum ConfigLocation {
    Home(&'static str),
    Workspace(&'static str),
}

impl ConfigLocation {
    fn scope(self) -> ToolConfigurationScope {
        match self {
            Self::Home(_) => ToolConfigurationScope::Home,
            Self::Workspace(_) => ToolConfigurationScope::Workspace,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolSpec {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) company: &'static str,
    pub(super) kind: ToolConfigurationKind,
    pub(super) path_hint: &'static str,
    pub(super) location: ConfigLocation,
    pub(super) template: &'static str,
    pub(super) detected_paths: &'static [&'static str],
    pub(super) detected_names: &'static [&'static str],
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        id: "codex",
        label: "Codex CLI and MCP",
        company: "OpenAI",
        kind: ToolConfigurationKind::Cli,
        path_hint: "~/.codex/config.toml",
        location: ConfigLocation::Home(".codex/config.toml"),
        template: "[mcp_servers]\n",
        detected_paths: CODEX_PATHS,
        detected_names: &["codex"],
    },
    ToolSpec {
        id: "claude-desktop",
        label: "Claude Desktop MCP",
        company: "Anthropic",
        kind: ToolConfigurationKind::Mcp,
        path_hint: "~/Library/Application Support/Claude/claude_desktop_config.json",
        location: ConfigLocation::Home(
            "Library/Application Support/Claude/claude_desktop_config.json",
        ),
        template: "{\n  \"mcpServers\": {}\n}\n",
        detected_paths: CLAUDE_PATHS,
        detected_names: &["claude"],
    },
    ToolSpec {
        id: "cursor",
        label: "Cursor MCP",
        company: "Anysphere",
        kind: ToolConfigurationKind::Mcp,
        path_hint: "~/.cursor/mcp.json",
        location: ConfigLocation::Home(".cursor/mcp.json"),
        template: "{\n  \"mcpServers\": {}\n}\n",
        detected_paths: CURSOR_PATHS,
        detected_names: &["cursor-agent", "cursor"],
    },
    ToolSpec {
        id: "gemini-cli",
        label: "Gemini CLI and MCP",
        company: "Google",
        kind: ToolConfigurationKind::Cli,
        path_hint: "~/.gemini/settings.json",
        location: ConfigLocation::Home(".gemini/settings.json"),
        template: "{\n  \"mcpServers\": {}\n}\n",
        detected_paths: GEMINI_PATHS,
        detected_names: &["gemini"],
    },
    ToolSpec {
        id: "vscode-workspace-mcp",
        label: "VS Code workspace MCP",
        company: "Microsoft",
        kind: ToolConfigurationKind::Mcp,
        path_hint: ".vscode/mcp.json",
        location: ConfigLocation::Workspace(".vscode/mcp.json"),
        template: "{\n  \"servers\": {}\n}\n",
        detected_paths: VSCODE_PATHS,
        detected_names: &["code"],
    },
    ToolSpec {
        id: "workspace-agents-md",
        label: "Codex workspace instructions",
        company: "OpenAI",
        kind: ToolConfigurationKind::Agent,
        path_hint: "AGENTS.md",
        location: ConfigLocation::Workspace("AGENTS.md"),
        template: "# Project instructions\n\nAdd project-specific guidance for coding agents here.\n",
        detected_paths: CODEX_PATHS,
        detected_names: &["codex"],
    },
    ToolSpec {
        id: "claude-workspace-instructions",
        label: "Claude project instructions",
        company: "Anthropic",
        kind: ToolConfigurationKind::Instructions,
        path_hint: "CLAUDE.md",
        location: ConfigLocation::Workspace("CLAUDE.md"),
        template: "# Claude project instructions\n\nAdd project-specific guidance here.\n",
        detected_paths: CLAUDE_PATHS,
        detected_names: &["claude"],
    },
    ToolSpec {
        id: "gemini-workspace-instructions",
        label: "Gemini project instructions",
        company: "Google",
        kind: ToolConfigurationKind::Instructions,
        path_hint: "GEMINI.md",
        location: ConfigLocation::Workspace("GEMINI.md"),
        template: "# Gemini project instructions\n\nAdd project-specific guidance here.\n",
        detected_paths: GEMINI_PATHS,
        detected_names: &["gemini"],
    },
    ToolSpec {
        id: "cursor-project-rule",
        label: "Cursor project rule",
        company: "Anysphere",
        kind: ToolConfigurationKind::Rules,
        path_hint: ".cursor/rules/aone-project.mdc",
        location: ConfigLocation::Workspace(".cursor/rules/aone-project.mdc"),
        template: "---\ndescription: Project-specific guidance\nalwaysApply: false\n---\n\n# Project guidance\n\nAdd project-specific guidance here.\n",
        detected_paths: CURSOR_PATHS,
        detected_names: &["cursor-agent", "cursor"],
    },
    ToolSpec {
        id: "copilot-workspace-instructions",
        label: "GitHub Copilot instructions",
        company: "GitHub",
        kind: ToolConfigurationKind::Instructions,
        path_hint: ".github/copilot-instructions.md",
        location: ConfigLocation::Workspace(".github/copilot-instructions.md"),
        template: "# Copilot project instructions\n\nAdd project-specific guidance here.\n",
        detected_paths: VSCODE_PATHS,
        detected_names: &["code", "copilot"],
    },
    ToolSpec {
        id: "portable-project-skill",
        label: "Portable project skill",
        company: "Agent Skills",
        kind: ToolConfigurationKind::Skill,
        path_hint: ".agents/skills/aone-project/SKILL.md",
        location: ConfigLocation::Workspace(".agents/skills/aone-project/SKILL.md"),
        template: "---\nname: aone-project\ndescription: Project-specific guidance for this workspace.\n---\n\n# Aone project skill\n\nAdd project-specific guidance here.\n",
        detected_paths: &[],
        detected_names: &[],
    },
];

pub(super) struct InspectionContext {
    home: Option<PathBuf>,
    path_directories: Vec<PathBuf>,
}

impl InspectionContext {
    pub(super) fn capture() -> Self {
        let home = UserDirs::new()
            .map(|directories| directories.home_dir().to_path_buf())
            .and_then(|path| path.canonicalize().ok());
        let mut directories = BTreeSet::new();
        if let Some(value) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .take(MAX_PATH_DIRECTORIES)
            {
                if let Ok(canonical) = directory.canonicalize()
                    && canonical.is_dir()
                {
                    directories.insert(canonical);
                }
            }
        }
        Self {
            home,
            path_directories: directories.into_iter().collect(),
        }
    }
}

pub(super) fn specs() -> &'static [ToolSpec] {
    TOOLS
}

pub(super) fn find_spec(tool_id: &str) -> AoneResult<&'static ToolSpec> {
    TOOLS
        .iter()
        .find(|spec| spec.id == tool_id)
        .ok_or_else(|| AoneError::InvalidRequest("unknown tool configuration id".into()))
}

pub(super) fn describe_static(spec: &ToolSpec) -> ToolConfiguration {
    description(
        spec,
        ToolInspectionState::NotInspected,
        ToolInspectionState::NotInspected,
    )
}

pub(super) fn describe_inspected(
    spec: &ToolSpec,
    state: &AppState,
    context: &InspectionContext,
) -> ToolConfiguration {
    let configuration_state = resolve_inspection_path(spec, state, context)
        .map(|(root, target)| inspection_state_for_target(&root, &target))
        .unwrap_or(ToolInspectionState::NotFound);
    let cli_state = if cli_detected(spec, context) {
        ToolInspectionState::Found
    } else {
        ToolInspectionState::NotFound
    };
    description(spec, configuration_state, cli_state)
}

fn description(
    spec: &ToolSpec,
    configuration_state: ToolInspectionState,
    cli_state: ToolInspectionState,
) -> ToolConfiguration {
    ToolConfiguration {
        id: spec.id.into(),
        label: spec.label.into(),
        company: spec.company.into(),
        kind: spec.kind,
        scope: spec.location.scope(),
        path_hint: spec.path_hint.into(),
        configuration_state,
        cli_state,
    }
}

pub(super) fn resolve_path(spec: &ToolSpec, state: &AppState) -> AoneResult<(PathBuf, PathBuf)> {
    let (base, relative) = match spec.location {
        ConfigLocation::Home(relative) => (canonical_home()?, relative),
        ConfigLocation::Workspace(relative) => (state.workspace()?.root, relative),
    };
    target_in_base(base, relative)
}

fn resolve_inspection_path(
    spec: &ToolSpec,
    state: &AppState,
    context: &InspectionContext,
) -> Option<(PathBuf, PathBuf)> {
    let (base, relative) = match spec.location {
        ConfigLocation::Home(relative) => (context.home.clone()?, relative),
        ConfigLocation::Workspace(relative) => (state.workspace_if_open()?.root, relative),
    };
    target_in_base(base, relative).ok()
}

fn target_in_base(base: PathBuf, relative: &str) -> AoneResult<(PathBuf, PathBuf)> {
    let target = base.join(relative);
    if !target.starts_with(&base) {
        return Err(AoneError::PathEscape);
    }
    Ok((base, target))
}

fn canonical_home() -> AoneResult<PathBuf> {
    UserDirs::new()
        .map(|directories| directories.home_dir().to_path_buf())
        .ok_or_else(|| AoneError::Task("macOS home directory is unavailable".into()))?
        .canonicalize()
        .map_err(AoneError::Io)
}

pub(super) fn inspection_state_for_target(root: &Path, target: &Path) -> ToolInspectionState {
    if attest_target(root, target).is_ok_and(|attestation| attestation.exists()) {
        ToolInspectionState::Found
    } else {
        ToolInspectionState::NotFound
    }
}

fn cli_detected(spec: &ToolSpec, context: &InspectionContext) -> bool {
    spec.detected_paths.iter().any(|raw| {
        let candidate = match raw.strip_prefix("~/") {
            Some(relative) => context.home.as_ref().map(|root| root.join(relative)),
            None => Some(PathBuf::from(*raw)),
        };
        candidate.is_some_and(|path| is_detected_tool(&path))
    }) || context.path_directories.iter().any(|directory| {
        spec.detected_names
            .iter()
            .any(|name| is_detected_tool(&directory.join(name)))
    })
}

fn is_detected_tool(path: &Path) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    let Ok(metadata) = canonical.metadata() else {
        return false;
    };
    if metadata.is_dir() {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}
