import {
  CheckCircle,
  FileLock,
  Key,
  Sparkle,
  TerminalWindow,
  UploadSimple,
  X,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";
import type { EnvLoadResult } from "../types";

interface EnvPopoverProps {
  open: boolean;
  aiEnv: EnvLoadResult | null;
  runEnv: EnvLoadResult | null;
  aiLoading: boolean;
  runLoading: boolean;
  onPickAi: () => void;
  onPickRun: () => void;
  onClose: () => void;
}

interface EnvironmentSectionProps {
  title: string;
  description: string;
  icon: ReactNode;
  env: EnvLoadResult | null;
  loading: boolean;
  emptyText: string;
  chooseLabel: string;
  onPick: () => void;
}

function EnvironmentSection({
  title,
  description,
  icon,
  env,
  loading,
  emptyText,
  chooseLabel,
  onPick,
}: EnvironmentSectionProps) {
  return (
    <section className="env-scope" aria-label={title}>
      <header className="env-scope-header">
        <span className="env-scope-icon">{icon}</span>
        <div><strong>{title}</strong><span>{description}</span></div>
      </header>
      {env ? (
        <>
          <div className="env-file-line">
            <CheckCircle size={13} weight="fill" />
            <span>Loaded for this session</span>
            <small>{env.loadedCount} names</small>
          </div>
          <ul className="env-key-list">
            {env.names.map((name) => (
              <li key={name}><span>{name}</span><em>name only</em></li>
            ))}
          </ul>
        </>
      ) : <p className="env-empty">{emptyText}</p>}
      <button type="button" className="secondary-button env-pick-button" onClick={onPick} disabled={loading}>
        <UploadSimple size={13} />
        {loading ? "Reading key names…" : env ? `Choose another ${chooseLabel}` : `Choose ${chooseLabel}`}
      </button>
    </section>
  );
}

export function EnvPopover({
  open,
  aiEnv,
  runEnv,
  aiLoading,
  runLoading,
  onPickAi,
  onPickRun,
  onClose,
}: EnvPopoverProps) {
  if (!open) return null;
  return (
    <section className="env-popover" aria-label="Environment settings">
      <header>
        <div><Key size={14} weight="bold" /><strong>Environment keys</strong></div>
        <button type="button" onClick={onClose} aria-label="Close environment panel"><X size={13} /></button>
      </header>
      <div className="env-security-note">
        <FileLock size={18} weight="duotone" />
        <p><strong>Secret values stay in native memory.</strong><span>Only names are shown here. Run values go only to the selected process; each AI request requires separate approval.</span></p>
      </div>
      <EnvironmentSection
        title="AI provider"
        description="Optional evidence-grounded explanations"
        icon={<Sparkle size={14} weight="duotone" />}
        env={aiEnv}
        loading={aiLoading}
        emptyText="No AI provider file loaded. Supported names: AONE_AI_PROVIDER, OPENAI_API_KEY, OPENAI_MODEL, ANTHROPIC_API_KEY, and ANTHROPIC_MODEL."
        chooseLabel="AI .env file"
        onPick={onPickAi}
      />
      <EnvironmentSection
        title="Run profile"
        description="Passed only to the selected local process"
        icon={<TerminalWindow size={14} weight="duotone" />}
        env={runEnv}
        loading={runLoading}
        emptyText="No run environment file loaded. Required names are checked before start."
        chooseLabel="run .env file"
        onPick={onPickRun}
      />
    </section>
  );
}
