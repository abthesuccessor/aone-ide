use crate::domain::GraphNode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SystemLayer {
    Configuration,
    Entry,
    Interface,
    Application,
    Data,
    External,
}

impl SystemLayer {
    pub(super) const ALL: [Self; 6] = [
        Self::Configuration,
        Self::Entry,
        Self::Interface,
        Self::Application,
        Self::Data,
        Self::External,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Entry => "entry",
            Self::Interface => "interface",
            Self::Application => "application",
            Self::Data => "data",
            Self::External => "external",
        }
    }
}

pub(super) struct LayerPlacement {
    pub(super) layer: SystemLayer,
    pub(super) basis: String,
    pub(super) evidence: &'static str,
}

pub(super) fn placement(node: &GraphNode) -> LayerPlacement {
    let path = node_path(node);
    let kind = node.kind.to_ascii_lowercase();
    let label = node.label.trim().to_ascii_lowercase();

    if kind == "file" && is_safe_configuration_path(path) {
        return exact(
            SystemLayer::Configuration,
            format!("exact safe configuration filename: {}", basename(path)),
        );
    }
    if matches!(kind.as_str(), "function" | "method") && label == "main" {
        return exact(SystemLayer::Entry, "exact entry symbol name: main".into());
    }
    if kind == "file" && is_conventional_entry_path(path) {
        return inferred(
            SystemLayer::Entry,
            "inferred from a conventional entry filename; runtime startup is not proven".into(),
        );
    }
    if matches!(kind.as_str(), "endpoint" | "controller" | "event") {
        return exact(
            SystemLayer::Interface,
            format!("exact semantic node kind: {kind}"),
        );
    }
    if is_job_path(path) {
        return inferred(
            SystemLayer::Interface,
            "inferred from a job, worker, batch, queue, command, task, or cron path; execution and scheduling are not proven".into(),
        );
    }
    if matches!(
        kind.as_str(),
        "repository" | "model" | "database" | "table" | "schema" | "entity"
    ) {
        return exact(
            SystemLayer::Data,
            format!("exact semantic node kind: {kind}"),
        );
    }
    if is_data_path(path, &kind) {
        return inferred(
            SystemLayer::Data,
            "inferred from a repository, persistence, database, model, or store path; datastore technology and operations are not claimed".into(),
        );
    }
    if matches!(kind.as_str(), "api" | "external-api" | "externalapi") {
        if has_absolute_non_local_http_target(&node.label) {
            return inferred(
                SystemLayer::External,
                "inferred from a statically extracted absolute HTTP(S) request target; provider and cloud identity are not claimed".into(),
            );
        }
        return inferred(
            SystemLayer::Interface,
            "outbound request target unverified; external provider and cloud identity are not claimed"
                .into(),
        );
    }
    if kind == "service" {
        return exact(
            SystemLayer::Application,
            "exact semantic node kind: service".into(),
        );
    }

    inferred(
        SystemLayer::Application,
        format!("inferred default placement for {} code", node.kind),
    )
}

pub(super) fn eligible_node(node: &GraphNode) -> bool {
    if matches!(node.kind.as_str(), "callTarget" | "module") {
        return false;
    }
    let path = node_path(node);
    if is_test_path(path) || is_auxiliary_path(path) || is_sensitive_configuration_path(path) {
        return false;
    }
    node.kind != "file" || is_safe_configuration_path(path) || is_conventional_entry_path(path)
}

pub(super) fn is_test_path(path: &str) -> bool {
    let normalized = format!("/{}", path.replace('\\', "/").to_ascii_lowercase());
    normalized.contains("/tests/")
        || normalized.contains("/test/")
        || normalized.contains("/e2e/")
        || normalized.contains("/__tests__/")
        || normalized.contains(".test.")
        || normalized.contains(".spec.")
        || normalized.contains("_test.")
        || normalized.contains("_tests.")
        || normalized.contains("-test.")
        || normalized.contains("-tests.")
}

pub(super) fn is_safe_configuration_path(path: &str) -> bool {
    let name = basename(path).to_ascii_lowercase();
    matches!(
        name.as_str(),
        "package.json"
            | "cargo.toml"
            | "pyproject.toml"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "tsconfig.json"
            | "jsconfig.json"
            | "tauri.conf.json"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | "compose.yml"
            | "compose.yaml"
            | "cmakelists.txt"
            | "makefile"
            | ".editorconfig"
            | "config.toml"
            | "config.json"
            | "config.yml"
            | "config.yaml"
            | "application.yml"
            | "application.yaml"
    ) || name.starts_with("vite.config.")
        || name.starts_with("next.config.")
        || name.starts_with("webpack.config.")
}

