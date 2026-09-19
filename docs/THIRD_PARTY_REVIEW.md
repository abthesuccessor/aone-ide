# Third-party licensing and provenance review

Review date: 2026-09-07. Status: **incomplete distribution review**.

AONE's original code uses the [root MIT license](../LICENSE). That grant does
not replace dependency licenses, copied-source notices, or asset permissions.
This review identifies a practical path to MIT publication; it does not certify
ownership, employer authorization, or compliance of a future release binary.

## What was checked

The [versioned inventory](licenses/dependency-inventory.json) records the two
lockfile SHA-256 hashes, every locked third-party package, declared license,
locally checked manifest, and root license/notice filenames and SHA-256 hashes.
Cache paths in that file are relative to Cargo's registry source directory;
they contain no user home path. A referenced local license file has been read
to calculate its hash; that does not mean every clause or embedded source file
has been reviewed.

| Evidence | Result | Limit |
| --- | --- | --- |
| `package-lock.json` | 255 dependency entries; all declare a license | Includes development tools and optional platform packages |
| Installed npm manifests after `npm ci` | 182 checked; versions match the lockfile | 73 entries are absent locally; their metadata comes from the lockfile |
| Selected npm owner resolutions | Monaco → DOMPurify 3.4.13; Vite → Rolldown 1.2.4 | Selected resolution checks, not an audit of every dependency edge or bundled output |
| `src-tauri/Cargo.lock` | 538 packages including AONE; 537 third-party packages | A lockfile includes targets and features beyond the built application |
| Cached Cargo manifests | All 537 third-party package manifests found and checked | Package-level declarations can omit embedded component details |
| Native dependency paths | `cargo tree --locked --offline` on the current host | Does not establish final binary contents or other operating systems |
| Copied UI components | Three files closely match the official shadcn registry | Historical import commit is unavailable |
| Theme/image provenance | Relevant local paths and candidate upstreams identified | Original source/rights records still need confirmation |

The installed npm snapshot was refreshed after the canonical `npm ci` install
from `package-lock.json`. It records actual manifest paths and hashes, plus
selected `createRequire` resolution checks from each owning package. Matching
package manifests at lockfile paths alone does not prove which package a nested
dependency resolves to; a mixed package-manager installation can have matching
files and still resolve different versions elsewhere.

The same selected checks resolve Vite's Lightning CSS to 1.33.0 and
`@tailwindcss/node`'s Lightning CSS to 1.32.0, matching their distinct lockfile
entries. These checks inspect resolution without executing dependency modules.
They do not cover every dependency edge, conditional export, bundler transform,
or final application artifact.

No GPL-only or AGPL-only declaration was found in these package-level metadata
fields. This is a bounded observation, not proof that every file is permissively
licensed. Two Cargo `r-efi` versions offer MIT or Apache alternatives alongside
LGPL; those alternatives are different from an LGPL-only dependency.

## Dependency licenses that need deliberate handling

The npm inventory includes MIT, Apache-2.0, ISC, BSD, MIT-0, CC0-1.0,
BlueOak-1.0.0, and MPL-2.0 declarations. The Cargo inventory also includes
Unicode-3.0, CDLA-Permissive-2.0, Zlib, Unlicense, LLVM exceptions, and compound
expressions. Preserve the exact declarations: `OR` offers alternatives, while
`AND` requires satisfying both parts. Do not replace them all with `MIT`.

### MPL dependencies

| Package | Locked version | Observed path or classification |
| --- | --- | --- |
| `option-ext` | 0.2.0 | Native normal dependency through `directories` / `dirs-sys`; also used by Tauri |
| `cssparser` | 0.36.0 | `dom_query` → `tauri-utils` in the current build/procedural macro graph |
| `cssparser-macros` | 0.6.1 | Associated parser procedural macro dependency |
| `dtoa-short` | 0.3.5 | Associated CSS parser dependency |
| `selectors` | 0.36.1 | `dom_query` → `tauri-utils` in the current build/procedural macro graph |
| `lightningcss` and platform packages | 1.32.0 and 1.33.0 | 24 npm lockfile entries, all marked development-only |
| `dompurify` | 3.4.13 | Runtime npm dependency; offers `MPL-2.0 OR Apache-2.0` |

