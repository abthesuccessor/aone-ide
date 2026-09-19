import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import tauriConfigSource from "../../src-tauri/tauri.conf.json?raw";
import chromeCss from "../styles/chrome.css?raw";
import vscodeWorkbenchCss from "../styles/vscode-workbench.css?raw";
import { Titlebar } from "./Titlebar";

const callbacks = {
  onProfileChange: vi.fn(),
  onScan: vi.fn(),
  onRun: vi.fn(),
  onDebug: vi.fn(),
  onStop: vi.fn(),
  onEnvToggle: vi.fn(),
  onCommandCenter: vi.fn(),
  onTogglePrimarySidebar: vi.fn(),
  onToggleSecondarySidebar: vi.fn(),
  primarySidebarVisible: true,
  secondarySidebarVisible: true,
};

const workspace = {
  id: "workspace-1",
  name: "TYSON",
  rootPath: "/tmp/TYSON",
  fileCount: 4_746,
  nodeCount: 0,
  edgeCount: 0,
  languages: [],
  lastScannedAt: "2026-08-17T00:00:00Z",
};

afterEach(() => {
  vi.clearAllMocks();
  delete window.__TAURI_INTERNALS__;
});

describe("Titlebar macOS chrome", () => {
  it("reserves native window-control space and removes empty-workspace identity text", () => {
    window.__TAURI_INTERNALS__ = {};
    const { container } = render(
      <Titlebar
        {...callbacks}
        workspace={null}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={null}
        runMode={null}
        aiEnvCount={0}
        runEnvCount={0}
      />,
    );

    expect(container.querySelector(".brand-mark")).not.toBeInTheDocument();
    expect(container.querySelector(".traffic-lights.is-native")).toHaveAttribute("data-tauri-drag-region");
    expect(screen.queryByText("Aone")).not.toBeInTheDocument();
    expect(screen.queryByText("No workspace")).not.toBeInTheDocument();
    expect(screen.getByText("Open a file or folder")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Run" })).not.toBeInTheDocument();
  });

  it("centers the configuration command and hides workspace identity while locked", () => {
    render(
      <Titlebar
        {...callbacks}
        workspace={workspace}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={null}
        runMode={null}
        aiEnvCount={0}
        runEnvCount={0}
        locked
      />,
    );

    expect(screen.getByText("Configure AI to continue").closest("header")).toHaveClass("is-locked");
    expect(screen.queryByText("Aone")).not.toBeInTheDocument();
    expect(screen.queryByText("TYSON")).not.toBeInTheDocument();
    expect(chromeCss).toMatch(
      /\.titlebar-command-center\s*{[^}]*left:\s*50%;[^}]*transform:\s*translateX\(-50%\);/s,
    );
  });

  it("keeps workspace controls without restoring the redundant identity block", () => {
    const { container } = render(
      <Titlebar
        {...callbacks}
        workspace={workspace}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={null}
        runMode={null}
        aiEnvCount={0}
        runEnvCount={0}
      />,
    );

    expect(screen.queryByText("Aone")).not.toBeInTheDocument();
    expect(container.querySelector(".brand-block")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quick open files in TYSON" })).toHaveTextContent("TYSON");
    expect(screen.getByRole("button", { name: "Scan" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Toggle primary sidebar" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Toggle secondary sidebar" })).toHaveAttribute("aria-pressed", "true");
  });

  it("keeps the scan label stable while exposing progress through its title", () => {
    render(
      <Titlebar
        {...callbacks}
        workspace={workspace}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={{ phase: "Complete", completed: 4_746, total: 4_746 }}
        runMode={null}
        aiEnvCount={0}
        runEnvCount={0}
      />,
    );

    expect(screen.getByRole("button", { name: "Scan" })).toHaveAttribute(
      "title",
      "Indexing Complete: 4746/4746",
    );
    expect(screen.queryByText(/Complete 4746\/4746/)).not.toBeInTheDocument();
  });

  it("names an indeterminate commit only through title help", () => {
    render(
      <Titlebar
        {...callbacks}
        workspace={workspace}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={{ phase: "committing", completed: 0, total: 0 }}
        runMode={null}
        aiEnvCount={0}
        runEnvCount={0}
      />,
    );

    expect(screen.getByRole("button", { name: "Scan" })).toHaveAttribute(
      "title",
      "Committing graph and search index",
    );
    expect(screen.queryByText(/Committing graph and search index/)).not.toBeInTheDocument();
  });

  it("keeps native chrome and progress feedback compact in CSS", () => {
    const tauriConfig = JSON.parse(tauriConfigSource) as {
      app: { windows: Array<{ trafficLightPosition?: { x: number; y: number } }> };
    };

    expect(vscodeWorkbenchCss).toMatch(/--vscode-titlebar-height:\s*35px;/);
    expect(vscodeWorkbenchCss).toMatch(/\.traffic-lights\.is-native\s*{[^}]*flex-basis:\s*68px;/s);
    expect(vscodeWorkbenchCss).toMatch(/\.titlebar\s*{[^}]*min-height:\s*var\(--vscode-titlebar-height\);/s);
    expect(tauriConfig.app.windows[0]?.trafficLightPosition).toEqual({ x: 14, y: 18 });
    expect(chromeCss).toMatch(/\.scan-progress\s*{[^}]*height:\s*2px;/s);
    expect(chromeCss).not.toMatch(/\.scan-progress\s+(?:span|small)\s*{/);
  });

  it("shows Debug separately and keeps Stop pending until process exit", () => {
    render(
      <Titlebar
        {...callbacks}
        workspace={workspace}
        runProfiles={[]}
        activeProfileId=""
        scanProgress={null}
        runMode="debug"
        stopping
        aiEnvCount={0}
        runEnvCount={0}
      />,
    );

    expect(screen.getByRole("button", { name: "Debug" })).toHaveClass("is-active");
    expect(screen.getByRole("button", { name: "Stopping" })).toBeDisabled();
  });
});
