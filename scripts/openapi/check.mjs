import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

import {
  generatedRoot,
  generatorDigest,
  generatorName,
  generatorVersion,
  hashTree,
  isGeneratedContractFile,
  isSpecificationFile,
  readContractManifest,
  repositoryRoot,
  sortedUnique,
  specificationPath,
  specificationRoot,
  typescriptMajorVersion,
  typescriptVersion,
} from "./contract.mjs";

function assertEqual(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} mismatch: expected ${expected}, received ${actual}`);
}

function commandNamesFromSpecification(source) {
  return sortedUnique([...source.matchAll(/^\s*x-aone-tauri-command:\s*([a-z][a-z0-9_]*)\s*$/gm)]
    .map((match) => match[1]));
}

function commandNamesFromRust(source) {
  const handler = source.match(/generate_handler!\[([\s\S]*?)\]\)/)?.[1];
  if (!handler) throw new Error("Could not find Tauri generate_handler command registry");
  return sortedUnique([...handler.matchAll(/(?:[a-z_][a-z0-9_]*::)+([a-z][a-z0-9_]*)/g)]
    .map((match) => match[1]));
}

async function readSpecificationTree(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const sources = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) sources.push(await readSpecificationTree(path));
    else if (entry.isFile() && /\.ya?ml$/i.test(entry.name)) sources.push(await readFile(path, "utf8"));
  }
  return sources.join("\n");
}

const manifest = await readContractManifest();
assertEqual(manifest.schemaVersion, 2, "contract manifest schema");
assertEqual(manifest.generator, "OpenAPI Generator", "generator name");
assertEqual(manifest.generatorVersion, generatorVersion, "generator version");
assertEqual(manifest.generatorName, generatorName, "generator name");
assertEqual(manifest.generatorDigest, generatorDigest, "generator image digest");
assertEqual(manifest.typescriptVersion, typescriptVersion, "generated TypeScript version");
assertEqual(manifest.typescriptMajorVersion, typescriptMajorVersion, "generated TypeScript major");

const packageJson = JSON.parse(await readFile(resolve(repositoryRoot, "package.json"), "utf8"));
const configuredTypeScript = packageJson.devDependencies?.typescript;
if (configuredTypeScript !== typescriptVersion) {
  throw new Error(`package.json must pin TypeScript ${typescriptVersion}; received ${configuredTypeScript}`);
}

const sourceSha256 = await hashTree(specificationRoot, isSpecificationFile);
const generatedSha256 = await hashTree(generatedRoot, isGeneratedContractFile);
assertEqual(manifest.sourceSha256, sourceSha256, "OpenAPI source digest");
assertEqual(manifest.generatedSha256, generatedSha256, "generated contract digest");

const specification = await readFile(specificationPath, "utf8");
if (!specification.startsWith("openapi: 3.1.")) throw new Error("IPC contract must remain OpenAPI 3.1");
const rust = await readFile(resolve(repositoryRoot, "src-tauri/src/lib.rs"), "utf8");
const documentedCommands = commandNamesFromSpecification(await readSpecificationTree(specificationRoot));
const registeredCommands = commandNamesFromRust(rust);
assertEqual(documentedCommands.join(","), registeredCommands.join(","), "documented Tauri commands");

process.stdout.write(
  `OpenAPI contract verified: ${documentedCommands.length} commands, TypeScript ${typescriptVersion}, ${generatedSha256}\n`,
);
