import { execFile } from "node:child_process";
import { lstat, mkdtemp, realpath, rmdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { inspectConfinedRegularFile } from "./confined-files.mjs";
import {
  architectureFacts,
  validateMinimumMacos,
  validateTarget,
  validateVersion,
} from "./release-metadata.mjs";

const execFileAsync = promisify(execFile);

const TRUSTED_TOOLS = Object.freeze({
  hdiutil: "/usr/bin/hdiutil",
  xcrun: "/usr/bin/xcrun",
  spctl: "/usr/sbin/spctl",
  codesign: "/usr/bin/codesign",
  lipo: "/usr/bin/lipo",
  otool: "/usr/bin/otool",
  plutil: "/usr/bin/plutil",
});

export function sameStringSet(actual, expected) {
  if (!Array.isArray(actual) || actual.some((item) => typeof item !== "string")) return false;
  const left = [...new Set(actual)].sort();
  const right = [...new Set(expected)].sort();
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

function normalizedNumericVersion(value) {
  const parts = String(value).split(".");
  if (!parts.every((part) => /^[0-9]+$/.test(part))) return null;
  while (parts.length < 3) parts.push("0");
  while (parts.length > 1 && parts.at(-1) === "0") parts.pop();
  return parts.map(Number).join(".");
}

export function validateExpectedTeamId(value) {
  if (typeof value !== "string" || !/^[A-Z0-9]{10}$/.test(value)) {
    throw new Error("--expected-team-id must be an explicit 10-character Apple Team ID");
  }
  return value;
}

function assertInside(parent, child, label) {
  const pathFromParent = relative(parent, child);
  if (pathFromParent.startsWith("..") || resolve(parent, pathFromParent) !== child) {
    throw new Error(`${label} resolves outside the mounted image`);
  }
}

export async function assertRealDirectory(path, expectedParent = undefined) {
  const absolutePath = resolve(path);
  const info = await lstat(absolutePath);
  if (info.isSymbolicLink() || !info.isDirectory()) throw new Error(`Expected a real directory: ${absolutePath}`);
  const realPath = await realpath(absolutePath);
  if (expectedParent) {
    const parentReal = await realpath(expectedParent);
    assertInside(parentReal, realPath, absolutePath);
  }
  return realPath;
}

async function assertTrustedToolPaths() {
  for (const path of Object.values(TRUSTED_TOOLS)) {
    const info = await lstat(path);
    if (info.isSymbolicLink() || !info.isFile()) throw new Error(`Trusted Apple tool is not a regular file: ${path}`);
    if (await realpath(path) !== path) throw new Error(`Trusted Apple tool resolved unexpectedly: ${path}`);
  }
}

async function runProductionTool(name, args) {
  const executable = TRUSTED_TOOLS[name];
  if (!executable) throw new Error(`Unknown trusted tool: ${name}`);
  const environment = { ...process.env, PATH: "/usr/bin:/bin:/usr/sbin:/sbin", LANG: "C", LC_ALL: "C" };
  for (const key of Object.keys(environment)) {
    if (
      key.startsWith("DYLD_") ||
      key.startsWith("APPLE_") ||
      key.startsWith("NPM_CONFIG_") ||
      key.startsWith("npm_config_") ||
      new Set([
        "AONE_RELEASE_PROOF_KEY",
        "BASH_ENV",
        "CDPATH",
        "CODESIGN_ALLOCATE",
        "DEVELOPER_DIR",
        "ENV",
        "GLOBIGNORE",
        "NODE_OPTIONS",
        "NODE_PATH",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "SDKROOT",
        "SHELLOPTS",
        "TOOLCHAINS",
      ]).has(key)
    ) {
      delete environment[key];
    }
  }
  return execFileAsync(executable, args, {
    encoding: "utf8",
    env: environment,
    maxBuffer: 4 * 1024 * 1024,
    windowsHide: true,
  });
}

async function plistValue(runTool, plistPath, key) {
  const result = await runTool("plutil", ["-extract", key, "raw", "-o", "-", plistPath]);
  return result.stdout.trim();
}

function codeSignLines(result) {
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

export async function verifyPublicAppleArtifact({
  artifactPath,
  expectedVersion,
  expectedTarget,
  expectedArchitecture,
  expectedMinimumMacos,
  expectedTeamId,
  expectedSigningIdentity,
  expectedBundleId,
  expectedExecutable,
}, dependencies = {}) {
  if (process.platform !== "darwin" && !dependencies.allowNonDarwin) {
    throw new Error("Public Apple verification must run on macOS");
  }
  if (!dependencies.runTool) await assertTrustedToolPaths();
  const runTool = dependencies.runTool ?? runProductionTool;

  validateVersion(expectedVersion, "expected version");
  validateTarget(expectedTarget);
  const expectedFacts = architectureFacts(expectedTarget, expectedArchitecture);
  validateMinimumMacos(expectedMinimumMacos);
  validateExpectedTeamId(expectedTeamId);
  if (typeof expectedSigningIdentity !== "string" || !expectedSigningIdentity.startsWith("Developer ID Application:")) {
    throw new Error("Expected signing identity must be a Developer ID Application identity");
  }
  if (/[\r\n]/.test(expectedSigningIdentity) || !expectedSigningIdentity.endsWith(` (${expectedTeamId})`)) {
    throw new Error("Expected signing identity must end with the explicit Apple Team ID");
  }
  if (typeof expectedBundleId !== "string" || expectedBundleId !== "com.aone.ide") {
    throw new Error("Expected bundle identifier must be com.aone.ide");
  }
  if (expectedExecutable !== "aone-ide") throw new Error("Expected executable must be aone-ide");

  await runTool("hdiutil", ["verify", artifactPath]);
  await runTool("xcrun", ["stapler", "validate", artifactPath]);
  await runTool("spctl", ["--assess", "--type", "open", "--context", "context:primary-signature", "-vv", artifactPath]);

  const mountDirectory = await mkdtemp(join(tmpdir(), "aone-public-verify-"));
  let attached = false;
  let primaryError;
  try {
    await runTool("hdiutil", ["attach", artifactPath, "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mountDirectory]);
    attached = true;
    const mountReal = await assertRealDirectory(mountDirectory);
    const appPath = join(mountDirectory, "Aone IDE.app");
    const appReal = await assertRealDirectory(appPath, mountDirectory);
    assertInside(mountReal, appReal, "Aone IDE.app");

    const contentsPath = join(appPath, "Contents");
    await assertRealDirectory(contentsPath, appPath);
    const plistPath = join(contentsPath, "Info.plist");
    await inspectConfinedRegularFile(plistPath, contentsPath);
    const executableDirectory = join(contentsPath, "MacOS");
    await assertRealDirectory(executableDirectory, contentsPath);
    const executablePath = join(executableDirectory, expectedExecutable);
    const executable = await inspectConfinedRegularFile(executablePath, executableDirectory);
    if ((executable.mode & 0o111) === 0) throw new Error("Application executable is not executable");

    const [bundleId, shortVersion, buildVersion, executableName, plistMinimumMacos] = await Promise.all([
      plistValue(runTool, plistPath, "CFBundleIdentifier"),
      plistValue(runTool, plistPath, "CFBundleShortVersionString"),
      plistValue(runTool, plistPath, "CFBundleVersion"),
      plistValue(runTool, plistPath, "CFBundleExecutable"),
      plistValue(runTool, plistPath, "LSMinimumSystemVersion"),
    ]);
    if (bundleId !== expectedBundleId) throw new Error(`Bundle identifier mismatch: ${bundleId}`);
    if (shortVersion !== expectedVersion) throw new Error(`Bundle version mismatch: ${shortVersion}`);
    if (buildVersion !== expectedVersion) throw new Error(`Bundle build mismatch: ${buildVersion}`);
    if (executableName !== expectedExecutable) throw new Error(`Bundle executable mismatch: ${executableName}`);
    if (plistMinimumMacos !== expectedMinimumMacos) {
      throw new Error(`Bundle minimum macOS mismatch: ${plistMinimumMacos}`);
    }

    const signatureRequirement = `=anchor apple generic and identifier "${expectedBundleId}" and certificate leaf[subject.OU] = "${expectedTeamId}" and certificate leaf[field.1.2.840.113635.100.6.1.13] exists`;
    await runTool("codesign", [
      "--verify",
      "--deep",
      "--strict",
      "--verbose=2",
      "-R",
      signatureRequirement,
      appPath,
    ]);
    const signature = await runTool("codesign", ["-d", "--verbose=4", appPath]);
    const signatureLines = codeSignLines(signature);
    if (!signatureLines.includes(`Identifier=${expectedBundleId}`)) throw new Error("Code signature identifier does not match");
    if (!signatureLines.includes(`TeamIdentifier=${expectedTeamId}`)) throw new Error("Code signature Team ID does not match");
    if (!signatureLines.includes(`Authority=${expectedSigningIdentity}`)) throw new Error("Code signature identity does not match");
    if (!signatureLines.some((line) => line.startsWith("Authority=Developer ID Application:"))) {
      throw new Error("Application is not signed with a Developer ID Application certificate");
    }

    await runTool("spctl", ["--assess", "--type", "execute", "-vv", appPath]);
    const lipo = await runTool("lipo", ["-archs", executablePath]);
    const actualArchitectures = lipo.stdout.trim().split(/\s+/).filter(Boolean);
    if (!sameStringSet(actualArchitectures, expectedFacts.macho)) {
      throw new Error(`Mach-O architecture mismatch: ${actualArchitectures.join(", ")}`);
    }

    const loadCommands = await runTool("otool", ["-l", executablePath]);
    const minimumVersions = [...loadCommands.stdout.matchAll(/^\s+minos\s+(\S+)$/gm)].map((match) => match[1]);
    const platforms = [...loadCommands.stdout.matchAll(/^\s+platform\s+(\S+)$/gm)].map((match) => match[1]);
    if (minimumVersions.length !== expectedFacts.macho.length) {
      throw new Error("Could not bind a minimum macOS version to every Mach-O architecture");
    }
    if (platforms.length !== expectedFacts.macho.length || platforms.some((platform) => platform !== "1")) {
      throw new Error("Every Mach-O architecture must target the macOS platform");
    }
    const configuredMinimum = normalizedNumericVersion(expectedMinimumMacos);
    if (minimumVersions.some((value) => normalizedNumericVersion(value) !== configuredMinimum)) {
      throw new Error(`Minimum macOS mismatch: ${minimumVersions.join(", ")}`);
    }

    return {
      bundleIdentifier: bundleId,
      version: shortVersion,
      build: buildVersion,
      executable: executableName,
      target: expectedTarget,
      architecture: expectedFacts.display,
      architectures: expectedFacts.macho,
      minimumMacos: expectedMinimumMacos,
      teamId: expectedTeamId,
      signingIdentity: expectedSigningIdentity,
    };
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    if (attached) {
      try {
        await runTool("hdiutil", ["detach", mountDirectory]);
        attached = false;
      } catch (detachError) {
        if (!primaryError) throw detachError;
      }
    }
    if (!attached) await rmdir(mountDirectory).catch(() => {});
  }
}
