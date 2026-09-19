import type {
  AiExplanation,
  ApiRequest,
  ApiResponse,
  AppSnapshot,
  GraphSnapshot,
  RuntimeEvent,
  SourceFile,
  WorkspaceFile,
  WorkspaceSummary,
} from "../types";

export const demoWorkspace: WorkspaceSummary = {
  id: "workspace:demo-checkout",
  name: "checkout-platform",
  rootPath: "/Users/demo/checkout-platform",
  fileCount: 42,
  nodeCount: 9,
  edgeCount: 10,
  languages: [
    { language: "TypeScript", fileCount: 7, capability: "semantic" },
    { language: "SQL", fileCount: 1, capability: "syntaxOnly" },
    { language: "YAML", fileCount: 2, capability: "syntaxOnly" },
    { language: "Markdown", fileCount: 1, capability: "textOnly" },
  ],
  lastScannedAt: new Date().toISOString(),
};

function demoFile(relativePath: string, language: string, sizeBytes: number, capability: WorkspaceFile["capability"] = "semantic"): WorkspaceFile {
  return {
    id: `file:${relativePath}`,
    relativePath,
    language,
    capability,
    sizeBytes,
    contentHash: `demo-${relativePath.length}-${sizeBytes}`,
    modifiedAt: "2026-08-16T08:42:00.000Z",
    parseErrors: false,
  };
}

export const demoFiles: WorkspaceFile[] = [
  demoFile("src/web/CheckoutPage.tsx", "typescript", 2_894),
  demoFile("src/web/api/checkoutClient.ts", "typescript", 1_640),
  demoFile("src/api/routes/checkout.ts", "typescript", 2_318),
  demoFile("src/api/services/CheckoutService.ts", "typescript", 3_121),
  demoFile("src/api/repositories/OrderRepository.ts", "typescript", 1_954),
  demoFile("src/api/clients/PaymentGateway.ts", "typescript", 1_722),
  demoFile("src/telemetry/httpTracing.ts", "typescript", 1_137),
  demoFile("migrations/004_create_orders.sql", "sql", 870, "syntaxOnly"),
  demoFile("openapi/checkout.yaml", "yaml", 4_260, "syntaxOnly"),
  demoFile("docker-compose.yml", "yaml", 1_488, "syntaxOnly"),
  demoFile("README.md", "markdown", 3_590, "textOnly"),
];

