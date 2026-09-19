import {
  ArrowClockwise,
  CheckCircle,
  Cloud,
  CodeBlock,
  Eye,
  EyeSlash,
  FileLock,
  HardDrives,
  PlugsConnected,
  Robot,
  ShieldCheck,
  SpinnerGap,
  Warning,
} from "@phosphor-icons/react";
import { useRef, useState } from "react";
import type {
  AiCliAdapterStatus,
  AiConfigurationStatus,
  ConfigureHostedAiRequest,
  ConfigureOllamaRequest,
  HostedAiProvider,
} from "../types";
import { isSupportedCliAdapter, type SupportedCliAdapter } from "../lib/bridge";
import { AssistantSelect } from "./AssistantSelect";
import { DEFAULT_CLI_ADAPTERS, cliState } from "./assistantCliState";
import {
  DEFAULT_HOSTED_MODEL,
  HOSTED_MODEL_OPTIONS,
  HOSTED_PROVIDER_OPTIONS,
} from "./hostedAiOptions";

type SetupTab = "hosted" | "local" | "cli";
type PendingAction = "hosted" | "environment" | "ollama" | "refresh-cli" | "connect-cli";

interface AssistantSetupDialogProps {
  status: AiConfigurationStatus | null;
  loading: boolean;
  error: string | null;
  onConfigureHosted?: (request: ConfigureHostedAiRequest) => Promise<void>;
  onImportEnvironment?: () => Promise<void>;
  onConfigureOllama?: (request: ConfigureOllamaRequest) => Promise<void>;
  cliAdapters?: AiCliAdapterStatus[];
  onRefreshCliAdapters?: () => Promise<void>;
  onConnectCli?: (adapter: SupportedCliAdapter) => Promise<void>;
  onContinueCodeOnly?: () => void;
  /** Compatibility path while the shell migrates to onImportEnvironment. */
  onChooseFile?: () => void | Promise<void>;
}

const SETUP_TABS: Array<{ id: SetupTab; label: string }> = [
  { id: "hosted", label: "Hosted" },
  { id: "local", label: "Local" },
  { id: "cli", label: "CLI" },
];

function ProviderState({ label, tone = "muted" }: { label: string; tone?: "ready" | "muted" | "warning" }) {
  return <span className={`provider-state is-${tone}`}>{label}</span>;
}

function configurationLabel(status: AiConfigurationStatus | null): string {
  if (!status?.inferenceAvailable) return "Configuration required";
  if (status.provider === "anthropic") return `Anthropic configured with ${status.model}`;
  if (status.provider === "ollama") return `Ollama ready with ${status.model}`;
  if (status.provider === "codex") return "Codex CLI ready";
  if (status.provider === "claude") return "Claude Code ready";
  return `OpenAI configured with ${status.model}`;
}

