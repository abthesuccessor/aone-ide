# macOS distribution

Aone has two release modes. They deliberately produce different claims.

The artifact recorded below is the current verified local-test ARM64 DMG. It
includes the deterministic execution Flow/Map and strict `AONE_TRACE_V1` source.
It is ad-hoc signed, not Developer ID signed or notarized, and is not published
from a main domain. Public availability remains blocked on the Apple authority
and controlled HTTPS host described below.

## Local test build

```bash
npm run release:macos:test
```

This mode installs from the locked dependency graph, runs the full application checks (including the release-security regression suite), builds an ad-hoc signed DMG, verifies its disk image, enforces the 300 MiB limit, computes SHA-256, and stages a dependency-free site in `release/site`. The page labels the artifact as a local test build. Gatekeeper approval may still be required on another Mac.

After any native or renderer change, previous DMG measurements must be treated as historical. The release log is current only when the canonical, target-specific, and staged DMG bytes match the newly generated manifest and `npm run release:verify` accepts that same artifact.

### Current debugger local-test artifact

- Canonical artifact: `release/Aone-IDE_0.1.0_aarch64.dmg`.
- The canonical artifact, Tauri target-specific bundle DMG, and `release/site/downloads/Aone-IDE_0.1.0_aarch64.dmg` are byte-identical regular `0644` files.
- Size: 8,813,878 bytes (about 8.41 MiB), below the 300 MiB release limit.
- SHA-256: `c15166f8369e39c792d66fe620846ca1c42d5f8ccbebfda5d08e75757ada019f`.
- `hdiutil` verification passed with overall CRC `9F4651A6` and HFS CRC `D75B11FF`.
- Executable architecture: thin `arm64`; minimum macOS 15.0; SDK 26.5.
- Bundle: `com.aone.ide`; application version and build version: `0.1.0`.
- `codesign --verify --deep --strict` passed structurally. The signature is ad-hoc with hardened runtime flags and no Team Identifier.
- No notarization ticket is stapled. Gatekeeper rejects both the app and DMG, which is expected for this local-test signature. Staged manifest trust and public-ready flags remain false.
- The canonical DMG was mounted read-only; bundle identity, architecture, minimum OS, signature, and expected local-test trust failures were independently rechecked from that mount.
- The packaged app was copied to `/Applications/Aone IDE.app`, the mount was detached, and the installed application launched successfully.
- The staged manifest/checksum describe these exact bytes and
  `npm run release:verify` passed.

The previous pre-debugger package was 8,519,003 bytes with SHA-256
`3852707e6162b7a4dd9b4fa8d6d6c2ab4ec799260c81e25c27c9d0489e86ef5b`.
It remains historical and does not substitute for the current package above.

The default target is Apple Silicon. Override it only after installing the Rust target:

```bash
rustup target add x86_64-apple-darwin
AONE_MACOS_TARGET=x86_64-apple-darwin npm run release:macos:test
```

For a universal build, install both architecture targets and use:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
AONE_MACOS_TARGET=universal-apple-darwin npm run release:macos:test
```

## Public build

Public distribution outside the App Store requires a paid Apple Developer membership, a `Developer ID Application` certificate in the signing Keychain, and a `notarytool` profile stored in the Keychain. Create that profile once with either method below.

Apple ID credentials, entered through `notarytool`'s secure password prompt:

```bash
xcrun notarytool store-credentials "aone-notary" \
  --apple-id "developer@example.com" \
  --team-id "TEAMID"
```

The command deliberately omits a password argument so `notarytool` prompts for the app-specific password instead of placing it in shell history or an environment variable.

App Store Connect API key:

```bash
xcrun notarytool store-credentials "aone-notary" \
  --key "/absolute/path/to/AuthKey_KEYID.p8" \
  --key-id "KEYID" \
  --issuer "ISSUER_UUID"
```

After the profile is stored, the public build accepts only non-secret release configuration:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Example Company (TEAMID)"
export AONE_EXPECTED_TEAM_ID="TEAMID"
export AONE_NOTARY_PROFILE="aone-notary"
export AONE_DOWNLOAD_ORIGIN="https://aone.example.com"
npm run release:macos
```

Raw Apple credential environment variables are intentionally rejected. Do not export `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `APPLE_API_ISSUER`, `APPLE_API_KEY`, or `APPLE_API_KEY_PATH` for this release command. Secret notarization credentials stay in the Keychain; an API key file is needed only while creating or replacing its profile.

The public gate refuses to continue unless:

- the signing identity exists in the current keychain
- the signing identity and `AONE_EXPECTED_TEAM_ID` identify the same Developer ID team
- the named `AONE_NOTARY_PROFILE` authenticates through the Keychain
- the download origin uses HTTPS
- all application checks pass
- the DMG verifies and remains below 300 MiB
- stapler validates the notarization ticket
- Gatekeeper accepts the primary signature
- the bundle identifier, application version/build, executable name, exact architecture set, and minimum macOS version match the release configuration
- the final verifier checks the exact staged DMG bytes, checksum, signature team, bundle identity, architecture, and minimum macOS version before marking the manifest signed, notarized, and public-ready

Credentials remain in the macOS Keychain and are never written to the repository or release site.

Both npm entry points invoke a non-login, non-interactive Bash process with startup-file variables and inherited shell tracing removed. The public flow binds its candidate manifest proof to the exact artifact and Apple release metadata, keeps the public flags false until independent signature/notary/Gatekeeper checks succeed, and rejects staged-artifact substitution or symlink escapes.

## Main-domain hosting

Upload the contents of `release/site`, not the directory itself, to the HTTPS document root for the configured domain. The output is static HTML, CSS, JavaScript, one screenshot, the DMG, and its release manifest. It has no analytics, cookies, server runtime, package registry, or graph service.

The checked-in `site/downloads/` content, including `manifest.json`, is a non-downloadable local-test template. Only the release script creates a downloadable staged manifest and DMG under `release/site/downloads/`. Do not hand-edit `publicReady`, `signed`, or `notarized`.

After upload, verify from a different Mac:

```bash
curl -fsS https://aone.example.com/downloads/manifest.json
curl -fLO https://aone.example.com/downloads/Aone-IDE_0.1.0_universal.dmg
shasum -a 256 Aone-IDE_0.1.0_universal.dmg
xcrun stapler validate Aone-IDE_0.1.0_universal.dmg
spctl --assess --type open --context context:primary-signature -vv Aone-IDE_0.1.0_universal.dmg
```

The domain and hosting provider are intentionally external configuration. This repository has no GitHub Actions and does not assume GitHub Releases.

No upload has been performed for v0.1 because the fresh local-test artifact cannot satisfy the public gate without Developer ID signing/notarization, and no controlled production domain has been configured.