MPL permits combining covered files with other differently licensed files.
When distributing an executable containing MPL code, recipients need notice of
where to obtain the covered source under MPL. Changes to covered source retain
MPL obligations. The dependency's presence therefore does not by itself force
AONE's original source to use MPL. Document the exact covered versions, their
source location and availability, modifications if any, and applicable notices.
Review what is actually shipped before treating build-only dependencies as
distributed components. [Mozilla MPL FAQ, questions 8–12](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)

For DOMPurify, choosing the Apache alternative is possible under its declared
expression; record that choice in the final notice manifest. Apache-2.0
redistribution requires its license text and retention of applicable notices,
including upstream NOTICE content where relevant, plus modification notices
when applicable. [Apache-2.0, section 4](https://www.apache.org/licenses/LICENSE-2.0)

### Embedded notices beyond a package's headline license

`monaco-editor` is declared MIT, but its installed `ThirdPartyNotices.txt`
contains additional component notices, including TypeScript and Unicode
material. Preserve/review that file for the actual bundled Monaco workers;
the `MIT` field alone is insufficient. AONE imports those workers from the
local package in `src/lib/monaco.ts`.

The current cache also contains multiple license texts for `ring`, including
`LICENSE-BoringSSL` and `LICENSE-other-bits`. Review bundled native source/data
such as SQLite, TLS implementations, and certificate roots using each
dependency's actual distribution. The inventory deliberately lists root files
without claiming to have exhaustively scanned nested vendored components.

The Tauri configuration currently has no notice-resource arrangement. A notice
file sitting in this repository does not establish that the same file reaches
a DMG recipient. Before binary publication, generate and package notices for
the actual release target and verify them inside the built application.

## Copied UI source: a known notice gap addressed

`components.json` selects shadcn's `base-nova` style. On the review date, these
local files were compared with the content of their official registry entries:

| Local source | Official registry | Text similarity |
| --- | --- | --- |
| `src/components/ui/button.tsx` | [button registry](https://ui.shadcn.com/r/styles/base-nova/button.json) | 99.78% |
| `src/components/ui/toggle.tsx` | [toggle registry](https://ui.shadcn.com/r/styles/base-nova/toggle.json) | 99.20% |
| `src/components/ui/toggle-group.tsx` | [toggle-group registry](https://ui.shadcn.com/r/styles/base-nova/toggle-group.json) | 98.29% |

These similarities establish substantial shared implementation, with local
adaptations. The registry's current content hashes and local file hashes are
recorded in the inventory; the original import revision remains unknown.

The verified upstream [shadcn MIT license](https://github.com/shadcn-ui/ui/blob/main/LICENSE.md),
including its `Copyright (c) 2023 shadcn` attribution, is preserved verbatim in
[licenses/shadcn-MIT.txt](licenses/shadcn-MIT.txt). Retain it with these files in
source distributions and include it when their implementation is bundled.
This attribution does not assert that shadcn wrote AONE-specific behavior.

The underlying `@base-ui/react` package has its own MIT attribution, verified
from the installed package and the [upstream Base UI license](https://github.com/mui/base-ui/blob/master/LICENSE).
It belongs in the dependency notices as well. The local `Resizable.tsx`,
`src/lib/utils.ts`, and `src/styles/shadcn.css` still need any historical
source/import records checked; similarity of a small utility or layout pattern
alone does not establish a specific author.

Functional toolbar and control icons come from `@phosphor-icons/react` 2.1.10.
Its verified installed MIT license, including `Copyright (c) 2020 Phosphor
Icons`, is preserved verbatim in
[licenses/phosphor-icons-MIT.txt](licenses/phosphor-icons-MIT.txt). These
dependency-provided UI glyphs have a known attribution and are separate from
the custom application/logo image files below. Include their notice when the
icons are redistributed.

## Provenance that remains unresolved

The author has not supplied the original source links or creation records for
the materials below. Their files remain present in the repository; absence of
records does not mean there are no image assets or named themes.

| Material | Evidence and remaining action |
| --- | --- |
| AONE logos and icons | `assets/aone-icon-source.png`, `src-tauri/icons/**`, and `site/assets/aone-icon.png` lack a checked origin/rights record. Confirm who created the source image, any generation or third-party inputs, and permission to redistribute it; record how derived icons were produced. |
| Website screenshot | `site/assets/aone-workbench.png` needs a checked capture record and confirmation that visible workspace names, code, paths, data, and branding are publishable. |
| VS Code theme presets | `src/features/editor/themes.ts` contains named Dark Modern, Light Modern, and High Contrast presets. Microsoft VS Code's source [license is MIT](https://github.com/microsoft/vscode/blob/main/LICENSE.txt), but the exact imported palette/source revision and applicable attribution must be recorded. |
| Cursor Dark preset | The same file includes a named Cursor Dark palette. No authoritative source/license for the palette was established in this review. Record an independent implementation origin or obtain the actual upstream permission; do not infer that a product being downloadable grants source reuse rights. |
| Tokyo Night preset | Values in `themes.ts` use Tokyo Night colors. [Enkia's VS Code theme is MIT](https://github.com/enkia/tokyo-night-vscode-theme/blob/master/LICENSE.txt), while [folke's Neovim implementation is Apache-2.0](https://github.com/folke/tokyonight.nvim/blob/main/LICENSE). Identify the actual source and revision before choosing notices. |
| Catppuccin Mocha preset | The local preset uses named Mocha colors. The [Catppuccin project has an MIT license](https://github.com/catppuccin/catppuccin/blob/main/LICENSE); record the actual palette source/version and preserve its applicable attribution. |
| Other borrowed source, examples, or documentation | No historical Git provenance is available in this initial repository. The author's review is still needed for material copied from earlier projects, online examples, workplace repositories, or external documents. |

A matching color or brand name alone does not prove copyright ownership,
copying, or trademark permission. The actions above request provenance rather
than make unsupported infringement findings. The README's statement that
Graphify was inspiration and that its source/runtime is not embedded is
consistent with the dependency manifests and inspected source, but it is not
an independent historical authorship audit.

## Generated contracts

`scripts/openapi/contract.mjs` pins OpenAPI Generator 7.24.0 by image digest.
`scripts/openapi/generate.mjs` generates TypeScript models/runtime from the
local OpenAPI specification. The [upstream generated-code policy](https://github.com/OpenAPITools/openapi-generator#34---license-information-on-generated-code)
allows generated output to be licensed by its user separately from the
generator. Therefore the generator's Apache license does not automatically
require this generated output to be Apache licensed. This does not resolve
rights in externally supplied specifications or extra templates.

Keep generated-file provenance and the drift check. Do not hand-edit generated
files to insert a new license header; if a header is desired, configure it at
the owning specification/generation layer and regenerate.

## Completion criteria

Before source publication, retain the AONE MIT grant, resolve the theme/image
and imported-source records above, and retain all notices for third-party
material actually committed. Publishing lockfiles alone is different from
republishing the dependency source archives or `node_modules`.

Before distributing an application, additionally identify the exact built
target/features and shipped components, gather their full license and embedded
notices, satisfy applicable source-availability obligations, and verify those
materials in the packaged artifact. Record any dual-license selections and
modifications. The metadata inventory is an input to that work, not its
replacement.

To repeat the native path checks without fetching or changing dependencies:

```sh
cargo tree --manifest-path src-tauri/Cargo.toml --locked --offline -i option-ext
cargo tree --manifest-path src-tauri/Cargo.toml --locked --offline -i cssparser
```

Refresh the inventory after dependency changes. Read npm license declarations
from every non-root `packages` entry in `package-lock.json`; confirm locally
installed manifests separately. Join every third-party Cargo lock entry to its
exact cached crate manifest, preserve its raw license expression, and collect
the package's root license/notice filenames and hashes. Do not silently treat
an unavailable cache entry or empty notice list as cleared. The inventory was
created by a bounded one-off read of these local files; the inventory step did
not install dependencies, upload source, push Git, or configure GitHub. Separate
development validation is recorded in [publication readiness](OPEN_SOURCE_READINESS.md).
