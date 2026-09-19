import { randomUUID } from "node:crypto";
import { open, rename, rm } from "node:fs/promises";
import { join, resolve } from "node:path";

import {
  readConfinedRegularFile,
  sha256ConfinedFile,
} from "./confined-files.mjs";
import {
  architectureFacts,
  normalizeHttpsOrigin,
  validateArtifactFilename,
  validateMinimumMacos,
  validateTarget,
  validateVersion,
} from "./release-metadata.mjs";
import {
  assertRealDirectory,
  sameStringSet,
  validateExpectedTeamId,
  verifyPublicAppleArtifact,
} from "./apple-verification.mjs";

export const DEFAULT_MAX_BYTES = 300 * 1024 * 1024;

async function writeJsonAtomic(path, value, parentDirectory) {
  const temporaryPath = join(parentDirectory, `.manifest-${randomUUID()}.tmp`);
  const handle = await open(temporaryPath, "wx", 0o644);
  try {
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  try {
    await rename(temporaryPath, path);
  } catch (error) {
    await rm(temporaryPath, { force: true });
    throw error;
  }
}

function validateManifestShape(manifest) {
  if (!manifest || typeof manifest !== "object" || manifest.schemaVersion !== 1 || manifest.product !== "Aone IDE") {
    throw new Error("Release manifest has an unsupported schema or product");
  }
  validateVersion(manifest.version);
  if (manifest.build !== manifest.version) throw new Error("Manifest build must exactly match its version");
  if (!Number.isFinite(Date.parse(manifest.publishedAt))) throw new Error("Manifest publishedAt is invalid");
  const artifact = manifest.artifact;
  if (!artifact || typeof artifact !== "object") throw new Error("Release manifest artifact is missing");
  validateArtifactFilename(artifact.filename);
  if (typeof artifact.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(artifact.sha256)) {
    throw new Error("Manifest artifact checksum is invalid");
  }
  if (!Number.isSafeInteger(artifact.bytes) || artifact.bytes <= 0) throw new Error("Manifest artifact byte size is invalid");
  const facts = architectureFacts(artifact.target, artifact.architecture);
  if (!sameStringSet(artifact.architectures, facts.macho)) throw new Error("Manifest architecture set is inconsistent");
  validateMinimumMacos(artifact.minimumMacos);
  for (const key of ["signed", "notarized", "publicReady"]) {
    if (typeof artifact[key] !== "boolean") throw new Error(`Manifest ${key} value must be boolean`);
  }
  return { artifact, facts };
}

function validatePublicExpectations(manifest, options, facts) {
  const expectedVersion = validateVersion(options.expectedVersion, "expected version");
  const expectedTarget = validateTarget(options.expectedTarget);
  const expectedFacts = architectureFacts(expectedTarget, options.expectedArchitecture);
  const expectedMinimumMacos = validateMinimumMacos(options.expectedMinimumMacos);
  const expectedOrigin = normalizeHttpsOrigin(options.origin);
  if (manifest.version !== expectedVersion || manifest.build !== expectedVersion) {
    throw new Error("Manifest version/build does not match the explicit release expectation");
  }
  if (
    manifest.artifact.target !== expectedTarget ||
    facts.display !== expectedFacts.display ||
    !sameStringSet(facts.macho, expectedFacts.macho)
  ) {
    throw new Error("Manifest architecture does not match the explicit release expectation");
  }
  if (manifest.artifact.minimumMacos !== expectedMinimumMacos) {
    throw new Error("Manifest minimum macOS does not match the explicit release expectation");
  }
  const expectedArtifactUrl = `${expectedOrigin}/downloads/${encodeURIComponent(manifest.artifact.filename)}`;
  if (manifest.artifact.url !== expectedArtifactUrl) throw new Error("Public artifact URL does not match the exact HTTPS origin");
  return { expectedVersion, expectedTarget, expectedFacts, expectedMinimumMacos, expectedOrigin };
}

export async function verifyRelease(options = {}, dependencies = {}) {
  const projectRoot = resolve(options.projectRoot ?? resolve(import.meta.dirname, "../.."));
  const releaseRoot = resolve(options.releaseRoot ?? join(projectRoot, "release", "site"));
  const downloads = join(releaseRoot, "downloads");
  await assertRealDirectory(releaseRoot);
  await assertRealDirectory(downloads, releaseRoot);

  const manifestPath = join(downloads, "manifest.json");
  const checksumPath = join(downloads, "SHA256SUMS.txt");
  const [manifestBytes, checksumBytes] = await Promise.all([
    readConfinedRegularFile(manifestPath, downloads),
    readConfinedRegularFile(checksumPath, downloads),
  ]);
  if (manifestBytes.length > 1024 * 1024) throw new Error("Release manifest is unexpectedly large");
  let manifest;
  try {
    manifest = JSON.parse(manifestBytes.toString("utf8"));
  } catch {
    throw new Error("Release manifest is not valid JSON");
  }
  const { artifact, facts } = validateManifestShape(manifest);
  const artifactPath = join(downloads, artifact.filename);
  const inspected = await sha256ConfinedFile(artifactPath, downloads, { requireDmg: true });
  const maxBytes = options.maxBytes ?? DEFAULT_MAX_BYTES;
  if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) throw new Error("Maximum artifact bytes must be a positive integer");
  if (inspected.sha256 !== artifact.sha256) throw new Error("Artifact checksum does not match the release manifest");
  if (inspected.bytes !== artifact.bytes) throw new Error("Artifact byte size does not match the release manifest");
  if (inspected.bytes >= maxBytes) throw new Error(`Artifact exceeds the configured ${maxBytes} byte limit`);
  const expectedChecksumFile = `${inspected.sha256}  ${artifact.filename}\n`;
  if (checksumBytes.toString("utf8") !== expectedChecksumFile) {
    throw new Error("SHA256SUMS.txt must contain exactly the manifest digest and filename on one line");
  }

  if (!options.publicMode) {
    if (manifest.channel !== "local-test") throw new Error("Integrity-only verification requires a local-test manifest");
    if (artifact.signed || artifact.notarized || artifact.publicReady) {
      throw new Error("Local-test manifests must keep every public trust flag false");
    }
    const expectedLocalUrl = `./downloads/${encodeURIComponent(artifact.filename)}`;
    if (artifact.url !== expectedLocalUrl) throw new Error("Local artifact URL must be a confined relative download URL");
    return { manifest, artifactPath, sha256: inspected.sha256, bytes: inspected.bytes };
  }

  if (!new Set(["public-candidate", "public"]).has(manifest.channel)) {
    throw new Error("Public verification requires a public-candidate manifest");
  }
  const flags = [artifact.signed, artifact.notarized, artifact.publicReady];
  if (!(flags.every(Boolean) || flags.every((value) => !value))) {
    throw new Error("Public trust flags must change together");
  }
  const expectations = validatePublicExpectations(manifest, options, facts);
  const expectedTeamId = validateExpectedTeamId(options.expectedTeamId);
  if (typeof options.expectedSigningIdentity !== "string" || !options.expectedSigningIdentity) {
    throw new Error("--expected-signing-identity is required for public verification");
  }
  if (options.expectedBundleId !== "com.aone.ide") throw new Error("--expected-bundle-id must be com.aone.ide");
  if (options.expectedExecutable !== "aone-ide") throw new Error("--expected-executable must be aone-ide");

  const { verification: _previousVerification, verifiedAt: _previousVerifiedAt, ...unverifiedManifest } = manifest;
  const candidateManifest = {
    ...unverifiedManifest,
    channel: "public-candidate",
    artifact: {
      ...artifact,
      signed: false,
      notarized: false,
      publicReady: false,
    },
  };
  await writeJsonAtomic(manifestPath, candidateManifest, downloads);

  const appleVerifier = dependencies.appleVerifier ?? verifyPublicAppleArtifact;
  const apple = await appleVerifier({
    artifactPath,
    expectedVersion: expectations.expectedVersion,
    expectedTarget: expectations.expectedTarget,
    expectedArchitecture: expectations.expectedFacts.display,
    expectedMinimumMacos: expectations.expectedMinimumMacos,
    expectedTeamId,
    expectedSigningIdentity: options.expectedSigningIdentity,
    expectedBundleId: options.expectedBundleId,
    expectedExecutable: options.expectedExecutable,
  });
  if (
    apple.bundleIdentifier !== options.expectedBundleId ||
    apple.version !== expectations.expectedVersion ||
    apple.build !== expectations.expectedVersion ||
    apple.executable !== options.expectedExecutable ||
    apple.target !== expectations.expectedTarget ||
    apple.architecture !== expectations.expectedFacts.display ||
    !sameStringSet(apple.architectures, expectations.expectedFacts.macho) ||
    apple.minimumMacos !== expectations.expectedMinimumMacos ||
    apple.teamId !== expectedTeamId ||
    apple.signingIdentity !== options.expectedSigningIdentity
  ) {
    throw new Error("Apple verification result does not match the explicit public release expectations");
  }
  const finalArtifact = await sha256ConfinedFile(artifactPath, downloads, { requireDmg: true });
  if (finalArtifact.sha256 !== inspected.sha256 || finalArtifact.bytes !== inspected.bytes) {
    throw new Error("Staged artifact changed during Apple verification");
  }

  const verifiedManifest = {
    ...candidateManifest,
    channel: "public",
    verifiedAt: new Date(dependencies.now?.() ?? Date.now()).toISOString(),
    artifact: {
      ...artifact,
      signed: true,
      notarized: true,
      publicReady: true,
    },
    verification: {
      provider: "Apple",
      ...apple,
    },
  };
  await writeJsonAtomic(manifestPath, verifiedManifest, downloads);
  return { manifest: verifiedManifest, artifactPath, sha256: inspected.sha256, bytes: inspected.bytes };
}
