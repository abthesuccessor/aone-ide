import { describe, expect, it } from "vitest";
import {
  applyEndpointToDraft,
  buildApiRequest,
  createApiClientDraft,
  findUnresolvedRoutePlaceholders,
  parseApiBody,
  parseApiHeaders,
} from "./model";

describe("API client model", () => {
  it("starts from a neutral local request instead of a project-specific demo", () => {
    const draft = createApiClientDraft();
    expect(draft).toEqual({
      protocol: "rest",
      method: "GET",
      url: "http://127.0.0.1:3000/",
      headers: "",
      restBody: "",
      graphqlBody: "query {\n  __typename\n}",
      graphqlVariables: "{}",
    });
    expect(JSON.stringify(draft)).not.toMatch(/checkout|cart_73/i);
  });

  it("parses headers without losing colons in their values", () => {
    expect(parseApiHeaders("content-type: application/json\nauthorization: Bearer a:b\ninvalid")).toEqual([
      { name: "content-type", value: "application/json" },
      { name: "authorization", value: "Bearer a:b" },
    ]);
  });

  it("builds REST and GraphQL requests with bounded debug timeouts", () => {
    const rest = buildApiRequest(createApiClientDraft({
      method: "PATCH",
      restBody: '{"enabled":true}',
    }), false);
    expect(rest).toMatchObject({
      method: "PATCH",
      body: '{"enabled":true}',
      timeoutMs: 15_000,
    });

    const graphql = buildApiRequest(createApiClientDraft({
      protocol: "graphql",
      graphqlBody: "query Project { project { id } }",
      graphqlVariables: '{"id":"project-1"}',
    }), true);
    expect(graphql.method).toBe("POST");
    expect(graphql.timeoutMs).toBe(60_000);
    expect(JSON.parse(graphql.body ?? "{}")).toEqual({
      query: "query Project { project { id } }",
      variables: { id: "project-1" },
    });
  });

  it("applies a bodyless endpoint with a clean payload while preserving the current origin", () => {
    const current = createApiClientDraft({
      url: "http://127.0.0.1:4310/previous?old=true",
      headers: "authorization: Bearer stale\ncontent-type: text/plain",
      restBody: '{"stale":true}',
      graphqlBody: "query Stale { stale }",
      graphqlVariables: '{"stale":true}',
    });
    expect(applyEndpointToDraft(current, {
      id: "shipments",
      method: "get",
      path: "/api/shipments/{shipmentId}?details=true",
      protocol: "http",
    })).toMatchObject({
      protocol: "rest",
      method: "GET",
      url: "http://127.0.0.1:4310/api/shipments/{shipmentId}?details=true",
      headers: "",
      restBody: "",
      graphqlBody: "query {\n  __typename\n}",
      graphqlVariables: "{}",
    });
  });

  it("gives body methods an empty JSON object and content type", () => {
    const current = createApiClientDraft({
      url: "http://127.0.0.1:4310/previous",
      headers: "x-stale: true",
      restBody: '{"stale":true}',
    });
    expect(applyEndpointToDraft(current, {
      id: "create-shipment",
      method: "post",
      path: "/api/shipments",
    })).toMatchObject({
      method: "POST",
      url: "http://127.0.0.1:4310/api/shipments",
      headers: "content-type: application/json",
      restBody: "{}",
    });
    expect(applyEndpointToDraft(current, {
      id: "external",
      method: "post",
      path: "https://service.example/v1/run",
    }).url).toBe("https://service.example/v1/run");
  });

  it.each(["GET", "HEAD", "OPTIONS"])("removes the request body for %s endpoints", (method) => {
    const draft = applyEndpointToDraft(createApiClientDraft({ restBody: "stale" }), {
      id: method,
      method,
      path: "/health/{probe}",
    });
    expect(draft.restBody).toBe("");
    expect(draft.headers).toBe("");
  });

  it("never sends a stale body after a request is changed to a bodyless method", () => {
    const request = buildApiRequest(createApiClientDraft({
      method: "get",
      restBody: '{"stale":true}',
    }), false);

    expect(request.method).toBe("GET");
    expect(request.body).toBeUndefined();
  });

  it("finds route placeholders and refuses to build a literal placeholder request", () => {
    const url = "http://[::1]:4310/teams/:teamId/orders/{orderId}?cursor=:cursor";
    expect(new Set(findUnresolvedRoutePlaceholders(url))).toEqual(new Set([
      ":teamId",
      "{orderId}",
      ":cursor",
    ]));
    expect(findUnresolvedRoutePlaceholders("http://[::1]:4310/orders/123?cursor=next")).toEqual([]);
    expect(() => buildApiRequest(createApiClientDraft({ url }), false))
      .toThrow("Resolve route parameters before sending: {orderId}, :teamId, :cursor");
  });

  it("classifies JSON, text, and empty bodies without changing their values", () => {
    expect(parseApiBody('{"rows":[1]}')).toEqual({ kind: "json", value: { rows: [1] } });
    expect(parseApiBody("plain response")).toEqual({ kind: "text", value: "plain response" });
    expect(parseApiBody("  ")).toEqual({ kind: "empty", value: null });
  });
});
