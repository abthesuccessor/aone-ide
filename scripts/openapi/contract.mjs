import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const specificationRoot = resolve(repositoryRoot, "openapi");
export const specificationPath = resolve(specificationRoot, "aone-ipc.openapi.yaml");
export const generatedParent = resolve(repositoryRoot, "src/generated");
export const generatedRoot = resolve(generatedParent, "ipc");
export const manifestName = "contract-manifest.json";
export const generatorVersion = "7.24.0";
export const generatorName = "typescript-fetch";
export const generatorDigest = "sha256:5bf3dc75f764c584da8e3344c51b2f3f1e74703461d46a035b5ac1d31515cc88";
export const generatorImage = `openapitools/openapi-generator-cli@${generatorDigest}`;
export const typescriptVersion = "7.0.2";
export const typescriptMajorVersion = Number.parseInt(typescriptVersion, 10);

export function assertContained(parent, candidate) {
  const canonicalParent = resolve(parent);
  const canonicalCandidate = resolve(candidate);
  if (canonicalCandidate === canonicalParent || canonicalCandidate.startsWith(`${canonicalParent}${sep}`)) {
    return canonicalCandidate;
  }
  throw new Error(`Path escapes ${canonicalParent}: ${canonicalCandidate}`);
}

async function collectFiles(root, include, directory = root) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await collectFiles(root, include, path));
    else if (entry.isFile() && include(path)) files.push(path);
  }
  return files.sort((left, right) => relative(root, left).localeCompare(relative(root, right)));
}

export async function hashTree(root, include = () => true) {
  const digest = createHash("sha256");
  for (const path of await collectFiles(root, include)) {
    digest.update(relative(root, path).split(sep).join("/"));
    digest.update("\0");
    digest.update(await readFile(path));
    digest.update("\0");
  }
  return digest.digest("hex");
}

export function isSpecificationFile(path) {
  return path.endsWith(".yaml") || path.endsWith(".yml") || path.endsWith(".json");
}

export function isGeneratedContractFile(path) {
  return !path.endsWith(manifestName);
}

export async function readContractManifest() {
  return JSON.parse(await readFile(resolve(generatedRoot, manifestName), "utf8"));
}

export function sortedUnique(values) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}
