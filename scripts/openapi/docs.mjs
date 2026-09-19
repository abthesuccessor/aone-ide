import { spawnSync } from "node:child_process";

import { repositoryRoot, specificationRoot } from "./contract.mjs";

const image = "swaggerapi/swagger-ui@sha256:e43eb34b978af58d8cb78e5da9c12d605cf43d113ad3a96b18f9b028d6479d68";
const port = process.env.AONE_SWAGGER_PORT ?? "8090";
if (!/^\d{2,5}$/.test(port) || Number(port) > 65_535) {
  throw new Error(`AONE_SWAGGER_PORT must be a valid TCP port; received ${port}`);
}

process.stdout.write(`Aone Swagger UI: http://127.0.0.1:${port}\n`);
process.stdout.write("The documented paths are Tauri IPC commands; network submission is disabled.\n");
const result = spawnSync("docker", [
  "run", "--rm",
  "-p", `127.0.0.1:${port}:8080`,
  "-e", "SWAGGER_JSON=/spec/aone-ipc.openapi.yaml",
  "-e", "SUPPORTED_SUBMIT_METHODS=[]",
  "-v", `${specificationRoot}:/spec:ro`,
  image,
], { cwd: repositoryRoot, stdio: "inherit" });

if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`Swagger UI exited with status ${result.status}`);
