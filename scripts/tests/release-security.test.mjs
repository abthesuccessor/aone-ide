import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { afterEach, describe, test } from "node:test";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { promisify } from "node:util";
import { buildDownloadSite } from "../build-download-site.mjs";
import {
  issuePublicProof,
  normalizeHttpsOrigin,
} from "../lib/release-common.mjs";
import { verifyPublicAppleArtifact, verifyRelease } from "../verify-release.mjs";

const temporaryDirectories = new Set();
const VERSION = "0.1.0";
const TARGET = "aarch64-apple-darwin";
const ARCHITECTURE = "Apple Silicon";
const MINIMUM_MACOS = "15.0";
const PROOF_KEY = "ab".repeat(32);
const execFileAsync = promisify(execFile);

afterEach(async () => {
  await Promise.all([...temporaryDirectories].map((path) => rm(path, { recursive: true, force: true })));
  temporaryDirectories.clear();
});

async function fixture() {
  const projectRoot = await mkdtemp(join(tmpdir(), "aone-release-test-"));
  temporaryDirectories.add(projectRoot);
  const releaseRoot = join(projectRoot, "release");
  const siteSource = join(projectRoot, "site");
  await mkdir(join(siteSource, "assets"), { recursive: true });
  await mkdir(join(siteSource, "downloads"), { recursive: true });
  await mkdir(releaseRoot, { recursive: true });
  await Promise.all([
    writeFile(join(siteSource, "index.html"), "<!doctype html><title>Aone</title>\n"),
    writeFile(join(siteSource, "download.js"), "export {};\n"),
    writeFile(join(siteSource, "styles.css"), "body {}\n"),
    writeFile(join(siteSource, "downloads", "manifest.json"), "{}\n"),
    writeFile(join(projectRoot, "package.json"), `${JSON.stringify({ version: VERSION })}\n`),
  ]);
  const artifactPath = join(releaseRoot, "Aone-IDE_0.1.0_aarch64.dmg");
  await writeFile(artifactPath, "synthetic DMG bytes for integrity tests\n");
  return { projectRoot, releaseRoot, siteSource, artifactPath };
}

function localBuildOptions(state) {
  return {
    ...state,
    version: VERSION,
    target: TARGET,
    architecture: ARCHITECTURE,
    minimumMacos: MINIMUM_MACOS,
    channel: "local-test",
  };
}

describe("public origin validation", () => {
  test("accepts and canonicalizes an HTTPS origin", () => {
    assert.equal(normalizeHttpsOrigin("https://downloads.example.com/"), "https://downloads.example.com");
    assert.equal(normalizeHttpsOrigin("https://downloads.example.com:8443"), "https://downloads.example.com:8443");
  });

  for (const origin of [
    "http://downloads.example.com",
    "https://user@downloads.example.com",
    "https://downloads.example.com/releases",
    "https://downloads.example.com/?candidate=true",
    "https://downloads.example.com/#release",
    "https:\\downloads.example.com",
    "https://good.example.com\\@evil.example.com",
    "downloads.example.com",
    "https://",
  ]) {
    test(`rejects malformed origin ${origin}`, () => {
      assert.throws(() => normalizeHttpsOrigin(origin));
    });
  }
});

test("release shell disables inherited xtrace before inspecting credentials", async () => {
  const sentinel = "AONE_XTRACE_SENTINEL_92";
  let output = "";
  await assert.rejects(
    execFileAsync(
      "/bin/bash",
      ["--noprofile", "--norc", join(import.meta.dirname, "..", "release-macos.sh"), "--invalid"],
      {
        env: { ...process.env, SHELLOPTS: "xtrace", APPLE_PASSWORD: sentinel },
        encoding: "utf8",
      },
    ),
    (error) => {
      output = `${error.stdout ?? ""}${error.stderr ?? ""}`;
      return true;
    },
  );
  assert.doesNotMatch(output, new RegExp(sentinel));
});

test("local staging and integrity verification keep all public flags false", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const result = await verifyRelease({ projectRoot: state.projectRoot });
  assert.equal(result.manifest.channel, "local-test");
  assert.equal(result.manifest.artifact.signed, false);
  assert.equal(result.manifest.artifact.notarized, false);
  assert.equal(result.manifest.artifact.publicReady, false);
});

