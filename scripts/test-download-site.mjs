import { access, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const site = join(root, "site");
const [html, css, script, manifestText] = await Promise.all([
  readFile(join(site, "index.html"), "utf8"),
  readFile(join(site, "styles.css"), "utf8"),
  readFile(join(site, "download.js"), "utf8"),
  readFile(join(site, "downloads", "manifest.json"), "utf8"),
]);
const manifest = JSON.parse(manifestText);

const assertions = [
  [html.includes('<main id="main">'), "main landmark is missing"],
  [html.includes("<h1"), "primary heading is missing"],
  [(html.match(/class="primary-download" data-download-link/g) ?? []).length === 2, "download controls changed unexpectedly"],
  [html.includes('width="1600"') && html.includes('height="1000"'), "screenshot dimensions are missing"],
  [css.includes("prefers-reduced-motion: reduce"), "reduced-motion fallback is missing"],
  [css.includes("@media (max-width: 800px)"), "mobile layout is missing"],
  [script.includes('fetch(manifestPath, { cache: "no-store" })'), "manifest must load without stale caching"],
  [script.includes("resolved.origin !== window.location.origin"), "artifact links must stay on the trusted site origin"],
  [script.includes("hasAppleReleaseEvidence"), "public copy must require complete Apple release evidence"],
  [!script.includes("Verified public build"), "the browser must not claim independent artifact verification"],
  [html.includes('href="./assets/aone-icon.png"'), "site favicon is missing"],
  [manifest.schemaVersion === 1, "manifest schema version is invalid"],
  [manifest.artifact.publicReady === false, "checked-in template must not claim public readiness"],
  [manifest.artifact.signed === false && manifest.artifact.notarized === false, "template signing state is dishonest"],
  [/^[a-f0-9]{64}$/.test(manifest.artifact.sha256), "template checksum is invalid"],
  [!/[—–]/u.test(html), "visible site copy contains a forbidden dash character"],
];

for (const [condition, message] of assertions) {
  if (!condition) throw new Error(message);
}

await access(join(site, "assets", "aone-workbench.png"));
await access(join(site, "assets", "aone-icon.png"));
process.stdout.write("download site checks passed\n");
