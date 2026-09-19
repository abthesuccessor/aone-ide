import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  askProjectAgent,
  explainProjectEnvironment,
  getProjectEnvironmentReport,
  inspectProjectEnvironment,
} from "./projectEnvironmentBridge";

const tauri = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => tauri);

beforeEach(() => {
  vi.clearAllMocks();
  delete window.__TAURI_INTERNALS__;
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
});

describe("project environment browser-demo bridge", () => {
  it("returns deterministic typed setup evidence without touching the host", async () => {
    const report = await inspectProjectEnvironment("workspace:test");

    expect(report).toMatchObject({
      workspaceId: "workspace:test",
      versionProbeApproved: true,
      stacks: expect.arrayContaining([
        expect.objectContaining({ label: "TypeScript + Node.js", confidence: "confirmed" }),
      ]),
      tools: expect.arrayContaining([
        expect.objectContaining({ id: "node", canonicalPath: "/opt/homebrew/bin/node" }),
      ]),
    });
  });

  it("keeps the optional AI explanation separate from deterministic inspection", async () => {
    const explanation = await explainProjectEnvironment("workspace:test", "report:test");

    expect(explanation.model).toBe("browser-demo");
    expect(explanation.answer).toContain("TypeScript and Node.js");
    expect(explanation.evidence.every((item) => item.evidence !== "observed")).toBe(true);
  });

  it("reuses a browser-session inspection without another host operation", async () => {
    expect(await getProjectEnvironmentReport("workspace:cached")).toBeNull();
    const inspected = await inspectProjectEnvironment("workspace:cached");

    await expect(getProjectEnvironmentReport("workspace:cached")).resolves.toBe(inspected);
  });

  it("provides bounded browser-demo project guidance", async () => {
    const response = await askProjectAgent({
      workspaceId: "workspace:test",
      reportId: "report:test",
      question: "How should I configure .env?",
    });

    expect(response).toMatchObject({ status: "completed", model: "browser-demo" });
    if (response.status === "completed") {
      expect(response.answer).toContain("native memory");
      expect(response.evidence).toEqual(expect.arrayContaining([
        expect.objectContaining({ id: "report:test", evidence: "resolved" }),
      ]));
    }
  });

  it("uses the frozen native command and bounded identifier-only request shapes", async () => {
    window.__TAURI_INTERNALS__ = {};
    tauri.invoke
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({ answer: "Bounded", evidence: [], model: "gpt-test" })
      .mockResolvedValueOnce({ status: "completed", answer: "Ready", evidence: [], model: "gpt-test" });

    await expect(getProjectEnvironmentReport("workspace:native")).resolves.toBeNull();
    await expect(inspectProjectEnvironment("workspace:native")).resolves.toBeNull();
    await expect(explainProjectEnvironment("workspace:native", "report:native"))
      .resolves.toMatchObject({ answer: "Bounded" });
    await expect(askProjectAgent({ workspaceId: "workspace:native", reportId: "report:native", question: "What should run?" }))
      .resolves.toMatchObject({ status: "completed", answer: "Ready" });
    expect(tauri.invoke).toHaveBeenNthCalledWith(1, "get_project_environment_report", {
      request: { workspaceId: "workspace:native" },
    });
    expect(tauri.invoke).toHaveBeenNthCalledWith(2, "inspect_project_environment", {
      request: { workspaceId: "workspace:native" },
    });
    expect(tauri.invoke).toHaveBeenNthCalledWith(3, "ai_explain_project_environment", {
      request: { workspaceId: "workspace:native", reportId: "report:native" },
    });
    expect(tauri.invoke).toHaveBeenNthCalledWith(4, "ai_ask_project_agent", {
      request: { workspaceId: "workspace:native", reportId: "report:native", question: "What should run?" },
    });
  });
});
