import { basename } from "node:path";
import process from "node:process";

import { DEFAULT_MAX_BYTES, verifyRelease } from "./lib/release-manifest.mjs";
import {
  isMainModule,
  parseNamedArguments,
  parsePositiveInteger,
} from "./lib/release-metadata.mjs";

export { verifyPublicAppleArtifact } from "./lib/apple-verification.mjs";
export { verifyRelease } from "./lib/release-manifest.mjs";

async function cli() {
  const args = parseNamedArguments(process.argv.slice(2), {
    flags: ["--public"],
    values: [
      "--expected-version",
      "--expected-target",
      "--expected-architecture",
      "--expected-minimum-macos",
      "--expected-team-id",
      "--expected-signing-identity",
      "--expected-bundle-id",
      "--expected-executable",
      "--origin",
      "--max-bytes",
    ],
  });
  if (args.positionals.length) throw new Error("Unexpected positional arguments");
  const publicMode = args.has("--public");
  const result = await verifyRelease({
    publicMode,
    expectedVersion: publicMode ? args.required("--expected-version") : undefined,
    expectedTarget: publicMode ? args.required("--expected-target") : undefined,
    expectedArchitecture: publicMode ? args.required("--expected-architecture") : undefined,
    expectedMinimumMacos: publicMode ? args.required("--expected-minimum-macos") : undefined,
    expectedTeamId: publicMode ? args.required("--expected-team-id") : undefined,
    expectedSigningIdentity: publicMode ? args.required("--expected-signing-identity") : undefined,
    expectedBundleId: publicMode ? args.required("--expected-bundle-id") : undefined,
    expectedExecutable: publicMode ? args.required("--expected-executable") : undefined,
    origin: publicMode ? args.required("--origin") : undefined,
    maxBytes: args.has("--max-bytes") ? parsePositiveInteger(args.required("--max-bytes"), "--max-bytes") : DEFAULT_MAX_BYTES,
  });
  process.stdout.write(`verified ${basename(result.artifactPath)}\nsha256 ${result.sha256}\nbytes ${result.bytes}\n`);
}

if (isMainModule(import.meta.url)) {
  cli().catch((error) => {
    process.stderr.write(`verify-release: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
