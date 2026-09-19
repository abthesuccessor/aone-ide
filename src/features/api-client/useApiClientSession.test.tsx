import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApiRequest, ApiResponse } from "../../types";
import type { ApiClientTarget } from "./model";
import { useApiClientSession } from "./useApiClientSession";

const response: ApiResponse = {
  requestId: "request-1",
  status: 200,
  statusText: "OK",
  headers: [{ name: "content-type", value: "application/json" }],
  body: '{"ok":true}',
  durationMs: 18,
  truncated: false,
};

describe("useApiClientSession", () => {
  it("publishes the exact request before awaiting its response", async () => {
    let resolveResponse: ((value: ApiResponse) => void) | undefined;
    const onSendRequest = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>(() => (
      new Promise<ApiResponse>((resolve) => { resolveResponse = resolve; })
    ));
    const { result } = renderHook(() => useApiClientSession({
      onSendRequest,
      debugActive: true,
      initialDraft: {
        method: "GET",
        url: "http://127.0.0.1:4310/api/shipments/SHP-1047",
        restBody: "",
      },
    }));

    let pending: Promise<ApiResponse | null> | undefined;
    act(() => { pending = result.current.send(); });

    expect(result.current.sending).toBe(true);
    expect(result.current.lastRequest).toMatchObject({
      method: "GET",
      url: "http://127.0.0.1:4310/api/shipments/SHP-1047",
      body: undefined,
      timeoutMs: 60_000,
    });
    expect(result.current.response).toBeNull();

    await act(async () => {
      resolveResponse?.(response);
      await pending;
    });
    expect(result.current.sending).toBe(false);
    expect(result.current.response).toEqual(response);
  });

  it("applies a changed endpoint once and preserves later payload edits", async () => {
    const onSendRequest = vi.fn().mockResolvedValue(response);
    const initialTarget = {
      id: "shipments",
      method: "GET",
      path: "/api/shipments/{id}",
      protocol: "http",
    };
    const { result, rerender } = renderHook(
      ({ target }) => useApiClientSession({
        onSendRequest,
        target,
        initialDraft: { url: "http://127.0.0.1:4310/start" },
      }),
      { initialProps: { target: initialTarget } },
    );

    await waitFor(() => expect(result.current.draft.url).toBe("http://127.0.0.1:4310/api/shipments/{id}"));
    act(() => result.current.updateDraft({ restBody: '{"manual":true}' }));
    rerender({ target: { ...initialTarget } });
    expect(result.current.draft.restBody).toBe('{"manual":true}');
  });

  it("resets stale request state and payload when the selected endpoint changes", async () => {
    const onSendRequest = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>()
      .mockResolvedValueOnce(response)
      .mockRejectedValueOnce(new Error("old endpoint failed"));
    const firstTarget: ApiClientTarget = {
      id: "orders",
      method: "POST",
      path: "/api/orders",
    };
    const secondTarget: ApiClientTarget = {
      id: "shipments",
      method: "POST",
      path: "/api/shipments",
    };
    const { result, rerender } = renderHook(
      ({ target }: { target: ApiClientTarget }) => useApiClientSession({
        onSendRequest,
        target,
        initialDraft: { url: "http://127.0.0.1:4310/start" },
      }),
      { initialProps: { target: firstTarget } },
    );

    await waitFor(() => expect(result.current.draft.url).toBe("http://127.0.0.1:4310/api/orders"));
    act(() => result.current.updateDraft({
      headers: "x-stale: true",
      restBody: '{"stale":true}',
    }));
    await act(async () => { await result.current.send(); });
    expect(result.current.response).toEqual(response);
    expect(result.current.lastRequest).not.toBeNull();

    await act(async () => { await result.current.send(); });
    expect(result.current.requestError).toBe("old endpoint failed");
    rerender({ target: secondTarget });

    await waitFor(() => expect(result.current.draft.url).toBe("http://127.0.0.1:4310/api/shipments"));
    expect(result.current.draft).toMatchObject({
      method: "POST",
      headers: "content-type: application/json",
      restBody: "{}",
    });
    expect(result.current.lastRequest).toBeNull();
    expect(result.current.response).toBeNull();
    expect(result.current.requestError).toBeNull();
  });

  it("clears the prior workspace request and ignores its late response when the scope changes", async () => {
    let resolveResponse: ((value: ApiResponse) => void) | undefined;
    const onSendRequest = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>(() => (
      new Promise<ApiResponse>((resolve) => { resolveResponse = resolve; })
    ));
    const authTarget: ApiClientTarget = {
      id: "auth-login",
      method: "POST",
      path: "/api/v1/login",
    };
    const { result, rerender } = renderHook(
      ({ scopeKey, target }: { scopeKey: string; target: ApiClientTarget | null }) => (
        useApiClientSession({ onSendRequest, scopeKey, target })
      ),
      { initialProps: { scopeKey: "auth:1", target: authTarget as ApiClientTarget | null } },
    );

    await waitFor(() => expect(result.current.draft.url).toBe("http://127.0.0.1:3000/api/v1/login"));
    act(() => result.current.updateDraft({
      headers: "authorization: stale",
      restBody: '{"username":"stale"}',
    }));
    let pending: Promise<ApiResponse | null> | undefined;
    act(() => { pending = result.current.send(); });
    expect(result.current.sending).toBe(true);

    rerender({ scopeKey: "trace-demo:2", target: authTarget });
    rerender({ scopeKey: "trace-demo:2", target: null });

    await waitFor(() => expect(result.current.draft).toMatchObject({
      protocol: "rest",
      method: "GET",
      url: "http://127.0.0.1:3000/",
      headers: "",
      restBody: "",
    }));
    expect(result.current.lastRequest).toBeNull();
    expect(result.current.response).toBeNull();
    expect(result.current.requestError).toBeNull();
    expect(result.current.sending).toBe(false);

    await act(async () => {
      resolveResponse?.(response);
      await pending;
    });
    expect(result.current.lastRequest).toBeNull();
    expect(result.current.response).toBeNull();
    expect(result.current.sending).toBe(false);
  });

  it("does not dispatch a request with unresolved route parameters", async () => {
    const onSendRequest = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>().mockResolvedValue(response);
    const { result } = renderHook(() => useApiClientSession({
      onSendRequest,
      target: {
        id: "shipment",
        method: "GET",
        path: "/api/shipments/:shipmentId",
      },
      initialDraft: { url: "http://127.0.0.1:4310/start" },
    }));

    await waitFor(() => expect(result.current.draft.url).toContain(":shipmentId"));
    await act(async () => { await result.current.send(); });

    expect(onSendRequest).not.toHaveBeenCalled();
    expect(result.current.lastRequest).toBeNull();
    expect(result.current.requestError).toBe("Resolve route parameters before sending: :shipmentId");
  });

  it("surfaces request construction and transport failures", async () => {
    const onSendRequest = vi.fn().mockRejectedValue(new Error("connection refused"));
    const { result } = renderHook(() => useApiClientSession({ onSendRequest }));

    await act(async () => { await result.current.send(); });
    expect(result.current.requestError).toBe("connection refused");
    expect(result.current.sending).toBe(false);

    act(() => {
      result.current.setProtocol("graphql");
      result.current.updateDraft({ graphqlVariables: "not json" });
    });
    await act(async () => { await result.current.send(); });
    expect(result.current.requestError).toMatch(/Unexpected token|JSON/);
  });
});
