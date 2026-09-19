import http from "node:http";
import { createRouter } from "./router.js";
import { getShipment } from "./shipment-service.js";
import { debugSafePoint, withDebugWorkflow } from "./aone-debug.js";

const router = createRouter();

router.get("/api/shipments/:shipmentId", async ({ params }, response) => {
  await withDebugWorkflow(async (workflowId) => {
    await debugSafePoint({
      workflowId,
      stepId: "route-entry",
      kind: "request",
      flowStage: "api",
      resource: "GET api shipments by id",
      source: { relativePath: "src/server.js", line: 8 },
    });
    const result = await getShipment(params.shipmentId, workflowId, "route-entry");
    await debugSafePoint({
      workflowId,
      stepId: "response-write",
      parentStepId: "branch-shipment-found",
      kind: "response",
      flowStage: "response",
      resource: "HTTP shipment response",
      source: { relativePath: "src/server.js", line: 19 },
      dataPreview: {
        kind: "object",
        typeName: "ShipmentResponse",
        fields: [
          { name: "status", valueType: "number", nullable: false },
          { name: "body", valueType: "object", nullable: false },
        ],
        itemCount: 2,
        nullCount: 0,
        truncated: false,
      },
      terminal: "workflowCompleted",
    });
    response.writeHead(result.status, { "content-type": "application/json" });
    response.end(JSON.stringify(result.body));
  });
});

router.get("/api/debug/stress", async (_context, response) => {
  await withDebugWorkflow(async (workflowId) => {
    await debugSafePoint({
      workflowId,
      stepId: "stress-entry",
      kind: "request",
      flowStage: "api",
      resource: "GET api debug stress",
      source: { relativePath: "src/server.js", line: 47 },
    });
    let parentStepId = "stress-entry";
    for (let index = 0; index < 120; index += 1) {
      const stepId = `stress-line-${index}`;
      await debugSafePoint({
        workflowId,
        stepId,
        parentStepId,
        kind: "line",
        flowStage: "backend",
        source: { relativePath: "src/server.js", line: 58 },
      });
      parentStepId = stepId;
    }
    await debugSafePoint({
      workflowId,
      stepId: "stress-response",
      parentStepId,
      kind: "response",
      flowStage: "response",
      resource: "HTTP stress response",
      source: { relativePath: "src/server.js", line: 68 },
      terminal: "workflowCompleted",
    });
    response.writeHead(204);
    response.end();
  });
});

router.get("/api/debug/fail", async (_context, response) => {
  try {
    await withDebugWorkflow(async (workflowId) => {
      await debugSafePoint({
        workflowId,
        stepId: "failure-entry",
        kind: "request",
        flowStage: "api",
        resource: "GET api debug fail",
        source: { relativePath: "src/server.js", line: 87 },
      });
      throw new Error("fixture failure value stays outside the debug protocol");
    });
  } catch {
    response.writeHead(500);
    response.end();
  }
});

const port = Number(process.env.PORT ?? 4310);
const server = http.createServer((request, response) => router.dispatch(request, response));

server.listen(port, "127.0.0.1", () => {
  const address = server.address();
  console.log(`trace-demo listening on http://127.0.0.1:${address.port}`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
