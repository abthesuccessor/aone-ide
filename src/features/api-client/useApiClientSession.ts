import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiRequest, ApiResponse } from "../../types";
import {
  applyEndpointToDraft,
  buildApiRequest,
  createApiClientDraft,
  type ApiClientDraft,
  type ApiClientProtocol,
  type ApiClientTarget,
} from "./model";

export interface UseApiClientSessionOptions {
  onSendRequest: (request: ApiRequest) => Promise<ApiResponse>;
  debugActive?: boolean;
  target?: ApiClientTarget | null;
  initialDraft?: Partial<ApiClientDraft>;
  scopeKey?: string | number;
}

export interface ApiClientSession {
  draft: ApiClientDraft;
  lastRequest: ApiRequest | null;
  response: ApiResponse | null;
  requestError: string | null;
  sending: boolean;
  updateDraft: (patch: Partial<ApiClientDraft>) => void;
  setProtocol: (protocol: ApiClientProtocol) => void;
  applyTarget: (target: ApiClientTarget) => void;
  send: () => Promise<ApiResponse | null>;
}

function graphqlUrl(currentUrl: string): string {
  try {
    return `${new URL(currentUrl).origin}/graphql`;
  } catch {
    return "http://127.0.0.1:3000/graphql";
  }
}

function withJsonContentType(headers: string): string {
  if (headers.split("\n").some((line) => line.trim().toLocaleLowerCase().startsWith("content-type:"))) {
    return headers;
  }
  return [headers.trim(), "content-type: application/json"].filter(Boolean).join("\n");
}

function targetIdentity(target: ApiClientTarget | null | undefined): string | null {
  if (!target) return null;
  return [target.id, target.method, target.path, target.protocol ?? ""].join("\u0000");
}

export function useApiClientSession({
  onSendRequest,
  debugActive = false,
  target,
  initialDraft,
  scopeKey,
}: UseApiClientSessionOptions): ApiClientSession {
  const [draft, setDraft] = useState<ApiClientDraft>(() => createApiClientDraft(initialDraft));
  const [lastRequest, setLastRequest] = useState<ApiRequest | null>(null);
  const [response, setResponse] = useState<ApiResponse | null>(null);
  const [sending, setSending] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);
  const appliedTargetRef = useRef<string | null>(null);
  const requestEpochRef = useRef(0);
  const scopeKeyRef = useRef(scopeKey);
  const identity = targetIdentity(target);

  const updateDraft = useCallback((patch: Partial<ApiClientDraft>) => {
    setDraft((current) => ({ ...current, ...patch }));
  }, []);

  const setProtocol = useCallback((protocol: ApiClientProtocol) => {
    setDraft((current) => ({
      ...current,
      protocol,
      url: protocol === "graphql" && !current.url.includes("graphql")
        ? graphqlUrl(current.url)
        : current.url,
      headers: protocol === "graphql" ? withJsonContentType(current.headers) : current.headers,
    }));
  }, []);

  const applyTarget = useCallback((nextTarget: ApiClientTarget) => {
    requestEpochRef.current += 1;
    appliedTargetRef.current = targetIdentity(nextTarget);
    setDraft((current) => applyEndpointToDraft(current, nextTarget));
    setLastRequest(null);
    setResponse(null);
    setRequestError(null);
    setSending(false);
  }, []);

  const resetSession = useCallback(() => {
    requestEpochRef.current += 1;
    appliedTargetRef.current = null;
    setDraft(createApiClientDraft());
    setLastRequest(null);
    setResponse(null);
    setRequestError(null);
    setSending(false);
  }, []);

  useEffect(() => {
    if (Object.is(scopeKeyRef.current, scopeKey)) return;
    scopeKeyRef.current = scopeKey;
    resetSession();
  }, [resetSession, scopeKey]);

  useEffect(() => {
    if (!target || !identity) {
      if (appliedTargetRef.current !== null) resetSession();
      return;
    }
    if (appliedTargetRef.current === identity) return;
    applyTarget(target);
  }, [applyTarget, identity, resetSession, target]);

  const send = useCallback(async (): Promise<ApiResponse | null> => {
    const requestEpoch = requestEpochRef.current + 1;
    requestEpochRef.current = requestEpoch;
    setSending(true);
    setRequestError(null);
    try {
      const request = buildApiRequest(draft, debugActive);
      setLastRequest(request);
      const next = await onSendRequest(request);
      if (requestEpochRef.current === requestEpoch) setResponse(next);
      return next;
    } catch (error) {
      if (requestEpochRef.current === requestEpoch) {
        setRequestError(error instanceof Error ? error.message : "Request failed");
      }
      return null;
    } finally {
      if (requestEpochRef.current === requestEpoch) setSending(false);
    }
  }, [debugActive, draft, onSendRequest]);

  return {
    draft,
    lastRequest,
    response,
    requestError,
    sending,
    updateDraft,
    setProtocol,
    applyTarget,
    send,
  };
}
