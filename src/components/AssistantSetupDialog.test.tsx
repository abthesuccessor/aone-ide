import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AiCliAdapterStatus, AiConfigurationStatus } from "../types";
import { AssistantSetupDialog } from "./AssistantSetupDialog";

const unconfiguredStatus: AiConfigurationStatus = {
  configured: false,
  provider: null,
  model: "",
  transport: null,
  inferenceAvailable: false,
};

function renderDialog(overrides: Partial<React.ComponentProps<typeof AssistantSetupDialog>> = {}) {
  const props: React.ComponentProps<typeof AssistantSetupDialog> = {
    status: unconfiguredStatus,
    loading: false,
    error: null,
    onConfigureHosted: vi.fn(async () => undefined),
    onImportEnvironment: vi.fn(async () => undefined),
    onConfigureOllama: vi.fn(async () => undefined),
    onRefreshCliAdapters: vi.fn(async () => undefined),
    onConnectCli: vi.fn(async () => undefined),
    ...overrides,
  };
  return { ...render(<AssistantSetupDialog {...props} />), props };
}

function chooseOption(listbox: HTMLElement, name: string) {
  const option = within(listbox).getByRole("option", { name });
  fireEvent.pointerDown(option, { pointerType: "mouse" });
  fireEvent.click(option);
}