const rawDemoSources: Record<string, Omit<SourceFile, "contentHash">> = {
  "src/web/CheckoutPage.tsx": {
    relativePath: "src/web/CheckoutPage.tsx",
    language: "typescript",
    content: `import { checkout } from "./api/checkoutClient";

export function CheckoutPage() {
  async function submitOrder(cartId: string) {
    const result = await checkout({ cartId });
    return result.redirectUrl;
  }

  return <button onClick={() => submitOrder("cart_73")}>Pay now</button>;
}
`,
  },
  "src/web/api/checkoutClient.ts": {
    relativePath: "src/web/api/checkoutClient.ts",
    language: "typescript",
    content: `export interface CheckoutInput { cartId: string }

export async function checkout(input: CheckoutInput) {
  const response = await fetch("/api/checkout", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
  if (!response.ok) throw new Error("Checkout failed");
  return response.json() as Promise<{ orderId: string; redirectUrl: string }>;
}
`,
  },
  "src/api/routes/checkout.ts": {
    relativePath: "src/api/routes/checkout.ts",
    language: "typescript",
    content: `import { Router } from "express";
import { CheckoutService } from "../services/CheckoutService";

export const checkoutRouter = Router();

checkoutRouter.post("/api/checkout", async (request, response) => {
  const service = new CheckoutService();
  const checkout = await service.createCheckout(request.body.cartId);
  response.status(201).json(checkout);
});
`,
  },
  "src/api/services/CheckoutService.ts": {
    relativePath: "src/api/services/CheckoutService.ts",
    language: "typescript",
    content: `import { OrderRepository } from "../repositories/OrderRepository";
import { PaymentGateway } from "../clients/PaymentGateway";

export class CheckoutService {
  constructor(
    private orders = new OrderRepository(),
    private payments = new PaymentGateway(),
  ) {}

  async createCheckout(cartId: string) {
    const order = await this.orders.createPending(cartId);
    const session = await this.payments.createSession(order);
    await this.orders.attachPayment(order.id, session.id);
    return { orderId: order.id, redirectUrl: session.url };
  }
}
`,
  },
  "src/api/repositories/OrderRepository.ts": {
    relativePath: "src/api/repositories/OrderRepository.ts",
    language: "typescript",
    content: `import { db } from "../database";

export class OrderRepository {
  async createPending(cartId: string) {
    return db.one(
      "insert into orders (cart_id, status) values ($1, 'pending') returning *",
      [cartId],
    );
  }

  async attachPayment(orderId: string, paymentId: string) {
    return db.none("update orders set payment_id = $2 where id = $1", [orderId, paymentId]);
  }
}
`,
  },
  "src/api/clients/PaymentGateway.ts": {
    relativePath: "src/api/clients/PaymentGateway.ts",
    language: "typescript",
    content: `import Stripe from "stripe";

export class PaymentGateway {
  private stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);

  async createSession(order: { id: string }) {
    return this.stripe.checkout.sessions.create({
      client_reference_id: order.id,
      mode: "payment",
      success_url: "http://localhost:5173/success",
    });
  }
}
`,
  },
  "migrations/004_create_orders.sql": {
    relativePath: "migrations/004_create_orders.sql",
    language: "sql",
    content: `create table orders (
  id uuid primary key default gen_random_uuid(),
  cart_id text not null,
  payment_id text,
  status text not null check (status in ('pending', 'paid', 'failed')),
  created_at timestamptz not null default now()
);

create index orders_cart_id_idx on orders(cart_id);
`,
  },
  "openapi/checkout.yaml": {
    relativePath: "openapi/checkout.yaml",
    language: "yaml",
    content: `openapi: 3.1.0
info:
  title: Checkout API
  version: 1.0.0
paths:
  /api/checkout:
    post:
      operationId: createCheckout
      responses:
        "201":
          description: Checkout created
`,
  },
  "docker-compose.yml": {
    relativePath: "docker-compose.yml",
    language: "yaml",
    content: `services:
  api:
    build: .
    ports: ["3000:3000"]
    depends_on: [postgres]
  postgres:
    image: postgres:17-alpine
    ports: ["5432:5432"]
`,
  },
  "README.md": {
    relativePath: "README.md",
    language: "markdown",
    content: `# Checkout platform

Local-first checkout service used by the Aone browser demo.

## Flow

The React client calls the checkout route, which delegates to a service, repository, PostgreSQL, and Stripe.
`,
  },
};

export const demoSources = Object.fromEntries(
  Object.entries(rawDemoSources).map(([relativePath, source]) => [
    relativePath,
    { ...source, contentHash: `demo-source-${relativePath.length}-${source.content.length}` },
  ]),
) as Record<string, SourceFile>;

function at(relativePath: string, startLine: number, endLine: number) {
  return { relativePath, startLine, startColumn: 1, endLine, endColumn: 1 };
}

