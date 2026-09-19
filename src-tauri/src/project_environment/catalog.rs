#[derive(Debug, Clone, Copy)]
pub(super) struct ToolSpec {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) category: &'static str,
    pub(super) names: &'static [&'static str],
    pub(super) version_args: &'static [&'static str],
}

macro_rules! tool {
    ($id:literal, $label:literal, $category:literal, [$($name:literal),+], [$($arg:literal),*]) => {
        ToolSpec {
            id: $id,
            label: $label,
            category: $category,
            names: &[$($name),+],
            version_args: &[$($arg),*],
        }
    };
}

pub(super) static TOOL_CATALOG: &[ToolSpec] = &[
    tool!("node", "Node.js", "JavaScript", ["node"], ["--version"]),
    tool!("npm", "npm", "JavaScript", ["npm"], ["--version"]),
    tool!("pnpm", "pnpm", "JavaScript", ["pnpm"], ["--version"]),
    tool!("yarn", "Yarn", "JavaScript", ["yarn"], ["--version"]),
    tool!("bun", "Bun", "JavaScript", ["bun"], ["--version"]),
    tool!("deno", "Deno", "JavaScript", ["deno"], ["--version"]),
    tool!(
        "python",
        "Python",
        "Python",
        ["python3", "python"],
        ["--version"]
    ),
    tool!("pip", "pip", "Python", ["pip3", "pip"], ["--version"]),
    tool!("uv", "uv", "Python", ["uv"], ["--version"]),
    tool!("poetry", "Poetry", "Python", ["poetry"], ["--version"]),
    tool!("cargo", "Cargo", "Rust", ["cargo"], ["--version"]),
    tool!("rustc", "Rust compiler", "Rust", ["rustc"], ["--version"]),
    tool!("go", "Go", "Go", ["go"], ["version"]),
    tool!("java", "Java runtime", "JVM", ["java"], ["--version"]),
    tool!("javac", "Java compiler", "JVM", ["javac"], ["-version"]),
    tool!("maven", "Maven", "JVM", ["mvn"], ["--version"]),
    tool!("gradle", "Gradle", "JVM", ["gradle"], ["--version"]),
    tool!("dotnet", ".NET SDK", ".NET", ["dotnet"], ["--version"]),
    tool!("ruby", "Ruby", "Ruby", ["ruby"], ["--version"]),
    tool!("bundler", "Bundler", "Ruby", ["bundle"], ["--version"]),
    tool!("php", "PHP", "PHP", ["php"], ["--version"]),
    tool!("composer", "Composer", "PHP", ["composer"], ["--version"]),
    tool!("swift", "Swift", "Swift", ["swift"], ["--version"]),
    tool!("kotlin", "Kotlin", "JVM", ["kotlin"], ["-version"]),
    tool!(
        "kotlinc",
        "Kotlin compiler",
        "JVM",
        ["kotlinc"],
        ["-version"]
    ),
    tool!(
        "clang",
        "Clang C compiler",
        "Native",
        ["clang"],
        ["--version"]
    ),
    tool!(
        "clangxx",
        "Clang C++ compiler",
        "Native",
        ["clang++"],
        ["--version"]
    ),
    tool!("gcc", "GCC C compiler", "Native", ["gcc"], ["--version"]),
    tool!("gxx", "GCC C++ compiler", "Native", ["g++"], ["--version"]),
    tool!("cmake", "CMake", "Native", ["cmake"], ["--version"]),
    tool!("ninja", "Ninja", "Native", ["ninja"], ["--version"]),
    tool!("make", "Make", "Native", ["make"], ["--version"]),
    tool!("docker", "Docker", "Containers", ["docker"], ["--version"]),
    tool!(
        "docker-compose",
        "Docker Compose",
        "Containers",
        ["docker"],
        ["compose", "version"]
    ),
    tool!(
        "terraform",
        "Terraform",
        "Infrastructure",
        ["terraform"],
        ["version"]
    ),
    tool!("git", "Git", "Source control", ["git"], ["--version"]),
    tool!("zig", "Zig", "Native", ["zig"], ["version"]),
    tool!("dart", "Dart", "Dart", ["dart"], ["--version"]),
    tool!("flutter", "Flutter", "Dart", ["flutter"], ["--version"]),
];

pub(super) fn tool_spec(id: &str) -> Option<&'static ToolSpec> {
    TOOL_CATALOG.iter().find(|spec| spec.id == id)
}