describe("AssistantSetupDialog", () => {
  it("allows an explicit local code-only session without configuring a provider", () => {
    const onContinueCodeOnly = vi.fn();
    renderDialog({ onContinueCodeOnly });

    fireEvent.click(screen.getByRole("button", { name: "Continue code-only" }));

    expect(onContinueCodeOnly).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/every provider request still requires native consent/i)).toBeInTheDocument();
    expect(screen.getByText(/Model text never starts project commands, services, or Docker/i)).toBeInTheDocument();
  });

  it("submits a masked hosted key, clears it, and keeps environment import secondary", async () => {
    const onConfigureHosted = vi.fn(async () => undefined);
    const onImportEnvironment = vi.fn(async () => undefined);
    renderDialog({ onConfigureHosted, onImportEnvironment });

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Hosted", "Local", "CLI"]);
    const keyInput = screen.getByLabelText("API key");
    expect(keyInput).toHaveAttribute("type", "password");
    expect(keyInput).toHaveAttribute("maxlength", "16384");
    expect(screen.getByLabelText("Model")).toHaveTextContent("GPT-5.6 Luna");

    fireEvent.click(screen.getByRole("button", { name: "Show API key" }));
    expect(keyInput).toHaveAttribute("type", "text");
    expect(screen.getByRole("button", { name: "Hide API key" })).toBeInTheDocument();

    const providerControl = screen.getByLabelText("Provider");
    expect(providerControl.tagName).toBe("BUTTON");
    fireEvent.mouseDown(providerControl);
    const providerPopup = await screen.findByRole("listbox", { name: "Select hosted AI provider" });
    expect(within(providerPopup).queryByRole("combobox")).not.toBeInTheDocument();
    chooseOption(providerPopup, "Anthropic");
    expect(providerControl).toHaveTextContent("Anthropic");

    const modelControl = screen.getByLabelText("Model");
    expect(modelControl).toHaveTextContent("Claude Sonnet 5");
    fireEvent.mouseDown(modelControl);
    const modelPopup = await screen.findByRole("listbox", { name: "Select Anthropic model" });
    chooseOption(modelPopup, "Claude Opus 5");

    fireEvent.change(keyInput, { target: { value: "private-provider-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Use API key" }));

    await waitFor(() => expect(onConfigureHosted).toHaveBeenCalledWith({
      provider: "anthropic",
      apiKey: "private-provider-key",
      model: "claude-opus-5",
    }));
    await waitFor(() => expect(keyInput).toHaveValue(""));
    expect(screen.queryByText("private-provider-key")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Choose file" }));
    await waitFor(() => expect(onImportEnvironment).toHaveBeenCalledTimes(1));
  });

  it("does not intercept the native paste shortcut in the API key field", () => {
    renderDialog();
    const keyInput = screen.getByLabelText("API key");

    expect(fireEvent.keyDown(keyInput, { key: "v", metaKey: true })).toBe(true);
    fireEvent.input(keyInput, {
      inputType: "insertFromPaste",
      data: "pasted-provider-key",
      target: { value: "pasted-provider-key" },
    });

    expect(keyInput).toHaveValue("pasted-provider-key");
    expect(keyInput).toHaveAttribute("type", "password");
  });

  it("renders fixed provider-specific model menus without filter fields", async () => {
    renderDialog();

    const providerControl = screen.getByLabelText("Provider");
    fireEvent.mouseDown(providerControl);
    const providerPopup = await screen.findByRole("listbox", { name: "Select hosted AI provider" });
    expect(within(providerPopup).queryByRole("combobox")).not.toBeInTheDocument();
    expect(within(providerPopup).getAllByRole("option")).toHaveLength(2);
    chooseOption(providerPopup, "Anthropic");

    await waitFor(() => expect(providerControl).toHaveTextContent("Anthropic"));
    const modelControl = screen.getByLabelText("Model");
    fireEvent.mouseDown(modelControl);
    const modelPopup = await screen.findByRole("listbox", { name: "Select Anthropic model" });
    expect(within(modelPopup).getByRole("option", { name: "Claude Sonnet 5" })).toBeInTheDocument();
    expect(within(modelPopup).getByRole("option", { name: "Claude Opus 5" })).toBeInTheDocument();
    expect(within(modelPopup).queryByRole("option", { name: "GPT-5.6 Luna" })).not.toBeInTheDocument();
  });

  it("labels hosted setup as configured without claiming remote readiness", () => {
    renderDialog({
      status: {
        configured: true,
        provider: "openai",
        model: "gpt-model",
        transport: "api",
        inferenceAvailable: true,
      },
    });

    expect(screen.getByText("Configured")).toBeInTheDocument();
    expect(screen.getByText("OpenAI configured with gpt-model")).toBeInTheDocument();
    expect(screen.queryByText(/OpenAI ready/i)).not.toBeInTheDocument();
    expect(screen.getByText(/authentication is checked on the first consented request/i)).toBeInTheDocument();
  });

  it("clears a rejected hosted key and renders a safe inline error", async () => {
    const onConfigureHosted = vi.fn(async () => {
      throw new Error("unsafe provider detail");
    });
    renderDialog({ onConfigureHosted });

    const keyInput = screen.getByLabelText("API key");
    fireEvent.change(keyInput, { target: { value: "secret-on-error" } });
    fireEvent.click(screen.getByRole("button", { name: "Use API key" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Aone could not configure this hosted provider");
    expect(screen.getByRole("alert")).not.toHaveTextContent("unsafe provider detail");
    expect(keyInput).toHaveValue("");
  });

  it("configures a loopback Ollama endpoint and model", async () => {
    const onConfigureOllama = vi.fn(async () => undefined);
    renderDialog({ onConfigureOllama });

    fireEvent.click(screen.getByRole("tab", { name: "Local" }));
    expect(screen.getByLabelText("Endpoint")).toHaveValue("http://127.0.0.1:11434");
    fireEvent.change(screen.getByLabelText("Installed model"), { target: { value: "qwen2.5-coder:7b" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect Ollama" }));

    await waitFor(() => expect(onConfigureOllama).toHaveBeenCalledWith({
      endpoint: "http://127.0.0.1:11434",
      model: "qwen2.5-coder:7b",
    }));
  });

  it("connects ready Codex and Claude adapters and keeps Copilot unavailable", async () => {
    const adapters: AiCliAdapterStatus[] = [
      {
        id: "codex",
        label: "Codex CLI",
        installed: true,
        readiness: "ready",
        version: "1.2.3",
        detail: "Authenticated and ready.",
      },
      {
        id: "claude",
        label: "Claude Code",
        installed: true,
        readiness: "ready",
        detail: "Backend claimed ready.",
      },
      {
        id: "copilot",
        label: "GitHub Copilot",
        installed: true,
        readiness: "authentication_required",
        detail: "Authentication is missing.",
      },
    ];
    const onRefreshCliAdapters = vi.fn(async () => undefined);
    const onConnectCli = vi.fn(async () => undefined);
    renderDialog({ cliAdapters: adapters, onRefreshCliAdapters, onConnectCli });

    fireEvent.click(screen.getByRole("tab", { name: "CLI" }));
    const codexRow = screen.getByText("Codex CLI").closest("article");
    const claudeRow = screen.getByText("Claude Code").closest("article");
    const copilotRow = screen.getByText("GitHub Copilot").closest("article");
    expect(codexRow).not.toBeNull();
    expect(claudeRow).not.toBeNull();
    expect(copilotRow).not.toBeNull();
    expect(within(codexRow!).getByText("Ready")).toBeInTheDocument();
    expect(within(claudeRow!).getByText("Ready")).toBeInTheDocument();
    // Copilot has no native adapter, so a backend "ready" must still not offer
    // a Connect action.
    expect(within(copilotRow!).getByText("Unavailable")).toBeInTheDocument();
    const connectButtons = screen.getAllByRole("button", { name: "Connect" });
    expect(connectButtons).toHaveLength(2);
    expect(within(copilotRow!).queryByRole("button", { name: "Connect" })).toBeNull();

    // Each row connects its own adapter, not a hard-coded one.
    fireEvent.click(within(codexRow!).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(onConnectCli).toHaveBeenCalledWith("codex"));
    fireEvent.click(within(claudeRow!).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(onConnectCli).toHaveBeenCalledWith("claude"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(onRefreshCliAdapters).toHaveBeenCalledTimes(1));
  });

  it("supports arrow-key movement across configuration tabs", () => {
    renderDialog();
    const hosted = screen.getByRole("tab", { name: "Hosted" });
    hosted.focus();
    fireEvent.keyDown(hosted, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Local" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveAccessibleName("Local");
  });

  it("distinguishes a retryable Codex probe error from unsupported adapters", () => {
    renderDialog({
      cliAdapters: [{
        id: "codex",
        label: "Codex CLI",
        installed: true,
        readiness: "error",
        detail: "The bounded readiness check failed.",
      }],
    });

    fireEvent.click(screen.getByRole("tab", { name: "CLI" }));
    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Connect" })).not.toBeInTheDocument();
  });
});