test("builder rejects a symlinked input DMG", async () => {
  const state = await fixture();
  const realArtifact = join(state.releaseRoot, "real.dmg");
  const symlinkArtifact = join(state.releaseRoot, "linked.dmg");
  await writeFile(realArtifact, "real bytes\n");
  await symlink(realArtifact, symlinkArtifact);
  await assert.rejects(
    buildDownloadSite(localBuildOptions({ ...state, artifactPath: symlinkArtifact })),
    /Symbolic links are not allowed/,
  );
  assert.equal(await readFile(realArtifact, "utf8"), "real bytes\n");
});

test("builder requires the input DMG to be a direct child of release root", async () => {
  const state = await fixture();
  const nested = join(state.releaseRoot, "nested");
  await mkdir(nested);
  const nestedArtifact = join(nested, "nested.dmg");
  await writeFile(nestedArtifact, "nested bytes\n");
  await assert.rejects(
    buildDownloadSite(localBuildOptions({ ...state, artifactPath: nestedArtifact })),
    /direct child/,
  );
});

test("builder rejects nested site symlinks without replacing the existing staged site", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const manifestPath = join(state.releaseRoot, "site", "downloads", "manifest.json");
  const before = await readFile(manifestPath, "utf8");
  const secretPath = join(state.projectRoot, "local-secret.txt");
  await writeFile(secretPath, "must not be published\n");
  await symlink(secretPath, join(state.siteSource, "assets", "leaked-secret.txt"));

  await assert.rejects(buildDownloadSite(localBuildOptions(state)), /Site source contains a symbolic link/);
  assert.equal(await readFile(manifestPath, "utf8"), before);
  await assert.rejects(readFile(join(state.releaseRoot, "site", "assets", "leaked-secret.txt"), "utf8"), /ENOENT/);
});

test("verifier rejects a symlinked staged DMG", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const stagedArtifact = join(state.releaseRoot, "site", "downloads", "Aone-IDE_0.1.0_aarch64.dmg");
  await rm(stagedArtifact);
  await symlink(state.artifactPath, stagedArtifact);
  await assert.rejects(verifyRelease({ projectRoot: state.projectRoot }), /Symbolic links are not allowed/);
});

test("verifier rejects any SHA256SUMS mutation or extra line", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const checksumPath = join(state.releaseRoot, "site", "downloads", "SHA256SUMS.txt");
  const checksum = await readFile(checksumPath, "utf8");
  await writeFile(checksumPath, `${checksum}extra\n`);
  await assert.rejects(
    verifyRelease({ projectRoot: state.projectRoot }),
    /must contain exactly the manifest digest and filename on one line/,
  );
});

test("verifier rejects a forged manifest digest even with a matching checksum line", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const downloads = join(state.releaseRoot, "site", "downloads");
  const manifestPath = join(downloads, "manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.artifact.sha256 = "0".repeat(64);
  await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
  await writeFile(join(downloads, "SHA256SUMS.txt"), `${manifest.artifact.sha256}  ${manifest.artifact.filename}\n`);
  await assert.rejects(verifyRelease({ projectRoot: state.projectRoot }), /does not match the release manifest/);
});

test("direct public staging fails without a trusted release proof", async () => {
  const state = await fixture();
  await assert.rejects(
    buildDownloadSite({
      ...localBuildOptions(state),
      channel: "public",
      origin: "https://downloads.example.com",
    }, { proofKey: PROOF_KEY }),
    /require --release-proof/,
  );
});

test("direct public staging rejects a forged proof", async () => {
  const state = await fixture();
  const proofDirectory = join(state.projectRoot, "forged-proof");
  await mkdir(proofDirectory, { mode: 0o700 });
  const proofPath = join(proofDirectory, "release-proof.json");
  await writeFile(proofPath, `${JSON.stringify({ claims: { purpose: "forged" }, signature: "0".repeat(64) })}\n`);
  await chmod(proofPath, 0o600);
  await assert.rejects(
    buildDownloadSite({
      ...localBuildOptions(state),
      channel: "public-candidate",
      origin: "https://downloads.example.com",
      releaseProof: proofPath,
    }, { proofKey: PROOF_KEY }),
    /signature is invalid/,
  );
});

