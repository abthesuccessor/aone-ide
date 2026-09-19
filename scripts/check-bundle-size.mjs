import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");

// The v0.1 workbench keeps Monaco language workers local for offline editing;
// D3 and xterm are split behind graph/terminal entry points. The first-use AI
// setup is also split so its accessible Base UI combobox does not increase the
// eager workbench. The total caps include the locally owned Shadcn Base UI
// button/toggle primitives requested for workbench controls. The bounded graph
// artifact generators and knowledge navigator add only lazy-loaded code, so the
// eager startup budget remains unchanged. The consent-gated Project Agent and the activity
// navigator are lazy boundaries, so readiness/chat code does not raise the
// eager startup cap.
// The total-JS cap is calibrated to the canonical npm-ci graph, including
// DOMPurify 3.4.13 and Rolldown 1.2.4. Two clean-tree builds measured 3,723,644 B;
// 3,730,000 B leaves 6,356 B of headroom. The previous mixed pnpm installation
// used other resolved versions and is not the release baseline. Raw and eager
// caps remain unchanged; see docs/PERFORMANCE.md for the recorded comparison.
const budgets = {
  totalDistRawBytes: 15_720_000,
  eagerEntryJsGzipBytes: 120_000,
  totalJsGzipBytes: 3_730_000,
};

async function walkFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await walkFiles(path));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

function formatBytes(bytes) {
  const kibibytes = bytes / 1_024;
  if (kibibytes < 1_024) return `${kibibytes.toFixed(1)} KiB`;
  return `${(kibibytes / 1_024).toFixed(2)} MiB`;
}

const files = await walkFiles(dist);
const contents = new Map();
let totalDistRawBytes = 0;
let totalJsGzipBytes = 0;

for (const file of files) {
  const body = await readFile(file);
  contents.set(file, body);
  totalDistRawBytes += body.byteLength;
  if (file.endsWith(".js")) totalJsGzipBytes += gzipSync(body).byteLength;
}

const htmlPath = join(dist, "index.html");
const html = (contents.get(htmlPath) ?? await readFile(htmlPath)).toString("utf8");
const entrySource = html.match(/<script\b[^>]*\bsrc=["']([^"']+\.js)["'][^>]*>/i)?.[1];
if (!entrySource) throw new Error("dist/index.html does not reference a JavaScript entry");

const entryRelativePath = entrySource.split(/[?#]/, 1)[0].replace(/^\/+/, "");
const entryPath = resolve(dist, entryRelativePath);
if (entryPath !== dist && !entryPath.startsWith(`${dist}${sep}`)) {
  throw new Error(`Entry path escapes dist: ${entrySource}`);
}
const entryBody = contents.get(entryPath) ?? await readFile(entryPath);
const eagerEntryJsGzipBytes = gzipSync(entryBody).byteLength;

const measurements = [
  ["Total dist raw", totalDistRawBytes, budgets.totalDistRawBytes],
  ["Eager entry JS gzip", eagerEntryJsGzipBytes, budgets.eagerEntryJsGzipBytes],
  ["Total JS gzip", totalJsGzipBytes, budgets.totalJsGzipBytes],
];

process.stdout.write(`Production bundle budgets (${relative(root, entryPath)})\n`);
let failed = false;
for (const [label, actual, limit] of measurements) {
  const passed = actual <= limit;
  failed ||= !passed;
  process.stdout.write(
    `${passed ? "PASS" : "FAIL"} ${label}: ${formatBytes(actual)} (${actual} B) / ${formatBytes(limit)} (${limit} B)\n`,
  );
}

if (failed) {
  process.stderr.write("Bundle size budget exceeded. Keep the eager workbench lightweight or update the cap with rationale.\n");
  process.exitCode = 1;
}
