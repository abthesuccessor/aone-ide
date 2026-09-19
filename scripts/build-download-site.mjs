import { randomUUID } from "node:crypto";
import {
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { basename, isAbsolute, join, relative, resolve } from "node:path";
import process from "node:process";
import {
  PUBLIC_PROOF_ENV,
  architectureFacts,
  consumePublicProof,
  copyConfinedFileExclusive,
  copyRegularFileExclusive,
  isMainModule,
  normalizeHttpsOrigin,
  parseNamedArguments,
  sha256ConfinedFile,
  validateMinimumMacos,
  validateTarget,
  validateVersion,
} from "./lib/release-common.mjs";

const MAX_ARTIFACT_BYTES = 300 * 1024 * 1024;

function isInside(root, candidate) {
  const pathFromRoot = relative(root, candidate);
  return pathFromRoot === "" || (!pathFromRoot.startsWith("..") && !isAbsolute(pathFromRoot));
}

async function copySymlinkFreeSite(sourceRoot, destinationRoot) {
  const sourceReal = await realpath(sourceRoot);
  async function copyDirectory(sourceDirectory, destinationDirectory, isRoot = false) {
    const directoryInfo = await lstat(sourceDirectory);
    if (directoryInfo.isSymbolicLink() || !directoryInfo.isDirectory()) {
      throw new Error(`Site source contains a non-directory or symbolic link: ${sourceDirectory}`);
    }
    const directoryReal = await realpath(sourceDirectory);
    if (!isInside(sourceReal, directoryReal)) throw new Error("Site source resolves outside its root");

    const entries = await readdir(sourceDirectory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      if (isRoot && entry.name === "downloads") continue;
      const sourcePath = join(sourceDirectory, entry.name);
      const destinationPath = join(destinationDirectory, entry.name);
      const info = await lstat(sourcePath);
      if (info.isSymbolicLink()) throw new Error(`Site source contains a symbolic link: ${sourcePath}`);
      if (info.isDirectory()) {
        await mkdir(destinationPath, { mode: 0o755 });
        await copyDirectory(sourcePath, destinationPath);
      } else if (info.isFile()) {
        await copyRegularFileExclusive(sourcePath, destinationPath);
      } else {
        throw new Error(`Site source contains an unsupported filesystem entry: ${sourcePath}`);
      }
    }
  }
  await copyDirectory(sourceRoot, destinationRoot, true);
}

async function atomicReplaceDirectory(stagedDirectory, outputDirectory, releaseRoot) {
  const backup = join(releaseRoot, `.site-backup-${randomUUID()}`);
  let hasBackup = false;
  let committed = false;
  try {
    const current = await lstat(outputDirectory).catch((error) => {
      if (error?.code === "ENOENT") return null;
      throw error;
    });
    if (current) {
      if (current.isSymbolicLink() || !current.isDirectory()) {
        throw new Error("Existing release site must be a real directory");
      }
      await rename(outputDirectory, backup);
      hasBackup = true;
    }
    await rename(stagedDirectory, outputDirectory);
    committed = true;
  } catch (error) {
    if (hasBackup && !committed) {
      await rm(outputDirectory, { recursive: true, force: true }).catch(() => {});
      try {
        await rename(backup, outputDirectory);
      } catch (restoreError) {
        throw new AggregateError([error, restoreError], "Failed to replace or restore the release site");
      }
    }
    throw error;
  }
  if (hasBackup) await rm(backup, { recursive: true, force: true }).catch(() => {});
}

function normalizeBuildOptions(options) {
  const channel = options.channel ?? "local-test";
  if (!new Set(["local-test", "public-candidate", "public"]).has(channel)) {
    throw new Error("Channel must be local-test, public-candidate, or public");
  }
  const isPublicCandidate = channel !== "local-test";
  const version = validateVersion(options.version);
  const minimumMacos = validateMinimumMacos(options.minimumMacos);
  const target = validateTarget(options.target ?? {
    "Apple Silicon": "aarch64-apple-darwin",
    Intel: "x86_64-apple-darwin",
    Universal: "universal-apple-darwin",
  }[options.architecture]);
  const architecture = architectureFacts(target, options.architecture);
  const origin = isPublicCandidate ? normalizeHttpsOrigin(options.origin) : "";
  if (!isPublicCandidate && (options.origin || options.releaseProof)) {
    throw new Error("Local-test staging must not receive public origin or proof inputs");
  }
  return { channel, isPublicCandidate, version, minimumMacos, target, architecture, origin };
}

export async function buildDownloadSite(options, dependencies = {}) {
  const projectRoot = resolve(options.projectRoot ?? resolve(import.meta.dirname, ".."));
  const releaseRoot = resolve(options.releaseRoot ?? join(projectRoot, "release"));
  const siteSource = resolve(options.siteSource ?? join(projectRoot, "site"));
  const output = resolve(options.output ?? join(releaseRoot, "site"));
  if (output !== join(releaseRoot, "site")) throw new Error("Release site output must be release/site");

  const normalized = normalizeBuildOptions(options);
  const artifact = await sha256ConfinedFile(options.artifactPath, releaseRoot, { requireDmg: true });
  if (artifact.bytes <= 0) throw new Error("Artifact must not be empty");
  if (artifact.bytes >= MAX_ARTIFACT_BYTES) throw new Error("Artifact exceeds the 300 MiB product limit");

  if (normalized.isPublicCandidate) {
    await (dependencies.consumeProof ?? consumePublicProof)({
      proofPath: options.releaseProof,
      proofKey: dependencies.proofKey ?? process.env[PUBLIC_PROOF_ENV],
      artifactPath: artifact.absolutePath,
      releaseRoot,
      version: normalized.version,
      target: normalized.target,
      architecture: normalized.architecture.display,
      minimumMacos: normalized.minimumMacos,
      origin: normalized.origin,
      now: dependencies.now?.() ?? Date.now(),
    });
  }

  const releaseInfo = await lstat(releaseRoot);
  if (releaseInfo.isSymbolicLink() || !releaseInfo.isDirectory()) {
    throw new Error("Release root must be a real directory");
  }
  const siteInfo = await lstat(siteSource);
  if (siteInfo.isSymbolicLink() || !siteInfo.isDirectory()) {
    throw new Error("Site source must be a real directory");
  }

  const stagedDirectory = await mkdtemp(join(releaseRoot, ".site-stage-"));
  try {
    await copySymlinkFreeSite(siteSource, stagedDirectory);
    const downloads = join(stagedDirectory, "downloads");
    await mkdir(downloads, { mode: 0o755 });

    const artifactName = basename(artifact.absolutePath);
    const stagedArtifact = join(downloads, artifactName);
    await copyConfinedFileExclusive(artifact.absolutePath, releaseRoot, stagedArtifact);
    const stagedDigest = await sha256ConfinedFile(stagedArtifact, downloads, { requireDmg: true });
    if (stagedDigest.sha256 !== artifact.sha256 || stagedDigest.bytes !== artifact.bytes) {
      throw new Error("Staged artifact does not match its confined source");
    }

    const url = normalized.isPublicCandidate
      ? `${normalized.origin}/downloads/${encodeURIComponent(artifactName)}`
      : `./downloads/${encodeURIComponent(artifactName)}`;
    const manifest = {
      schemaVersion: 1,
      product: "Aone IDE",
      version: normalized.version,
      build: normalized.version,
      channel: normalized.isPublicCandidate ? "public-candidate" : "local-test",
      publishedAt: new Date(dependencies.now?.() ?? Date.now()).toISOString(),
      artifact: {
        url,
        filename: artifactName,
        bytes: artifact.bytes,
        sha256: artifact.sha256,
        target: normalized.target,
        architecture: normalized.architecture.display,
        architectures: normalized.architecture.macho,
        minimumMacos: normalized.minimumMacos,
        signed: false,
        notarized: false,
        publicReady: false,
      },
    };

    await writeFile(join(downloads, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o644,
    });
    await writeFile(join(downloads, "SHA256SUMS.txt"), `${artifact.sha256}  ${artifactName}\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o644,
    });

    await atomicReplaceDirectory(stagedDirectory, output, releaseRoot);
  } catch (error) {
    await rm(stagedDirectory, { recursive: true, force: true }).catch(() => {});
    throw error;
  }

  return output;
}

async function cli() {
  const args = parseNamedArguments(process.argv.slice(2), {
    values: [
      "--artifact",
      "--version",
      "--target",
      "--architecture",
      "--minimum-macos",
      "--channel",
      "--origin",
      "--release-proof",
    ],
  });
  if (args.positionals.length) throw new Error("Unexpected positional arguments");
  const output = await buildDownloadSite({
    artifactPath: args.required("--artifact"),
    version: args.required("--version"),
    target: args.optional("--target"),
    architecture: args.required("--architecture"),
    minimumMacos: args.required("--minimum-macos"),
    channel: args.optional("--channel", "local-test"),
    origin: args.optional("--origin", ""),
    releaseProof: args.optional("--release-proof"),
  });
  process.stdout.write(`${output}\n`);
}

if (isMainModule(import.meta.url)) {
  cli().catch((error) => {
    process.stderr.write(`build-download-site: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
