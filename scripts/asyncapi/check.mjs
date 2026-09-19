import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");

async function readSourceTree(directory, extensions) {
  const entries = await readdir(directory, { withFileTypes: true });
  const parts = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) parts.push(await readSourceTree(path, extensions));
    else if (entry.isFile() && extensions.some((extension) => entry.name.endsWith(extension))) {
      parts.push(await readFile(path, "utf8"));
    }
  }
  return parts.join("\n");
}

const specification = JSON.parse(
  await readFile(resolve(root, "asyncapi/aone-events.asyncapi.json"), "utf8"),
);

if (specification.asyncapi !== "3.1.0") {
  throw new Error(`AsyncAPI 3.1.0 is required; received ${specification.asyncapi}`);
}

const channels = Object.values(specification.channels ?? {});
const addresses = channels.map((channel) => channel.address).sort();
if (addresses.length === 0 || addresses.some((address) => typeof address !== "string")) {
  throw new Error("Every AsyncAPI channel must have a string address");
}
if (new Set(addresses).size !== addresses.length) {
  throw new Error("AsyncAPI channel addresses must be unique");
}

const operations = Object.values(specification.operations ?? {});
if (operations.length !== channels.length || operations.some((operation) => operation.action !== "send")) {
  throw new Error("Each backend event channel must have one send operation");
}

const rendererBridges = await readSourceTree(resolve(root, "src/lib"), [".ts", ".tsx"]);
const listenedAddresses = [...rendererBridges.matchAll(/["'](aone-[a-z-]+)["']/g)]
  .map((match) => match[1])
  .filter((address) => address.endsWith("-event") || address.endsWith("-progress") || address.endsWith("-changed"))
  .sort();

if (addresses.join(",") !== [...new Set(listenedAddresses)].join(",")) {
  throw new Error(
    `AsyncAPI/renderer event mismatch: documented ${addresses.join(",")}; listened ${listenedAddresses.join(",")}`,
  );
}

const backend = await readSourceTree(resolve(root, "src-tauri/src"), [".rs"]);
const missingEmitters = addresses.filter((address) => !backend.includes(`"${address}"`));
if (missingEmitters.length > 0) {
  throw new Error(`AsyncAPI channels have no Rust emitter: ${missingEmitters.join(",")}`);
}

process.stdout.write(`AsyncAPI contract verified: ${addresses.length} backend event channels\n`);
