import { constants as fsConstants } from "node:fs";
import { createHmac, randomBytes, randomUUID, timingSafeEqual } from "node:crypto";
import { chmod, lstat, open, rename, unlink } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";

import { openRegularNoFollow, sha256ConfinedFile } from "./confined-files.mjs";
import {
  PUBLIC_PROOF_ENV,
  PUBLIC_PROOF_LIFETIME_MS,
  PUBLIC_PROOF_PURPOSE,
  architectureFacts,
  normalizeHttpsOrigin,
  validateMinimumMacos,
  validateVersion,
} from "./release-metadata.mjs";

function proofKeyBuffer(rawKey) {
  if (typeof rawKey !== "string" || !/^[a-f0-9]{64,}$/i.test(rawKey) || rawKey.length % 2 !== 0) {
    throw new Error(`${PUBLIC_PROOF_ENV} must be at least 32 random bytes encoded as hexadecimal`);
  }
  return Buffer.from(rawKey, "hex");
}

function signClaims(claims, rawKey) {
  return createHmac("sha256", proofKeyBuffer(rawKey)).update(JSON.stringify(claims)).digest("hex");
}

function validateMetadata({ version, target, architecture, minimumMacos, origin }) {
  const facts = architectureFacts(target, architecture);
  return {
    version: validateVersion(version),
    build: version,
    target,
    architecture: facts.display,
    architectures: facts.macho,
    minimumMacos: validateMinimumMacos(minimumMacos),
    origin: normalizeHttpsOrigin(origin),
  };
}

export async function issuePublicProof({
  artifactPath,
  releaseRoot,
  version,
  target,
  architecture,
  minimumMacos,
  origin,
  outputPath,
  proofKey,
  now = Date.now(),
}) {
  const metadata = validateMetadata({ version, target, architecture, minimumMacos, origin });
  const artifact = await sha256ConfinedFile(artifactPath, releaseRoot, { requireDmg: true });
  const claims = {
    schemaVersion: 1,
    purpose: PUBLIC_PROOF_PURPOSE,
    artifactRealPath: artifact.realPath,
    artifactFilename: basename(artifact.absolutePath),
    bytes: artifact.bytes,
    sha256: artifact.sha256,
    ...metadata,
    issuedAt: new Date(now).toISOString(),
    expiresAt: new Date(now + PUBLIC_PROOF_LIFETIME_MS).toISOString(),
    nonce: randomBytes(32).toString("hex"),
  };
  const proof = { claims, signature: signClaims(claims, proofKey) };
  const absoluteOutput = resolve(outputPath);
  const parent = dirname(absoluteOutput);
  if (basename(absoluteOutput) !== "release-proof.json") {
    throw new Error("Release proof output must be named release-proof.json");
  }
  const parentInfo = await lstat(parent);
  if (parentInfo.isSymbolicLink() || !parentInfo.isDirectory()) throw new Error("Proof parent must be a real directory");
  if ((parentInfo.mode & 0o077) !== 0) throw new Error("Proof parent directory must be private to the current user");
  const handle = await open(absoluteOutput, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600);
  try {
    await handle.writeFile(`${JSON.stringify(proof)}\n`, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  await chmod(absoluteOutput, 0o600);
  return absoluteOutput;
}

async function consumeProofFile(proofPath) {
  const absolutePath = resolve(proofPath);
  if (basename(absolutePath) !== "release-proof.json") throw new Error("Release proof must be named release-proof.json");
  const parent = dirname(absolutePath);
  const parentInfo = await lstat(parent);
  if (parentInfo.isSymbolicLink() || !parentInfo.isDirectory() || (parentInfo.mode & 0o077) !== 0) {
    throw new Error("Release proof must be stored in a private real directory");
  }
  const consumingPath = resolve(parent, `.release-proof-consuming-${randomUUID()}`);
  const originalInfo = await lstat(absolutePath);
  if (originalInfo.isSymbolicLink() || !originalInfo.isFile()) throw new Error("Release proof must be a regular file");
  if ((originalInfo.mode & 0o077) !== 0) throw new Error("Release proof must not be readable by group or other users");
  if (originalInfo.size <= 0 || originalInfo.size > 64 * 1024) throw new Error("Release proof has an invalid byte size");
  await rename(absolutePath, consumingPath);
  const opened = await openRegularNoFollow(consumingPath);
  try {
    if (
      opened.stats.dev !== originalInfo.dev ||
      opened.stats.ino !== originalInfo.ino ||
      opened.stats.size !== originalInfo.size
    ) {
      throw new Error("Release proof changed while it was being claimed");
    }
    const text = await opened.handle.readFile("utf8");
    await opened.handle.close();
    return {
      text,
      async commit() {
        await unlink(consumingPath);
      },
      async rollback() {
        await rename(consumingPath, absolutePath);
      },
    };
  } catch (error) {
    await opened.handle.close().catch(() => {});
    await rename(consumingPath, absolutePath).catch(() => {});
    throw error;
  }
}

export async function consumePublicProof({
  proofPath,
  proofKey,
  artifactPath,
  releaseRoot,
  version,
  target,
  architecture,
  minimumMacos,
  origin,
  now = Date.now(),
}) {
  if (!proofPath) throw new Error("Public candidates require --release-proof from the trusted release workflow");
  proofKeyBuffer(proofKey);
  const claimedProof = await consumeProofFile(proofPath);
  let proof;
  try {
    proof = JSON.parse(claimedProof.text);
  } catch {
    await claimedProof.rollback();
    throw new Error("Release proof is not valid JSON");
  }
  try {
    if (!proof || typeof proof !== "object" || !proof.claims || typeof proof.signature !== "string") {
      throw new Error("Release proof has an invalid shape");
    }
    const expectedSignature = Buffer.from(signClaims(proof.claims, proofKey), "hex");
    const actualSignature = /^[a-f0-9]{64}$/i.test(proof.signature)
      ? Buffer.from(proof.signature, "hex")
      : Buffer.alloc(0);
    if (actualSignature.length !== expectedSignature.length || !timingSafeEqual(actualSignature, expectedSignature)) {
      throw new Error("Release proof signature is invalid");
    }

    const metadata = validateMetadata({ version, target, architecture, minimumMacos, origin });
    const artifact = await sha256ConfinedFile(artifactPath, releaseRoot, { requireDmg: true });
    const claims = proof.claims;
    const issuedAt = Date.parse(claims.issuedAt);
    const expiresAt = Date.parse(claims.expiresAt);
    if (
      claims.schemaVersion !== 1 ||
      claims.purpose !== PUBLIC_PROOF_PURPOSE ||
      !Number.isFinite(issuedAt) ||
      !Number.isFinite(expiresAt) ||
      issuedAt > now + 30_000 ||
      expiresAt <= now ||
      expiresAt - issuedAt !== PUBLIC_PROOF_LIFETIME_MS ||
      typeof claims.nonce !== "string" ||
      !/^[a-f0-9]{64}$/i.test(claims.nonce)
    ) {
      throw new Error("Release proof is expired or malformed");
    }

    const expectedClaims = {
      artifactRealPath: artifact.realPath,
      artifactFilename: basename(artifact.absolutePath),
      bytes: artifact.bytes,
      sha256: artifact.sha256,
      ...metadata,
    };
    for (const [key, expected] of Object.entries(expectedClaims)) {
      if (JSON.stringify(claims[key]) !== JSON.stringify(expected)) {
        throw new Error(`Release proof does not match ${key}`);
      }
    }
    await claimedProof.commit();
    return claims;
  } catch (error) {
    await claimedProof.rollback();
    throw error;
  }
}