export const demoGraph: GraphSnapshot = {
  truncated: false,
  nodes: [
    {
      id: "web:checkout-page",
      kind: "component",
      label: "CheckoutPage",
      language: "TypeScript",
      evidence: "declared",
      source: at("src/web/CheckoutPage.tsx", 3, 10),
      metadata: { layer: "Web", role: "UI entry" },
    },
    {
      id: "client:checkout",
      kind: "function",
      label: "checkout()",
      language: "TypeScript",
      evidence: "resolved",
      source: at("src/web/api/checkoutClient.ts", 3, 11),
      metadata: { layer: "Web", role: "HTTP client" },
    },
    {
      id: "endpoint:checkout",
      kind: "endpoint",
      label: "POST /api/checkout",
      language: "TypeScript",
      evidence: "declared",
      source: at("src/api/routes/checkout.ts", 6, 10),
      metadata: { layer: "API", method: "POST", route: "/api/checkout" },
    },
    {
      id: "service:checkout",
      kind: "service",
      label: "CheckoutService.createCheckout",
      language: "TypeScript",
      evidence: "resolved",
      source: at("src/api/services/CheckoutService.ts", 10, 16),
      metadata: { layer: "Domain", role: "orchestrator" },
    },
    {
      id: "repo:orders",
      kind: "repository",
      label: "OrderRepository",
      language: "TypeScript",
      evidence: "resolved",
      source: at("src/api/repositories/OrderRepository.ts", 3, 15),
      metadata: { layer: "Data", table: "orders" },
    },
    {
      id: "db:orders",
      kind: "database",
      label: "PostgreSQL · orders",
      language: "SQL",
      evidence: "declared",
      source: at("migrations/004_create_orders.sql", 1, 10),
      metadata: { layer: "Data", engine: "PostgreSQL" },
    },
    {
      id: "external:stripe",
      kind: "external-api",
      label: "Stripe Checkout",
      language: "HTTPS",
      evidence: "declared",
      source: at("src/api/clients/PaymentGateway.ts", 3, 13),
      metadata: { layer: "External", host: "api.stripe.com" },
    },
    {
      id: "trace:req-842",
      kind: "runtime-span",
      label: "req_842 · 201",
      evidence: "observed",
      metadata: { layer: "Runtime", durationMs: 184, traceId: "tr_01J8A2" },
    },
    {
      id: "inferred:retry",
      kind: "risk",
      label: "Missing retry boundary",
      evidence: "inferred",
      metadata: { layer: "Analysis", note: "No retry policy was resolved around Stripe", confidence: 0.73 },
    },
  ],
  edges: [
    { id: "e1", source: "web:checkout-page", target: "client:checkout", kind: "calls", evidence: "resolved", confidence: 1, metadata: {} },
    { id: "e2", source: "client:checkout", target: "endpoint:checkout", kind: "http", evidence: "resolved", confidence: 0.98, metadata: {} },
    { id: "e3", source: "endpoint:checkout", target: "service:checkout", kind: "calls", evidence: "resolved", confidence: 1, metadata: {} },
    { id: "e4", source: "service:checkout", target: "repo:orders", kind: "calls", evidence: "resolved", confidence: 0.96, metadata: {} },
    { id: "e5", source: "repo:orders", target: "db:orders", kind: "writes", evidence: "declared", confidence: 1, metadata: {} },
    { id: "e6", source: "service:checkout", target: "external:stripe", kind: "http", evidence: "resolved", confidence: 0.91, metadata: {} },
    { id: "e7", source: "trace:req-842", target: "endpoint:checkout", kind: "entered", evidence: "observed", confidence: 1, metadata: {} },
    { id: "e8", source: "trace:req-842", target: "db:orders", kind: "query · 18 ms", evidence: "observed", confidence: 1, metadata: {} },
    { id: "e9", source: "trace:req-842", target: "external:stripe", kind: "request · 126 ms", evidence: "observed", confidence: 1, metadata: {} },
    { id: "e10", source: "inferred:retry", target: "external:stripe", kind: "concerns", evidence: "inferred", confidence: 0.73, metadata: {} },
  ],
};