fn is_sensitive_configuration_path(path: &str) -> bool {
    let name = basename(path).to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name.contains("credential")
        || name.contains("secret")
}

fn is_conventional_entry_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let name = basename(&normalized);
    if matches!(
        name,
        "main.rs"
            | "main.go"
            | "main.py"
            | "main.kt"
            | "main.ts"
            | "main.tsx"
            | "main.js"
            | "main.jsx"
            | "server.ts"
            | "server.js"
            | "app.ts"
            | "app.js"
    ) {
        return true;
    }
    matches!(
        normalized.as_str(),
        "index.ts"
            | "index.tsx"
            | "index.js"
            | "index.jsx"
            | "src/index.ts"
            | "src/index.tsx"
            | "src/index.js"
            | "src/index.jsx"
    )
}

fn is_job_path(path: &str) -> bool {
    let normalized = format!("/{}/", path.replace('\\', "/").to_ascii_lowercase());
    [
        "/job/",
        "/jobs/",
        "/worker/",
        "/workers/",
        "/cron/",
        "/batch/",
        "/background/",
        "/queue/",
        "/queues/",
        "/command/",
        "/commands/",
        "/task/",
        "/tasks/",
    ]
    .iter()
    .any(|segment| normalized.contains(segment))
}

fn is_data_path(path: &str, kind: &str) -> bool {
    if !matches!(
        kind,
        "function"
            | "method"
            | "class"
            | "service"
            | "repository"
            | "model"
            | "interface"
            | "trait"
            | "struct"
            | "enum"
            | "implementation"
            | "object"
            | "type"
    ) {
        return false;
    }
    let normalized = format!("/{}/", path.replace('\\', "/").to_ascii_lowercase());
    let strong = [
        "/data/",
        "/database/",
        "/databases/",
        "/repository/",
        "/repositories/",
        "/persistence/",
        "/model/",
        "/models/",
        "/schema/",
        "/schemas/",
        "/migration/",
        "/migrations/",
        "/repository.",
        "/model.",
        "_repository.",
        "-repository.",
        "_model.",
        "-model.",
    ]
    .iter()
    .any(|token| normalized.contains(token));
    let typed_storage = normalized.contains("/storage/")
        && matches!(
            kind,
            "class"
                | "service"
                | "repository"
                | "model"
                | "interface"
                | "trait"
                | "struct"
                | "enum"
                | "implementation"
                | "object"
                | "type"
        );
    strong || typed_storage
}

fn is_auxiliary_path(path: &str) -> bool {
    let normalized = format!("/{}/", path.replace('\\', "/").to_ascii_lowercase());
    [
        "/evaluation/",
        "/evaluations/",
        "/eval/",
        "/example/",
        "/examples/",
        "/benchmark/",
        "/benchmarks/",
    ]
    .iter()
    .any(|segment| normalized.contains(segment))
}

fn has_absolute_non_local_http_target(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    let rest = lower
        .find("https://")
        .map(|index| &lower[index + 8..])
        .or_else(|| lower.find("http://").map(|index| &lower[index + 7..]));
    let Some(rest) = rest else {
        return false;
    };
    let authority = rest
        .split(['/', '?', '#', ' ', '\t', '\r', '\n'])
        .next()
        .unwrap_or_default();
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = if let Some(ipv6) = host_port.strip_prefix('[') {
        ipv6.split(']').next().unwrap_or_default()
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    !(host == "localhost" || host == "::1" || host == "0.0.0.0" || host.starts_with("127."))
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn node_path(node: &GraphNode) -> &str {
    node.source
        .as_ref()
        .map(|source| source.relative_path.as_str())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| if node.kind == "file" { &node.label } else { "" })
}

fn exact(layer: SystemLayer, basis: String) -> LayerPlacement {
    LayerPlacement {
        layer,
        basis,
        evidence: "exact",
    }
}

fn inferred(layer: SystemLayer, basis: String) -> LayerPlacement {
    LayerPlacement {
        layer,
        basis,
        evidence: "inferred",
    }
}
