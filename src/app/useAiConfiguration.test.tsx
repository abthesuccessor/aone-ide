import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAiConfiguration } from "./useAiConfiguration";

const bridge = vi.hoisted(() => ({
  configureAiCli: vi.fn(),
  configureHostedAi: vi.fn(),
  configureOllama: vi.fn(),
  getAiConfigurationStatus: vi.fn(),
  listAiCliAdapters: vi.fn(),
  pickAndLoadEnvFile: vi.fn(),
}));

vi.mock("../lib/bridge", () => bridge);

const hostedStatus = {
  configured: true,
  provider: "openai" as const,
  model: "gpt-test",
  transport: "api" as const,
  inferenceAvailable: true,
};

function renderConfiguration() {
  const notify = vi.fn();
  const operationBlockedRef = { current: false };
  return {
    notify,
    ...renderHook(() => useAiConfiguration({ operationBlockedRef, notify })),
  };
}

beforeEach(() => vi.clearAllMocks());

describe("useAiConfiguration", () => {
  it("installs a direct hosted credential without retaining it in safe status", async () => {
    bridge.configureHostedAi.mockResolvedValueOnce(hostedStatus);
    const harness = renderConfiguration();

    await act(async () => harness.result.current.configureHosted({
      provider: "openai",
      apiKey: "private-test-value",
      model: "gpt-test",
    }));

    expect(bridge.configureHostedAi).toHaveBeenCalledWith({
      provider: "openai",
      apiKey: "private-test-value",
      model: "gpt-test",
    });
    expect(harness.result.current.configuration).toEqual(hostedStatus);
    expect(JSON.stringify(harness.result.current.configuration)).not.toContain("private-test-value");
    expect(harness.notify).toHaveBeenCalledWith("Configured OpenAI with gpt-test", "success");
  });

  it("keeps the gate closed and exposes a safe native error", async () => {
    bridge.configureHostedAi.mockRejectedValueOnce("OpenAI credential was rejected");
    const harness = renderConfiguration();

    await act(async () => harness.result.current.configureHosted({
      provider: "openai",
      apiKey: "bad-test-value",
    }));

    expect(harness.result.current.configuration).toBeNull();
    expect(harness.result.current.error).toBe("OpenAI credential was rejected");
    expect(harness.result.current.loading).toBe(false);
  });

  it("treats private environment import as a secondary provider source", async () => {
    bridge.pickAndLoadEnvFile.mockResolvedValueOnce({
      loadedCount: 2,
      names: ["OPENAI_API_KEY", "OPENAI_MODEL"],
    });
    bridge.getAiConfigurationStatus.mockResolvedValueOnce(hostedStatus);
    const harness = renderConfiguration();

    await act(async () => harness.result.current.chooseEnv());

    expect(harness.result.current.env?.names).toEqual(["OPENAI_API_KEY", "OPENAI_MODEL"]);
    expect(harness.result.current.configuration?.inferenceAvailable).toBe(true);
  });

  it("does not overlap two native configuration operations", async () => {
    let resolveHosted: ((value: typeof hostedStatus) => void) | undefined;
    bridge.configureHostedAi.mockReturnValueOnce(new Promise((resolve) => { resolveHosted = resolve; }));
    const harness = renderConfiguration();

    act(() => {
      void harness.result.current.configureHosted({ provider: "openai", apiKey: "first" });
      void harness.result.current.configureHosted({ provider: "openai", apiKey: "second" });
    });
    expect(bridge.configureHostedAi).toHaveBeenCalledTimes(1);
    await act(async () => resolveHosted?.(hostedStatus));
    await waitFor(() => expect(harness.result.current.loading).toBe(false));
  });
});
