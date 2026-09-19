use std::{ffi::OsString, path::Path};

pub(super) const BUILTIN_FORMATTER: &str = "Aone safe normalizer";

#[derive(Debug, Clone)]
pub(super) struct FormatterDefinition {
    pub(super) languages: &'static [&'static str],
    pub(super) formatter: &'static str,
    pub(super) executable: &'static str,
    pub(super) kind: FormatterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormatterKind {
    Black,
    ClangFormat,
    GoogleJavaFormat,
    Gofmt,
    Prettier,
    Rustfmt,
    Shfmt,
    Taplo,
}

#[derive(Debug, Clone)]
pub(super) struct FormatterPlan {
    pub(super) formatter: &'static str,
    pub(super) executable: &'static str,
    pub(super) args: Vec<OsString>,
}

pub(super) const FORMATTERS: &[FormatterDefinition] = &[
    FormatterDefinition {
        languages: &["Rust"],
        formatter: "rustfmt",
        executable: "rustfmt",
        kind: FormatterKind::Rustfmt,
    },
    FormatterDefinition {
        languages: &["Go"],
        formatter: "gofmt",
        executable: "gofmt",
        kind: FormatterKind::Gofmt,
    },
    FormatterDefinition {
        languages: &["Python"],
        formatter: "Black",
        executable: "black",
        kind: FormatterKind::Black,
    },
    FormatterDefinition {
        languages: &[
            "TypeScript",
            "JavaScript",
            "JSON",
            "YAML",
            "Markdown",
            "GraphQL",
            "HTML",
            "CSS",
        ],
        formatter: "Prettier",
        executable: "prettier",
        kind: FormatterKind::Prettier,
    },
    FormatterDefinition {
        languages: &["C++", "Protocol Buffers"],
        formatter: "clang-format",
        executable: "clang-format",
        kind: FormatterKind::ClangFormat,
    },
    FormatterDefinition {
        languages: &["Java"],
        formatter: "google-java-format",
        executable: "google-java-format",
        kind: FormatterKind::GoogleJavaFormat,
    },
    FormatterDefinition {
        languages: &["Shell"],
        formatter: "shfmt",
        executable: "shfmt",
        kind: FormatterKind::Shfmt,
    },
    FormatterDefinition {
        languages: &["TOML"],
        formatter: "Taplo",
        executable: "taplo",
        kind: FormatterKind::Taplo,
    },
];

pub(super) fn formatter_plan(
    language: &str,
    source_path: &Path,
    tab_size: u8,
    insert_spaces: bool,
    print_width: u16,
) -> Option<FormatterPlan> {
    let definition = FORMATTERS
        .iter()
        .find(|definition| definition.languages.contains(&language))?;
    let path = source_path.as_os_str().to_owned();
    let width = print_width.to_string();
    let tabs = tab_size.to_string();
    let args = match definition.kind {
        FormatterKind::Rustfmt => strings(&["--emit", "stdout", "--edition", "2024"]),
        FormatterKind::Gofmt => Vec::new(),
        FormatterKind::Black => strings(&["--quiet", "--line-length", &width, "-"]),
        FormatterKind::Prettier => {
            let mut args = strings(&[
                "--stdin-filepath",
                "",
                "--print-width",
                &width,
                "--tab-width",
                &tabs,
            ]);
            args[1] = path;
            args.push(if insert_spaces {
                "--no-use-tabs".into()
            } else {
                "--use-tabs".into()
            });
            args
        }
        FormatterKind::ClangFormat => {
            let mut args = strings(&["--style=file", "--fallback-style=LLVM", "--assume-filename"]);
            args.push(path);
            args
        }
        FormatterKind::GoogleJavaFormat => strings(&["-"]),
        FormatterKind::Shfmt => strings(&["-i", if insert_spaces { tabs.as_str() } else { "0" }]),
        FormatterKind::Taplo => strings(&["format", "-"]),
    };
    Some(FormatterPlan {
        formatter: definition.formatter,
        executable: definition.executable,
        args,
    })
}

pub(super) fn normalize_source(content: &str) -> String {
    let normalized_newlines = content.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::with_capacity(normalized_newlines.len().saturating_add(1));
    for (index, line) in normalized_newlines.lines().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(line.trim_end_matches([' ', '\t']));
    }
    while output.ends_with('\n') {
        output.pop();
    }
    output.push('\n');
    output
}

fn strings(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