export const demoRuntimeEvents: RuntimeEvent[] = [
  {
    id: "rt-1",
    runId: "demo-run",
    timestamp: new Date(Date.now() - 4100).toISOString(),
    kind: "process.stdout",
    label: "API listening on http://127.0.0.1:3000",
    evidence: "observed",
    sourceNodeId: "endpoint:checkout",
    metadata: { stream: "stdout", profileId: "api-dev" },
  },
  {
    id: "rt-2",
    runId: "demo-run",
    timestamp: new Date(Date.now() - 2300).toISOString(),
    kind: "http.completed",
    label: "POST /api/checkout → 201",
    evidence: "observed",
    sourceNodeId: "endpoint:checkout",
    traceId: "tr_01J8A2",
    metadata: { durationMs: 184, status: 201, detail: "Response body bounded and sensitive fields redacted" },
  },
  {
    id: "rt-3",
    runId: "demo-run",
    timestamp: new Date(Date.now() - 2150).toISOString(),
    kind: "database.query",
    label: "INSERT orders",
    evidence: "observed",
    sourceNodeId: "db:orders",
    traceId: "tr_01J8A2",
    metadata: { durationMs: 18, parameters: "[REDACTED]" },
  },
];

export const demoRunProfiles = [
  { id: "api-dev", name: "API · dev", kind: "node", executable: "npm", args: ["run", "dev:api"], requiredEnv: ["DATABASE_URL", "STRIPE_SECRET_KEY"], source: "package.json#dev:api" },
  { id: "web-dev", name: "Web · dev", kind: "node", executable: "npm", args: ["run", "dev:web"], requiredEnv: [], source: "package.json#dev:web" },
  { id: "compose", name: "Local stack", kind: "dockerCompose", executable: "docker", args: ["compose", "up"], requiredEnv: ["DATABASE_URL"], source: "docker-compose.yml" },
];

export const demoAiEnv = {
  names: ["OPENAI_API_KEY", "OPENAI_MODEL"],
  loadedCount: 2,
};

export const demoRunEnv = {
  names: ["DATABASE_URL", "STRIPE_SECRET_KEY", "PORT", "LOG_LEVEL"],
  loadedCount: 4,
};

export const demoSnapshot: AppSnapshot = {
  version: "0.1.0-demo",
  workspace: demoWorkspace,
  runtimeEventCount: demoRuntimeEvents.length,
};

export function getDemoSource(relativePath: string): SourceFile {
  return demoSources[relativePath] ?? {
    relativePath,
    language: demoFiles.find((file) => file.relativePath === relativePath)?.language ?? "plaintext",
    content: `// Browser demo source for ${relativePath}\n// The desktop build reads this through read_workspace_file.\n`,
    contentHash: `demo-fallback-${relativePath.length}`,
  };
}

export function createDemoApiResponse(request: ApiRequest): ApiResponse {
  const isGraphql = request.url.includes("graphql") || request.body?.includes('"query"');
  return {
    requestId: `request:demo-${Date.now()}`,
    status: 201,
    statusText: "Created",
    durationMs: 184,
    headers: [
      { name: "content-type", value: "application/json; charset=utf-8" },
      { name: "x-trace-id", value: "tr_01J8A2" },
    ],
    body: isGraphql
      ? JSON.stringify({ data: { checkout: { orderId: "ord_0189", status: "pending" } } }, null, 2)
      : JSON.stringify({ orderId: "ord_0189", redirectUrl: "https://checkout.stripe.com/redacted" }, null, 2),
    truncated: false,
  };
}

export const demoExplanation: AiExplanation = {
  answer: "The checkout request crosses the declared route, a resolved service and repository chain, then PostgreSQL and Stripe. The observed request event confirms the endpoint ran, while the missing retry boundary remains only an inference.",
  evidence: [
    { kind: "node", id: "endpoint:checkout", label: "POST /api/checkout", evidence: "declared" },
    { kind: "node", id: "service:checkout", label: "CheckoutService.createCheckout", evidence: "resolved" },
    { kind: "runtimeEvent", id: "rt-2", label: "POST /api/checkout → 201", evidence: "observed" },
    { kind: "node", id: "inferred:retry", label: "Missing retry boundary", evidence: "inferred" },
  ],
  model: "browser-demo",
};