export function AssistantSetupDialog({
  status,
  loading,
  error,
  onConfigureHosted,
  onImportEnvironment,
  onConfigureOllama,
  cliAdapters = DEFAULT_CLI_ADAPTERS,
  onRefreshCliAdapters,
  onConnectCli,
  onContinueCodeOnly,
  onChooseFile,
}: AssistantSetupDialogProps) {
  const [activeTab, setActiveTab] = useState<SetupTab>("hosted");
  const [provider, setProvider] = useState<HostedAiProvider>("openai");
  const [apiKey, setApiKey] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [hostedModel, setHostedModel] = useState(DEFAULT_HOSTED_MODEL.openai);
  const [ollamaEndpoint, setOllamaEndpoint] = useState("http://127.0.0.1:11434");
  const [ollamaModel, setOllamaModel] = useState("");
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const busy = loading || pending !== null;
  const ready = status?.inferenceAvailable === true;
  const hostedConfigured = ready && (status?.provider === "openai" || status?.provider === "anthropic");
  const importEnvironment = onImportEnvironment ?? (onChooseFile
    ? async () => { await onChooseFile(); }
    : undefined);

  const handleProviderChange = (nextProvider: HostedAiProvider) => {
    setProvider(nextProvider);
    setHostedModel(DEFAULT_HOSTED_MODEL[nextProvider]);
  };

  const selectTab = (tab: SetupTab, focus = false) => {
    setActiveTab(tab);
    setLocalError(null);
    if (focus) tabRefs.current[SETUP_TABS.findIndex((item) => item.id === tab)]?.focus();
  };

  const handleTabKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % SETUP_TABS.length;
    else if (event.key === "ArrowLeft") nextIndex = (index - 1 + SETUP_TABS.length) % SETUP_TABS.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = SETUP_TABS.length - 1;
    else return;
    event.preventDefault();
    const nextTab = SETUP_TABS[nextIndex];
    if (nextTab) selectTab(nextTab.id, true);
  };

  const handleHostedSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!onConfigureHosted) {
      setLocalError("The hosted API adapter is not connected to the native shell.");
      setApiKey("");
      return;
    }
    if (!apiKey.trim()) {
      setLocalError("Enter an API key.");
      return;
    }

    setPending("hosted");
    setLocalError(null);
    try {
      await onConfigureHosted({
        provider,
        apiKey,
        model: hostedModel,
      });
    } catch {
      setLocalError("Aone could not configure this hosted provider. Check the fields and try again.");
    } finally {
      setApiKey("");
      setPending(null);
    }
  };

  const handleEnvironmentImport = async () => {
    if (!importEnvironment) {
      setLocalError("Environment-file import is not connected to the native shell.");
      return;
    }
    setPending("environment");
    setLocalError(null);
    try {
      await importEnvironment();
    } catch {
      setLocalError("Aone could not import that environment file. Check its provider fields and try again.");
    } finally {
      setPending(null);
    }
  };

  const handleOllamaSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!onConfigureOllama) {
      setLocalError("The Ollama adapter is not connected to the native shell.");
      return;
    }
    if (!ollamaModel.trim()) {
      setLocalError("Enter an installed Ollama model.");
      return;
    }
    setPending("ollama");
    setLocalError(null);
    try {
      await onConfigureOllama({ endpoint: ollamaEndpoint.trim(), model: ollamaModel.trim() });
    } catch {
      setLocalError("Aone could not verify this local model. Check that Ollama is running and try again.");
    } finally {
      setPending(null);
    }
  };

  const handleCliAction = async (
    action: "refresh-cli" | "connect-cli",
    callback: (() => Promise<void>) | undefined,
  ) => {
    if (!callback) {
      setLocalError("This CLI action is not connected to the native shell.");
      return;
    }
    setPending(action);
    setLocalError(null);
    try {
      await callback();
    } catch {
      setLocalError(action === "refresh-cli"
        ? "Aone could not refresh CLI status."
        : "Aone could not connect that CLI. Confirm authentication and try again.");
    } finally {
      setPending(null);
    }
  };

  return (
    <div className="assistant-setup-layer">
      <section
        className="assistant-setup-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="assistant-setup-title"
        aria-describedby="assistant-setup-description"
      >
        <header className="assistant-setup-heading">
          <div className="assistant-setup-mark" aria-hidden="true"><Robot size={20} weight="duotone" /></div>
          <div>
            <h1 id="assistant-setup-title">Connect an AI agent</h1>
            <p id="assistant-setup-description">Choose a coding-agent CLI or model provider before Aone plans from workspace evidence.</p>
          </div>
          <ProviderState
            label={hostedConfigured ? "Configured" : ready ? "Ready" : "Required"}
            tone={ready ? "ready" : "warning"}
          />
        </header>

        <div className="assistant-current-status" role="status" aria-live="polite">
          {status === null && !error
            ? <><SpinnerGap className="spin" size={13} /> Checking native configuration</>
            : ready
              ? <><CheckCircle size={13} weight="fill" /> {configurationLabel(status)}</>
              : <><Warning size={13} /> {configurationLabel(status)}</>}
        </div>

        <div className="assistant-agent-contract">
          <ShieldCheck size={15} weight="duotone" aria-hidden="true" />
          <p>After a folder opens, Project Agent combines readiness, an approval plan, and engineering chat. Model text never starts project commands, services, or Docker.</p>
        </div>

        <div className="assistant-setup-tabs" role="tablist" aria-label="AI configuration methods">
          {SETUP_TABS.map((tab, index) => (
            <button
              key={tab.id}
              ref={(element) => { tabRefs.current[index] = element; }}
              id={`assistant-tab-${tab.id}`}
              type="button"
              role="tab"
              aria-selected={activeTab === tab.id}
              aria-controls={`assistant-panel-${tab.id}`}
              tabIndex={activeTab === tab.id ? 0 : -1}
              onClick={() => selectTab(tab.id)}
              onKeyDown={(event) => handleTabKeyDown(event, index)}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {activeTab === "hosted" && (
          <section className="assistant-setup-panel" id="assistant-panel-hosted" role="tabpanel" aria-labelledby="assistant-tab-hosted">
            <div className="assistant-method-heading">
              <Cloud size={18} weight="duotone" />
              <div><h2>Hosted agent</h2><p>Use a direct key or import a private environment file.</p></div>
            </div>

            <form className="assistant-configuration-form" onSubmit={(event) => void handleHostedSubmit(event)}>
              <div className="assistant-field">
                <AssistantSelect
                  label="Provider"
                  value={provider}
                  options={HOSTED_PROVIDER_OPTIONS}
                  onValueChange={handleProviderChange}
                  disabled={busy}
                  describedBy="assistant-hosted-provider-help"
                  popupLabel="Select hosted AI provider"
                />
                <small id="assistant-hosted-provider-help">Select the provider that issued this key.</small>
              </div>

              <div className="assistant-field">
                <label htmlFor="assistant-hosted-key">API key</label>
                <div className="assistant-secret-input">
                  <input
                    id="assistant-hosted-key"
                    type={showApiKey ? "text" : "password"}
                    value={apiKey}
                    onChange={(event) => setApiKey(event.target.value)}
                    aria-describedby="assistant-hosted-key-help"
                    autoComplete="off"
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    maxLength={16384}
                    disabled={busy}
                    required
                  />
                  <button
                    type="button"
                    aria-label={showApiKey ? "Hide API key" : "Show API key"}
                    title={showApiKey ? "Hide API key" : "Show API key"}
                    onClick={() => setShowApiKey((current) => !current)}
                    disabled={busy}
                  >
                    {showApiKey ? <EyeSlash size={15} /> : <Eye size={15} />}
                  </button>
                </div>
                <small id="assistant-hosted-key-help">Masked by default. After submission, retained only in native memory for this app session.</small>
              </div>

              <div className="assistant-field">
                <AssistantSelect
                  label="Model"
                  value={hostedModel}
                  options={HOSTED_MODEL_OPTIONS[provider]}
                  onValueChange={setHostedModel}
                  disabled={busy}
                  describedBy="assistant-hosted-model-help"
                  popupLabel={`Select ${provider === "openai" ? "OpenAI" : "Anthropic"} model`}
                />
                <small id="assistant-hosted-model-help">Choose the model Aone will use for explanations and reviewed setup plans.</small>
              </div>

              {(localError || error) && <div className="provider-feedback is-error" role="alert"><Warning size={14} />{localError ?? error}</div>}

              <button className="primary-button assistant-connect-button" type="submit" disabled={busy || !onConfigureHosted}>
                {pending === "hosted" ? <SpinnerGap className="spin" size={14} /> : <PlugsConnected size={14} weight="bold" />}
                {pending === "hosted" ? "Configuring" : "Use API key"}
              </button>
            </form>

            <div className="assistant-secondary-action">
              <div><strong>Private environment file</strong><span>Optional import for provider key and model variables.</span></div>
              <button type="button" onClick={() => void handleEnvironmentImport()} disabled={busy || !importEnvironment}>
                {pending === "environment" ? <SpinnerGap className="spin" size={13} /> : <FileLock size={13} />}
                Choose file
              </button>
            </div>
          </section>
        )}

        {activeTab === "local" && (
          <section className="assistant-setup-panel" id="assistant-panel-local" role="tabpanel" aria-labelledby="assistant-tab-local">
            <div className="assistant-method-heading">
              <HardDrives size={18} />
              <div><h2>Local Ollama</h2><p>Connect to a model served on this computer.</p></div>
            </div>

            <form className="assistant-configuration-form" onSubmit={(event) => void handleOllamaSubmit(event)}>
              <div className="assistant-field">
                <label htmlFor="assistant-ollama-endpoint">Endpoint</label>
                <input
                  id="assistant-ollama-endpoint"
                  type="url"
                  value={ollamaEndpoint}
                  onChange={(event) => setOllamaEndpoint(event.target.value)}
                  aria-describedby="assistant-ollama-endpoint-help"
                  autoComplete="off"
                  spellCheck={false}
                  disabled={busy}
                  required
                />
                <small id="assistant-ollama-endpoint-help">Loopback endpoints only. Default: http://127.0.0.1:11434</small>
              </div>
              <div className="assistant-field">
                <label htmlFor="assistant-ollama-model">Installed model</label>
                <input
                  id="assistant-ollama-model"
                  type="text"
                  value={ollamaModel}
                  onChange={(event) => setOllamaModel(event.target.value)}
                  aria-describedby="assistant-ollama-model-help"
                  placeholder="Example: qwen2.5-coder:7b"
                  autoComplete="off"
                  spellCheck={false}
                  maxLength={120}
                  disabled={busy}
                  required
                />
                <small id="assistant-ollama-model-help">Aone verifies the model through the native Ollama adapter.</small>
              </div>
              {(localError || error) && <div className="provider-feedback is-error" role="alert"><Warning size={14} />{localError ?? error}</div>}
              <button className="primary-button assistant-connect-button" type="submit" disabled={busy || !onConfigureOllama}>
                {pending === "ollama" ? <SpinnerGap className="spin" size={14} /> : <PlugsConnected size={14} weight="bold" />}
                {pending === "ollama" ? "Verifying" : "Connect Ollama"}
              </button>
            </form>
          </section>
        )}

        {activeTab === "cli" && (
          <section className="assistant-setup-panel" id="assistant-panel-cli" role="tabpanel" aria-labelledby="assistant-tab-cli">
            <div className="assistant-method-heading assistant-cli-heading">
              <CodeBlock size={18} />
              <div><h2>Coding-agent CLIs</h2><p>Detection alone does not count as a working agent connection.</p></div>
              <button
                type="button"
                className="assistant-refresh-button"
                onClick={() => void handleCliAction("refresh-cli", onRefreshCliAdapters)}
                disabled={busy || !onRefreshCliAdapters}
              >
                <ArrowClockwise className={pending === "refresh-cli" ? "spin" : ""} size={13} />
                Refresh
              </button>
            </div>

            <div className="assistant-cli-list">
              {cliAdapters.map((adapter) => {
                const supported = isSupportedCliAdapter(adapter.id);
                const unavailable = !supported;
                const state = unavailable
                  ? { label: "Unavailable", tone: "muted" as const }
                  : cliState(adapter);
                const canConnect = supported && adapter.readiness === "ready";
                return (
                  <article key={adapter.id} className={`assistant-cli-row${unavailable ? " is-unavailable" : ""}`}>
                    <CodeBlock size={16} aria-hidden="true" />
                    <div>
                      <strong>{adapter.label}</strong>
                      <span>{unavailable ? "The Aone adapter is not available yet." : adapter.detail}</span>
                      {!unavailable && adapter.version && <small>Version {adapter.version}</small>}
                    </div>
                    <ProviderState label={state.label} tone={state.tone} />
                    {canConnect && (
                      <button
                        type="button"
                        className="assistant-cli-connect"
                        onClick={() => void handleCliAction(
                          "connect-cli",
                          onConnectCli && (() => onConnectCli(adapter.id as SupportedCliAdapter)),
                        )}
                        disabled={busy || !onConnectCli}
                      >
                        {pending === "connect-cli" ? <SpinnerGap className="spin" size={13} /> : <PlugsConnected size={13} />}
                        Connect
                      </button>
                    )}
                  </article>
                );
              })}
            </div>
            {(localError || error) && <div className="provider-feedback is-error" role="alert"><Warning size={14} />{localError ?? error}</div>}
            <p className="assistant-cli-note">Install and authenticate a supported CLI, then select Refresh. Aone never asks you to paste a key into a shell command. Unsupported agents remain visible instead of being simulated.</p>
          </section>
        )}

        <footer className="assistant-setup-footer">
          <span>
            Direct keys stay only in native memory for this app session. Hosted authentication
            is checked on the first consented request; every provider request still requires
            native consent.
          </span>
          {onContinueCodeOnly && (
            <button type="button" onClick={onContinueCodeOnly} disabled={busy}>
              Continue code-only
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
