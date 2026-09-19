import { isTauriRuntime } from "../../lib/bridge";
import type {
  ListApiEndpoints200Response as GeneratedApiEndpointPage,
  ListApiEndpointsRequest as GeneratedListApiEndpointsArgs,
} from "../../generated/ipc";
import type {
  ApiEndpointInventoryItem,
  ApiEndpointInventoryPage,
  ApiEndpointInventoryRequest,
} from "./model";

const demoEndpoints: ApiEndpointInventoryItem[] = [
  {
    id: "api:post:/api/checkout",
    label: "Create checkout",
    method: "POST",
    path: "/api/checkout",
    role: "producer",
    protocol: "http",
    sourceFile: "openapi/checkout.yaml",
    source: { relativePath: "openapi/checkout.yaml", startLine: 6, startColumn: 5, endLine: 11, endColumn: 1 },
    language: "yaml",
    framework: "OpenAPI",
    operationId: "createCheckout",
    evidence: "declared",
    grouping: { key: "Checkout API / checkout", segments: ["Checkout API", "checkout"], basis: "openApiTag" },
    handler: {
      id: "endpoint:checkout",
      kind: "endpoint",
      label: "checkoutRouter.post",
      source: { relativePath: "src/api/routes/checkout.ts", startLine: 6, startColumn: 1, endLine: 10, endColumn: 1 },
      evidence: "resolved",
    },
    clientCoverage: {
      count: 1,
      firstSource: { relativePath: "src/web/api/checkoutClient.ts", startLine: 3, startColumn: 1, endLine: 11, endColumn: 1 },
    },
    occurrenceCount: 3,
  },
  {
    id: "api:post:https://api.stripe.com/checkout/sessions",
    label: "Stripe checkout request",
    method: "POST",
    path: "https://api.stripe.com/checkout/sessions",
    role: "consumer",
    protocol: "http",
    sourceFile: "src/api/clients/PaymentGateway.ts",
    source: { relativePath: "src/api/clients/PaymentGateway.ts", startLine: 6, startColumn: 1, endLine: 13, endColumn: 1 },
    language: "typescript",
    framework: "Stripe SDK",
    evidence: "resolved",
    grouping: { key: "src / api / clients", segments: ["src", "api", "clients"], basis: "sourcePath" },
    clientCoverage: {
      count: 1,
      firstSource: { relativePath: "src/api/clients/PaymentGateway.ts", startLine: 6, startColumn: 1, endLine: 13, endColumn: 1 },
    },
    occurrenceCount: 1,
  },
];

function demoPage(request: ApiEndpointInventoryRequest): ApiEndpointInventoryPage {
  const query = request.query?.trim().toLocaleLowerCase();
  const filtered = demoEndpoints.filter((endpoint) => !query || [
    endpoint.method,
    endpoint.path,
    endpoint.label,
    endpoint.sourceFile,
    endpoint.operationId,
    endpoint.handler?.label,
    endpoint.handler?.kind,
  ].filter(Boolean).join(" ").toLocaleLowerCase().includes(query));
  const offset = Math.max(0, Number.parseInt(request.cursor ?? "0", 10) || 0);
  const limit = Math.max(1, Math.min(request.limit ?? 100, 200));
  const endpoints = filtered.slice(offset, offset + limit);
  const nextOffset = offset + endpoints.length;
  const truncated = nextOffset < filtered.length;
  return {
    workspaceId: request.workspaceId,
    endpoints,
    total: filtered.length,
    indexedTotal: demoEndpoints.length,
    returned: endpoints.length,
    nextCursor: truncated ? String(nextOffset) : undefined,
    truncated,
    counts: {
      producer: filtered.filter((endpoint) => endpoint.role === "producer").length,
      consumerOnly: filtered.filter((endpoint) => endpoint.role === "consumer").length,
      clientCovered: filtered.filter((endpoint) => (
        endpoint.role === "producer" && endpoint.clientCoverage.count > 0
      )).length,
      unknown: filtered.filter((endpoint) => endpoint.role === "unknown").length,
    },
  };
}

export async function listApiEndpointsPage(
  request: ApiEndpointInventoryRequest,
): Promise<ApiEndpointInventoryPage> {
  if (!isTauriRuntime()) {
    await new Promise((resolve) => window.setTimeout(resolve, 70));
    return demoPage(request);
  }
  const { invoke } = await import("@tauri-apps/api/core");
  const args = { request } satisfies GeneratedListApiEndpointsArgs;
  return invoke<GeneratedApiEndpointPage>("list_api_endpoints", args);
}
