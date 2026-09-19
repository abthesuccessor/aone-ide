import { PlugsConnected, Robot, TerminalWindow } from "@phosphor-icons/react";

interface ToolsSetupProps {
  onOpenTools: () => void;
  onOpenProjectAgent: () => void;
}

const tools = [
  "Codex CLI · agent bridge",
  "Claude Code · configuration only",
  "Copilot CLI · configuration only",
  "MCP servers · configuration",
];

export function ToolsSetup({ onOpenTools, onOpenProjectAgent }: ToolsSetupProps) {
  return (
    <section className="onboarding-section tools-setup" aria-labelledby="onboarding-tools-title">
      <header className="onboarding-section-heading">
        <span className="onboarding-kicker">Existing AI tools</span>
        <h2 id="onboarding-tools-title">Open configuration only when you choose</h2>
        <p>Aone can guide you to known CLI and MCP configuration locations. It does not inspect files, PATH, or installed apps on this screen.</p>
      </header>
      <div className="onboarding-tool-list" aria-label="Supported AI tool setup">
        {tools.map((tool) => <span key={tool}><TerminalWindow size={14} />{tool}</span>)}
      </div>
      <div className="onboarding-route-actions">
        <button type="button" className="primary-button" onClick={onOpenTools}>
          <PlugsConnected size={14} /> Open MCP and CLI configuration
        </button>
        <button type="button" className="secondary-button" onClick={onOpenProjectAgent}>
          <Robot size={14} /> Open Project Agent
        </button>
      </div>
      <p className="onboarding-route-note">Each destination has its own permission button. Opening a destination does not start inspection.</p>
    </section>
  );
}
