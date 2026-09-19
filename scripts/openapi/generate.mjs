import { spawnSync } from "node:child_process";
import { cp, mkdir, readdir, rename, rm, writeFile } from "node:fs/promises";
import { basename, relative, resolve, sep } from "node:path";

import {
  assertContained,
  generatedParent,
  generatedRoot,
  generatorDigest,
  generatorImage,
  generatorName,
  generatorVersion,
  hashTree,
  isGeneratedContractFile,
  isSpecificationFile,
  manifestName,
  repositoryRoot,
  specificationRoot,
  typescriptMajorVersion,
  typescriptVersion,
} from "./contract.mjs";

const stageRoot = assertContained(generatedParent, resolve(generatedParent, `.ipc-stage-${process.pid}`));
const rawRoot = assertContained(stageRoot, resolve(stageRoot, "raw"));
const cleanRoot = assertContained(stageRoot, resolve(stageRoot, "clean"));
const backupRoot = assertContained(generatedParent, resolve(generatedParent, `.ipc-backup-${process.pid}`));

async function collectTypeScriptFiles(directory, root = directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await collectTypeScriptFiles(path, root));
    else if (entry.isFile() && entry.name.endsWith(".ts")) files.push(relative(root, path));
  }
  return files.sort((left, right) => left.localeCompare(right));
}

async function prepareGeneratedTree() {
  for (const relativePath of await collectTypeScriptFiles(rawRoot)) {
    const source = assertContained(rawRoot, resolve(rawRoot, relativePath));
    const normalizedPath = relativePath.startsWith(`src${sep}`)
      ? relativePath.slice(`src${sep}`.length)
      : relativePath;
    const destination = assertContained(cleanRoot, resolve(cleanRoot, normalizedPath));
    await mkdir(resolve(destination, ".."), { recursive: true });
    await cp(source, destination);
  }
  await writeFile(resolve(cleanRoot, "README.md"), [
    "# Generated Aone IPC contract",
    "",
    "Generated from `openapi/aone-ipc.openapi.yaml` by the pinned official",
    `OpenAPI Generator ${generatorVersion} ${generatorName} generator, verified with TypeScript ${typescriptVersion}.`,
    "Do not edit these files by hand; run `npm run openapi:generate`.",
    "",
  ].join("\n"));
}

async function installAtomically() {
  await rm(backupRoot, { recursive: true, force: true });
  let backedUp = false;
  try {
    await rename(generatedRoot, backupRoot);
    backedUp = true;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  try {
    await rename(cleanRoot, generatedRoot);
    if (backedUp) await rm(backupRoot, { recursive: true, force: true });
  } catch (error) {
    if (backedUp) await rename(backupRoot, generatedRoot);
    throw error;
  }
}

await mkdir(rawRoot, { recursive: true });
const uid = typeof process.getuid === "function" ? process.getuid() : 1000;
const gid = typeof process.getgid === "function" ? process.getgid() : 1000;
const outputWithinMount = `/local/${relative(repositoryRoot, rawRoot).split(sep).join("/")}`;
const result = spawnSync("docker", [
  "run", "--rm", "--user", `${uid}:${gid}`,
  "-v", `${repositoryRoot}:/local`,
  generatorImage,
  "generate",
  "-i", "/local/openapi/aone-ipc.openapi.yaml",
  "-g", generatorName,
  "-o", outputWithinMount,
  "--additional-properties=supportsES6=true,stringEnums=true,npmName=aone-ipc-contract,npmVersion=0.1.0",
  "--global-property=models,supportingFiles,modelDocs=false,modelTests=false",
], { encoding: "utf8", stdio: "inherit" });

if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`OpenAPI Generator exited with status ${result.status}`);

await mkdir(cleanRoot, { recursive: true });
await prepareGeneratedTree();
const sourceSha256 = await hashTree(specificationRoot, isSpecificationFile);
const generatedSha256 = await hashTree(cleanRoot, isGeneratedContractFile);
await writeFile(resolve(cleanRoot, manifestName), `${JSON.stringify({
  schemaVersion: 2,
  generator: "OpenAPI Generator",
  generatorVersion,
  generatorName,
  generatorDigest,
  typescriptVersion,
  typescriptMajorVersion,
  sourceSha256,
  generatedSha256,
}, null, 2)}\n`);
await installAtomically();
await rm(stageRoot, { recursive: true, force: true });

process.stdout.write(`generated ${basename(generatedRoot)} contract ${generatedSha256}\n`);
