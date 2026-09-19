import { describe, expect, it } from "vitest";
import type { ApiRequest, ApiResponse } from "../types";
import { createBrowserHttpEvent } from "./runtimeEvents";

describe("browser HTTP runtime evidence", () => {
  it("captures only the method/path envelope and never asserts a source node", () => {
    const request: ApiRequest = {
      method: "POST",
      url: "https://example.test/orders?token=secret",
      headers: [{ name: "Authorization", value: "Bearer secret" }],
      body: "private body",
      timeoutMs: 1_000,
    };
    const response: ApiResponse = {
      requestId: "request-1",
      status: 201,
      statusText: "Created",
      headers: [],
      body: "response body",
      durationMs: 12,
      truncated: false,
    };
    const event = createBrowserHttpEvent(request, response);
    expect(event.sourceNodeId).toBeUndefined();
    expect(event.metadata).toMatchObject({ method: "POST", path: "/orders", status: 201 });
    expect(JSON.stringify(event)).not.toContain("secret");
    expect(JSON.stringify(event)).not.toContain("private body");
    expect(JSON.stringify(event)).not.toContain("response body");
  });
});
