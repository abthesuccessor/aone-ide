import { readFile } from "node:fs/promises";

const endpoint = process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT;
const headers = process.env.OTEL_EXPORTER_OTLP_TRACES_HEADERS ?? "";
if (!endpoint) throw new Error("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT is required");

const requestHeaders = new Headers({ "content-type": "application/json" });
for (const entry of headers.split(",").map((value) => value.trim()).filter(Boolean)) {
  const separator = entry.indexOf("=");
  if (separator <= 0) throw new Error("OTEL_EXPORTER_OTLP_TRACES_HEADERS must use name=value entries");
  requestHeaders.set(entry.slice(0, separator), entry.slice(separator + 1));
}

const body = await readFile(new URL("./otlp-trace.json", import.meta.url));
const response = await fetch(endpoint, { method: "POST", headers: requestHeaders, body });
if (!response.ok) throw new Error(`OTLP receiver returned ${response.status}: ${await response.text()}`);
console.log(`OTLP demo trace accepted: ${response.status} ${await response.text()}`);
