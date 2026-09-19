import { lstat, readFile, readdir } from "node:fs/promises";
import { basename, dirname, extname, join, relative, resolve, sep } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const PROJECT_ROOT = resolve(import.meta.dirname, "..");
const SOURCE_ROOTS = ["src", "src-tauri/src", "scripts", "openapi", "asyncapi"];
const HANDWRITTEN_EXTENSIONS = new Set([
  ".rs",
  ".ts",
  ".tsx",
  ".mjs",
  ".css",
  ".yaml",
  ".yml",
  ".json",
]);
const MAX_LINES = 500;
const DOCUMENTATION_EXCLUSIONS = ["src/generated"];

function displayPath(path) {
  return relative(PROJECT_ROOT, path).split(sep).join("/") || ".";
}

function physicalLineCount(source) {
  if (source.length === 0) return 0;
  const lines = source.split(/\r\n|\r|\n/).length;
  return /(?:\r\n|\r|\n)$/.test(source) ? lines - 1 : lines;
}

async function collectHandwrittenFiles(root, failures) {
  const files = [];

  async function visit(directory) {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        failures.push(`${displayPath(path)}: symbolic links are not allowed in checked source roots`);
      } else if (entry.isDirectory()) {
        await visit(path);
      } else if (entry.isFile() && HANDWRITTEN_EXTENSIONS.has(extname(entry.name))) {
        files.push(path);
      }
    }
  }

  await visit(root);
  return files;
}

function stripRustComments(source) {
  let output = "";
  let index = 0;
  let blockDepth = 0;
  while (index < source.length) {
    const current = source[index];
    const next = source[index + 1];
    if (blockDepth > 0) {
      if (current === "/" && next === "*") {
        blockDepth += 1;
        output += "  ";
        index += 2;
      } else if (current === "*" && next === "/") {
        blockDepth -= 1;
        output += "  ";
        index += 2;
      } else {
        output += current === "\n" || current === "\r" ? current : " ";
        index += 1;
      }
      continue;
    }
    if (current === "/" && next === "*") {
      blockDepth = 1;
      output += "  ";
      index += 2;
      continue;
    }
    if (current === "/" && next === "/") {
      while (index < source.length && !["\n", "\r"].includes(source[index])) {
        output += " ";
        index += 1;
      }
      continue;
    }
    output += current;
    index += 1;
  }
  if (blockDepth !== 0) throw new Error("unterminated block comment");
  return output;
}

function consumeAttribute(source, start) {
  if (source[start] !== "#" || source[start + 1] !== "[") return null;
  let depth = 1;
  let index = start + 2;
  while (index < source.length && depth > 0) {
    if (source[index] === "[") depth += 1;
    if (source[index] === "]") depth -= 1;
    index += 1;
  }
  if (depth !== 0) throw new Error("unterminated attribute");
  return { text: source.slice(start, index), end: index };
}

function isAllowedAttribute(attribute, statementKind) {
  const name = attribute.match(/^#\[\s*([A-Za-z_][A-Za-z0-9_]*)/)?.[1];
  if (statementKind === "module") return name === "cfg" || name === "cfg_attr";
  return ["allow", "cfg", "cfg_attr"].includes(name);
}

function validateRustModuleIndex(path, source, failures) {
  let stripped;
  try {
    stripped = stripRustComments(source);
  } catch (error) {
    failures.push(`${displayPath(path)}: ${error.message}`);
    return;
  }

  let cursor = 0;
  while (cursor < stripped.length) {
    while (/\s/.test(stripped[cursor] ?? "")) cursor += 1;
    if (cursor >= stripped.length) break;
    const statementStart = cursor;
    const attributes = [];
    while (stripped.startsWith("#[", cursor)) {
      try {
        const attribute = consumeAttribute(stripped, cursor);
        attributes.push(attribute.text);
        cursor = attribute.end;
        while (/\s/.test(stripped[cursor] ?? "")) cursor += 1;
      } catch (error) {
        failures.push(`${displayPath(path)}: ${error.message}`);
        return;
      }
    }

    const rest = stripped.slice(cursor);
    const moduleMatch = rest.match(/^(?:(?:pub(?:\s*\([^)]*\))?)\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;/);
    if (moduleMatch) {
      if (attributes.some((attribute) => !isAllowedAttribute(attribute, "module"))) {
        failures.push(`${displayPath(path)}: only cfg attributes may guard a module declaration`);
      }
      cursor += moduleMatch[0].length;
      continue;
    }

    const useMatch = rest.match(/^(?:(?:pub(?:\s*\([^)]*\))?)\s+)?use\b/);
    if (useMatch) {
      if (attributes.some((attribute) => !isAllowedAttribute(attribute, "use"))) {
        failures.push(`${displayPath(path)}: unsupported attribute on a use declaration`);
      }
      const semicolon = rest.indexOf(";");
      if (semicolon < 0) {
        failures.push(`${displayPath(path)}: unterminated use declaration`);
        return;
      }
      cursor += semicolon + 1;
      continue;
    }

    const line = physicalLineCount(stripped.slice(0, statementStart)) + 1;
    const preview = rest.split(/\r?\n/, 1)[0].trim().slice(0, 100);
    failures.push(
      `${displayPath(path)}:${line}: mod.rs may contain only module declarations and use re-exports; found ${preview || "unsupported syntax"}`,
    );
    return;
  }
}