test("malformed public origin fails before replacing an existing staged site", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const manifestPath = join(state.releaseRoot, "site", "downloads", "manifest.json");
  const before = await readFile(manifestPath, "utf8");
  await assert.rejects(
    buildDownloadSite({
      ...localBuildOptions(state),
      channel: "public-candidate",
      origin: "https://downloads.example.com/path",
      releaseProof: join(state.projectRoot, "unused-proof.json"),
    }, { proofKey: PROOF_KEY }),
    /must not contain a path/,
  );
  assert.equal(await readFile(manifestPath, "utf8"), before);
});

test("forged public-ready booleans cannot bypass independent Apple verification", async () => {
  const state = await fixture();
  await buildDownloadSite(localBuildOptions(state));
  const downloads = join(state.releaseRoot, "site", "downloads");
  const manifestPath = join(downloads, "manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.channel = "public";
  manifest.artifact.url = `https://downloads.example.com/downloads/${encodeURIComponent(manifest.artifact.filename)}`;
  manifest.artifact.signed = true;
  manifest.artifact.notarized = true;
  manifest.artifact.publicReady = true;
  await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`);
  await assert.rejects(
    verifyRelease({
      projectRoot: state.projectRoot,
      publicMode: true,
      expectedVersion: VERSION,
      expectedTarget: TARGET,
      expectedArchitecture: ARCHITECTURE,
      expectedMinimumMacos: MINIMUM_MACOS,
      expectedTeamId: "ABCDE12345",
      expectedSigningIdentity: "Developer ID Application: Example Developer (ABCDE12345)",
      expectedBundleId: "com.aone.ide",
      expectedExecutable: "aone-ide",
      origin: "https://downloads.example.com",
    }, {
      appleVerifier: async () => {
        throw new Error("unsigned artifact rejected");
      },
    }),
    /unsigned artifact rejected/,
  );
  const demoted = JSON.parse(await readFile(manifestPath, "utf8"));
  assert.equal(demoted.channel, "public-candidate");
  assert.deepEqual(
    [demoted.artifact.signed, demoted.artifact.notarized, demoted.artifact.publicReady],
    [false, false, false],
  );
});

test("public trust flags are outputs of independent staged-artifact verification", async () => {
  const state = await fixture();
  const proofDirectory = join(state.projectRoot, "proof");
  await mkdir(proofDirectory, { mode: 0o700 });
  const proofPath = join(proofDirectory, "release-proof.json");
  await issuePublicProof({
    artifactPath: state.artifactPath,
    releaseRoot: state.releaseRoot,
    version: VERSION,
    target: TARGET,
    architecture: ARCHITECTURE,
    minimumMacos: MINIMUM_MACOS,
    origin: "https://downloads.example.com",
    outputPath: proofPath,
    proofKey: PROOF_KEY,
  });
  await buildDownloadSite({
    ...localBuildOptions(state),
    channel: "public-candidate",
    origin: "https://downloads.example.com",
    releaseProof: proofPath,
  }, { proofKey: PROOF_KEY });

  const stagedManifestPath = join(state.releaseRoot, "site", "downloads", "manifest.json");
  const candidate = JSON.parse(await readFile(stagedManifestPath, "utf8"));
  assert.deepEqual(
    [candidate.artifact.signed, candidate.artifact.notarized, candidate.artifact.publicReady],
    [false, false, false],
  );

  let verifiedPath;
  const signingIdentity = "Developer ID Application: Example Developer (ABCDE12345)";
  const result = await verifyRelease({
    projectRoot: state.projectRoot,
    publicMode: true,
    expectedVersion: VERSION,
    expectedTarget: TARGET,
    expectedArchitecture: ARCHITECTURE,
    expectedMinimumMacos: MINIMUM_MACOS,
    expectedTeamId: "ABCDE12345",
    expectedSigningIdentity: signingIdentity,
    expectedBundleId: "com.aone.ide",
    expectedExecutable: "aone-ide",
    origin: "https://downloads.example.com",
  }, {
    appleVerifier: async (options) => {
      verifiedPath = options.artifactPath;
      return {
        bundleIdentifier: options.expectedBundleId,
        version: options.expectedVersion,
        build: options.expectedVersion,
        executable: options.expectedExecutable,
        target: options.expectedTarget,
        architecture: options.expectedArchitecture,
        architectures: ["arm64"],
        minimumMacos: options.expectedMinimumMacos,
        teamId: options.expectedTeamId,
        signingIdentity: options.expectedSigningIdentity,
      };
    },
  });
  assert.equal(verifiedPath, join(state.releaseRoot, "site", "downloads", "Aone-IDE_0.1.0_aarch64.dmg"));
  assert.equal(result.manifest.channel, "public");
  assert.deepEqual(
    [result.manifest.artifact.signed, result.manifest.artifact.notarized, result.manifest.artifact.publicReady],
    [true, true, true],
  );
  assert.equal(result.manifest.verification.teamId, "ABCDE12345");
});

test("Apple verifier binds the exact mounted app metadata and Mach-O facts", async () => {
  const state = await fixture();
  const calls = [];
  const plist = {
    CFBundleIdentifier: "com.aone.ide",
    CFBundleShortVersionString: VERSION,
    CFBundleVersion: VERSION,
    CFBundleExecutable: "aone-ide",
    LSMinimumSystemVersion: MINIMUM_MACOS,
  };
  const signingIdentity = "Developer ID Application: Example Developer (ABCDE12345)";
  const runTool = async (tool, args) => {
    calls.push({ tool, args: [...args] });
    if (tool === "hdiutil" && args[0] === "attach") {
      const mount = args.at(-1);
      const contents = join(mount, "Aone IDE.app", "Contents");
      await mkdir(join(contents, "MacOS"), { recursive: true });
      await writeFile(join(contents, "Info.plist"), "synthetic plist\n");
      await writeFile(join(contents, "MacOS", "aone-ide"), "synthetic executable\n", { mode: 0o755 });
    }
    if (tool === "hdiutil" && args[0] === "detach") {
      await rm(join(args[1], "Aone IDE.app"), { recursive: true, force: true });
    }
    if (tool === "plutil") return { stdout: `${plist[args[1]]}\n`, stderr: "" };
    if (tool === "codesign" && args[0] === "-d") {
      return {
        stdout: "",
        stderr: `Identifier=com.aone.ide\nTeamIdentifier=ABCDE12345\nAuthority=${signingIdentity}\n`,
      };
    }
    if (tool === "lipo") return { stdout: "arm64\n", stderr: "" };
    if (tool === "otool") return { stdout: "      cmd LC_BUILD_VERSION\n platform 1\n    minos 15.0\n", stderr: "" };
    return { stdout: "", stderr: "" };
  };

  const result = await verifyPublicAppleArtifact({
    artifactPath: state.artifactPath,
    expectedVersion: VERSION,
    expectedTarget: TARGET,
    expectedArchitecture: ARCHITECTURE,
    expectedMinimumMacos: MINIMUM_MACOS,
    expectedTeamId: "ABCDE12345",
    expectedSigningIdentity: signingIdentity,
    expectedBundleId: "com.aone.ide",
    expectedExecutable: "aone-ide",
  }, { runTool, allowNonDarwin: true });

  assert.equal(result.teamId, "ABCDE12345");
  assert.ok(calls.some(({ tool, args }) => tool === "hdiutil" && args[0] === "verify" && args[1] === state.artifactPath));
  assert.ok(calls.some(({ tool, args }) => tool === "xcrun" && args[0] === "stapler" && args.at(-1) === state.artifactPath));
  assert.ok(calls.some(({ tool, args }) => tool === "spctl" && args.includes("open") && args.at(-1) === state.artifactPath));
  assert.ok(calls.some(({ tool, args }) =>
    tool === "codesign" &&
    args.includes("-R") &&
    args.some((argument) => argument.includes("1.2.840.113635.100.6.1.13")),
  ));
  assert.ok(calls.some(({ tool, args }) => tool === "lipo" && args.at(-1).endsWith("/Contents/MacOS/aone-ide")));
  assert.ok(calls.some(({ tool, args }) => tool === "otool" && args.at(-1).endsWith("/Contents/MacOS/aone-ide")));
});
