import { CheckCircle, FileLock, Sparkle, UploadSimple } from "@phosphor-icons/react";
import { useState } from "react";
import type { EnvLoadResult } from "../../types";
import { AI_ENVIRONMENTS, type AiProvider } from "./model";

interface AiSetupProps {
  aiEnv: EnvLoadResult | null;
  loading: boolean;
  onPickEnv: () => Promise<void>;
}

export function AiSetup({ aiEnv, loading, onPickEnv }: AiSetupProps) {
  const [provider, setProvider] = useState<AiProvider>("openai");
  const environment = AI_ENVIRONMENTS[provider];
  return (
    <section className="onboarding-section ai-setup" aria-labelledby="onboarding-ai-title">
      <header className="onboarding-section-heading">
        <span className="onboarding-kicker">AI provider</span>
        <h2 id="onboarding-ai-title">Load keys without exposing values to React</h2>
        <p>Choose a private env file only when you want AI explanations. Aone does not store key values in browser storage.</p>
      </header>
      <div className="ai-provider-grid">
        <div className="ai-provider-choice" role="radiogroup" aria-label="AI provider template">
          {(Object.keys(AI_ENVIRONMENTS) as AiProvider[]).map((id) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={provider === id}
              className={provider === id ? "is-selected" : ""}
              onClick={() => setProvider(id)}
            >
              <Sparkle size={15} weight={provider === id ? "fill" : "regular"} />
              <span><strong>{AI_ENVIRONMENTS[id].label}</strong><small>{AI_ENVIRONMENTS[id].fileName}</small></span>
            </button>
          ))}
        </div>
        <div className="ai-env-template" aria-label={`${environment.label} environment template`}>
          <header><FileLock size={14} /><strong>{environment.fileName}</strong><span>Private file template</span></header>
          <pre>{environment.names.map((name) => `${name}=${name === "AONE_AI_PROVIDER" ? provider : ""}`).join("\n")}</pre>
          <p>Keep this file outside version control. The picker sends key names to React and keeps values in Rust for this app session.</p>
        </div>
      </div>
      <div className="ai-consent-note">
        <FileLock size={16} weight="duotone" />
        <div><strong>Loading is not sending.</strong><span>Provider network consent remains a separate action when you request an AI explanation.</span></div>
      </div>
      {aiEnv && (
        <div className="ai-loaded-names" role="status">
          <CheckCircle size={15} weight="fill" />
          <div><strong>Loaded for this session</strong><span>{aiEnv.names.join(", ")}</span></div>
        </div>
      )}
      <button type="button" className="primary-button" disabled={loading} onClick={() => void onPickEnv()}>
        <UploadSimple size={14} />{loading ? "Reading key names" : "Choose private AI env file"}
      </button>
    </section>
  );
}
