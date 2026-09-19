import { resolve } from "node:path";
import process from "node:process";

import { issuePublicProof } from "./public-proof.mjs";
import {
  PUBLIC_PROOF_ENV,
  isMainModule,
  normalizeHttpsOrigin,
  parseNamedArguments,
} from "./release-metadata.mjs";

export {
  PUBLIC_PROOF_ENV,
  PUBLIC_PROOF_LIFETIME_MS,
  PUBLIC_PROOF_PURPOSE,
  architectureFacts,
  isMainModule,
  normalizeHttpsOrigin,
  parseNamedArguments,
  parsePositiveInteger,
  validateArtifactFilename,
  validateMinimumMacos,
  validateTarget,
  validateVersion,
} from "./release-metadata.mjs";
export {
  copyConfinedFileExclusive,
  copyRegularFileExclusive,
  inspectConfinedRegularFile,
  openConfinedRegularFile,
  readConfinedRegularFile,
  sha256ConfinedFile,
} from "./confined-files.mjs";
export {
  consumePublicProof,
  issuePublicProof,
} from "./public-proof.mjs";

async function cli() {
  const [command, ...argv] = process.argv.slice(2);
  if (command === "validate-origin") {
    const args = parseNamedArguments(argv, { values: ["--origin"] });
    const rawOrigin = args.optional("--origin", args.positionals[0]);
    if (args.positionals.length > (args.has("--origin") ? 0 : 1)) throw new Error("Unexpected positional arguments");
    process.stdout.write(`${normalizeHttpsOrigin(rawOrigin)}\n`);
    return;
  }
  if (command === "issue-proof") {
    const args = parseNamedArguments(argv, {
      values: [
        "--artifact",
        "--version",
        "--target",
        "--architecture",
        "--minimum-macos",
        "--origin",
        "--output",
      ],
    });
    if (args.positionals.length) throw new Error("Unexpected positional arguments");
    const projectRoot = resolve(import.meta.dirname, "../..");
    const output = await issuePublicProof({
      artifactPath: args.required("--artifact"),
      releaseRoot: resolve(projectRoot, "release"),
      version: args.required("--version"),
      target: args.required("--target"),
      architecture: args.required("--architecture"),
      minimumMacos: args.required("--minimum-macos"),
      origin: args.required("--origin"),
      outputPath: args.required("--output"),
      proofKey: process.env[PUBLIC_PROOF_ENV],
    });
    process.stdout.write(`${output}\n`);
    return;
  }
  throw new Error("usage: release-common.mjs validate-origin <origin> | issue-proof --artifact <dmg> --version <version> --target <target> --architecture <label> --minimum-macos <version> --origin <origin> --output <path>");
}

if (isMainModule(import.meta.url)) {
  cli().catch((error) => {
    process.stderr.write(`release-common: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
