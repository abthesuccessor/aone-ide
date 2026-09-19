import { useCallback, useRef, useState } from "react";
import type { MutableRefObject } from "react";
import type { ToastTone } from "../components/Toast";
import {
  configureAiCli,
  type SupportedCliAdapter,
  configureHostedAi,
  configureOllama,
  getAiConfigurationStatus,
  listAiCliAdapters,
  pickAndLoadEnvFile,
} from "../lib/bridge";
import { failureMessage } from "../lib/errorMessage";
import type {
  AiCliAdapterStatus,
  AiConfigurationStatus,
  ConfigureHostedAiRequest,
  ConfigureOllamaRequest,
  EnvLoadResult,
} from "../types";

type ConfigurationAction = "hosted" | "local" | "env" | "cli" | "cli-list" | null;

interface UseAiConfigurationOptions {
  operationBlockedRef: MutableRefObject<boolean>;
  notify: (text: string, tone?: ToastTone) => void;
}

function configuredMessage(status: AiConfigurationStatus): string {
  const provider = status.provider === "openai" ? "OpenAI"
    : status.provider === "anthropic" ? "Anthropic"
      : status.provider === "ollama" ? "Ollama"
        : status.provider === "codex" ? "Codex CLI"
          : status.provider === "claude" ? "Claude Code" : "AI provider";
  return `Configured ${provider} with ${status.model ?? "its default model"}`;
}

export function useAiConfiguration({ operationBlockedRef, notify }: UseAiConfigurationOptions) {
  const [configuration, setConfiguration] = useState<AiConfigurationStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [env, setEnv] = useState<EnvLoadResult | null>(null);
  const [action, setAction] = useState<ConfigurationAction>(null);
  const actionRef = useRef<ConfigurationAction>(null);
  const [cliAdapters, setCliAdapters] = useState<AiCliAdapterStatus[]>([]);

  const acceptStatus = useCallback((status: AiConfigurationStatus) => {
    setConfiguration(status);
    setError(null);
    if (status.inferenceAvailable) notify(configuredMessage(status), "success");
  }, [notify]);

  const run = useCallback(async (
    nextAction: Exclude<ConfigurationAction, "cli-list" | null>,
    operation: () => Promise<AiConfigurationStatus | null>,
  ) => {
    if (operationBlockedRef.current || actionRef.current !== null) return;
    actionRef.current = nextAction;
    setAction(nextAction);
    setError(null);
    try {
      const status = await operation();
      if (status) acceptStatus(status);
    } catch (cause) {
      const message = failureMessage(cause, "AI provider configuration failed");
      setError(message);
      notify(message, "error");
    } finally {
      actionRef.current = null;
      setAction(null);
    }
  }, [acceptStatus, notify, operationBlockedRef]);

  const configureHosted = useCallback((request: ConfigureHostedAiRequest) => run(
    "hosted",
    () => configureHostedAi(request),
  ), [run]);

  const configureLocal = useCallback((request: ConfigureOllamaRequest) => run(
    "local",
    () => configureOllama(request),
  ), [run]);

  const chooseEnv = useCallback(() => run("env", async () => {
    const next = await pickAndLoadEnvFile();
    if (!next) return null;
    setEnv(next);
    const status = await getAiConfigurationStatus();
    if (!status.inferenceAvailable) {
      throw new Error("The selected file does not contain a usable OpenAI or Anthropic configuration");
    }
    return status;
  }), [run]);

  const refreshCliAdapters = useCallback(async () => {
    if (operationBlockedRef.current || actionRef.current !== null) return;
    actionRef.current = "cli-list";
    setAction("cli-list");
    setError(null);
    try {
      setCliAdapters(await listAiCliAdapters());
    } catch (cause) {
      const message = failureMessage(cause, "Could not inspect coding agent CLIs");
      setError(message);
    } finally {
      actionRef.current = null;
      setAction(null);
    }
  }, [operationBlockedRef]);

  const configureCli = useCallback((adapter: SupportedCliAdapter) => run(
    "cli",
    () => configureAiCli(adapter),
  ), [run]);

  return {
    configuration,
    setConfiguration,
    error,
    setError,
    env,
    loading: action !== null,
    action,
    cliAdapters,
    configureHosted,
    configureLocal,
    chooseEnv,
    refreshCliAdapters,
    configureCli,
  };
}