async function validateFeatureReadme(modPath, failures) {
  const readme = join(dirname(modPath), "README.md");
  try {
    const info = await lstat(readme);
    if (info.isSymbolicLink() || !info.isFile()) throw new Error("not a regular file");
  } catch {
    failures.push(`${displayPath(dirname(modPath))}: feature directory must contain a regular README.md`);
  }
}

function isDocumentationExcluded(path) {
  const projectPath = displayPath(path);
  return DOCUMENTATION_EXCLUSIONS.some(
    (excluded) => projectPath === excluded || projectPath.startsWith(`${excluded}/`),
  );
}

async function validateSourceDirectoryReadmes(root, failures) {
  async function visit(directory) {
    if (isDocumentationExcluded(directory)) return;
    const entries = await readdir(directory, { withFileTypes: true });
    const hasDirectSource = entries.some(
      (entry) => entry.isFile() && HANDWRITTEN_EXTENSIONS.has(extname(entry.name)),
    );
    if (hasDirectSource) {
      const readme = join(directory, "README.md");
      try {
        const info = await lstat(readme);
        if (info.isSymbolicLink() || !info.isFile()) throw new Error("not a regular file");
      } catch {
        failures.push(
          `${displayPath(directory)}: source module directory must contain a regular README.md`,
        );
      }
    }
    for (const entry of entries) {
      if (entry.isDirectory()) await visit(join(directory, entry.name));
    }
  }

  await visit(root);
}

export async function checkStructure() {
  const failures = [];
  const files = [];
  for (const root of SOURCE_ROOTS) {
    const absoluteRoot = join(PROJECT_ROOT, root);
    files.push(...await collectHandwrittenFiles(absoluteRoot, failures));
    await validateSourceDirectoryReadmes(absoluteRoot, failures);
  }
  files.sort();

  let featureCount = 0;
  for (const path of files) {
    const source = await readFile(path, "utf8");
    const lines = physicalLineCount(source);
    if (lines > MAX_LINES) {
      failures.push(`${displayPath(path)}: ${lines} lines exceeds the ${MAX_LINES}-line limit`);
    }
    if (basename(path) === "mod.rs") {
      featureCount += 1;
      validateRustModuleIndex(path, source, failures);
      await validateFeatureReadme(path, failures);
    }
  }

  if (failures.length > 0) {
    throw new Error(`structure check failed:\n- ${failures.join("\n- ")}`);
  }
  return { fileCount: files.length, featureCount, maxLines: MAX_LINES };
}

const isMainModule =
  Boolean(process.argv[1]) &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href;

if (isMainModule) {
  checkStructure()
    .then(({ fileCount, featureCount, maxLines }) => {
      process.stdout.write(
        `structure check passed: ${fileCount} handwritten files, ${featureCount} Rust features, ${maxLines}-line maximum\n`,
      );
    })
    .catch((error) => {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
      process.exitCode = 1;
    });
}
