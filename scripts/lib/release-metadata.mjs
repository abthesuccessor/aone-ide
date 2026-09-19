import { basename, extname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import process from "node:process";

export const PUBLIC_PROOF_ENV = "AONE_RELEASE_PROOF_KEY";
export const PUBLIC_PROOF_PURPOSE = "aone-public-release-staging";
export const PUBLIC_PROOF_LIFETIME_MS = 10 * 60 * 1000;

const ARCHITECTURES_BY_TARGET = Object.freeze({
  "aarch64-apple-darwin": ["arm64"],
  "x86_64-apple-darwin": ["x86_64"],
  "universal-apple-darwin": ["arm64", "x86_64"],
});

const DISPLAY_BY_TARGET = Object.freeze({
  "aarch64-apple-darwin": "Apple Silicon",
  "x86_64-apple-darwin": "Intel",
  "universal-apple-darwin": "Universal",
});

export function isMainModule(metaUrl) {
  return Boolean(process.argv[1]) && metaUrl === pathToFileURL(resolve(process.argv[1])).href;
}

export function parseNamedArguments(argv, { flags = [], values: allowedValues = undefined } = {}) {
  const flagSet = new Set(flags);
  const allowedSet = allowedValues ? new Set([...allowedValues, ...flags]) : null;
  const values = new Map();
  const positionals = [];

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      positionals.push(token);
      continue;
    }
    if (allowedSet && !allowedSet.has(token)) throw new Error(`Unknown argument ${token}`);
    if (values.has(token)) throw new Error(`Duplicate argument ${token}`);
    if (flagSet.has(token)) {
      values.set(token, true);
      continue;
    }
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) throw new Error(`Missing value for ${token}`);
    values.set(token, value);
    index += 1;
  }

  return {
    has(name) {
      return values.has(name);
    },
    optional(name, fallback = undefined) {
      return values.has(name) ? values.get(name) : fallback;
    },
    required(name) {
      const value = values.get(name);
      if (typeof value !== "string" || value.length === 0) throw new Error(`Missing required argument ${name}`);
      return value;
    },
    positionals,
  };
}

export function normalizeHttpsOrigin(rawOrigin) {
  if (typeof rawOrigin !== "string" || rawOrigin.length === 0) {
    throw new Error("Public releases require an HTTPS origin");
  }
  if (rawOrigin !== rawOrigin.trim() || rawOrigin.includes("\\") || !rawOrigin.startsWith("https://")) {
    throw new Error("Public release origin must use canonical https:// URL syntax");
  }

  let parsed;
  try {
    parsed = new URL(rawOrigin);
  } catch {
    throw new Error("Public release origin must be a valid absolute URL");
  }

  if (parsed.protocol !== "https:") throw new Error("Public release origin must use HTTPS");
  if (!parsed.hostname) throw new Error("Public release origin must include a hostname");
  if (parsed.username || parsed.password) throw new Error("Public release origin must not contain credentials");
  if (parsed.pathname !== "/") throw new Error("Public release origin must not contain a path");
  if (parsed.search) throw new Error("Public release origin must not contain a query");
  if (parsed.hash) throw new Error("Public release origin must not contain a fragment");

  return parsed.origin;
}

export function validateVersion(value, label = "version") {
  if (typeof value !== "string" || !/^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$/.test(value)) {
    throw new Error(`${label} must be a semantic version`);
  }
  return value;
}

export function validateMinimumMacos(value) {
  if (typeof value !== "string" || !/^[0-9]+\.[0-9]+(?:\.[0-9]+)?$/.test(value)) {
    throw new Error("minimum macOS must be a numeric version");
  }
  return value;
}

export function validateTarget(target) {
  if (!Object.hasOwn(ARCHITECTURES_BY_TARGET, target)) throw new Error(`Unsupported macOS target: ${target}`);
  return target;
}

export function architectureFacts(target, displayArchitecture = undefined) {
  validateTarget(target);
  const expectedDisplay = DISPLAY_BY_TARGET[target];
  if (displayArchitecture !== undefined && displayArchitecture !== expectedDisplay) {
    throw new Error(`Architecture ${displayArchitecture} does not match target ${target}`);
  }
  return {
    display: expectedDisplay,
    macho: [...ARCHITECTURES_BY_TARGET[target]],
  };
}

export function parsePositiveInteger(raw, label) {
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

export function validateArtifactFilename(filename) {
  if (
    typeof filename !== "string" ||
    filename !== basename(filename) ||
    extname(filename) !== ".dmg" ||
    !/^[A-Za-z0-9][A-Za-z0-9._ ()+-]*\.dmg$/.test(filename)
  ) {
    throw new Error("Artifact must have a safe .dmg filename without a path");
  }
  return filename;
}
