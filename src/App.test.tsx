import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@monaco-editor/react", () => ({
  default: ({ path, value }: { path?: string; value?: string }) => (
    <div data-testid="monaco-editor" data-path={path}>
      <pre>{value}</pre>
    </div>
  ),
}));

vi.mock("./lib/monaco", () => ({}));

afterEach(() => {
  vi.restoreAllMocks();
});

async function renderReadyApp() {
  render(<App />);
  await screen.findAllByText("checkout-platform");
  const setup = screen.queryByRole("button", { name: "Use API key" });
  if (setup) {
    await waitFor(() => expect(setup).toBeEnabled());
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "browser-demo-key" } });
    fireEvent.click(setup);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Connect an AI agent" })).not.toBeInTheDocument());
  }
  await screen.findByTestId("graph-canvas", {}, { timeout: 10_000 });
}

describe("Aone browser demo", () => {
  it("requires a verified provider before exposing the workbench", async () => {
    render(<App />);
    const dialog = await screen.findByRole("dialog", { name: "Connect an AI agent" });
    expect(dialog).toBeVisible();
    expect(document.querySelector(".ide-grid")).toHaveAttribute("inert");
    expect(within(dialog).getByRole("tab", { name: "Hosted" })).toHaveAttribute("aria-selected", "true");
    expect(within(dialog).getByRole("tab", { name: "Local" })).toBeVisible();
    expect(within(dialog).getByRole("tab", { name: "CLI" })).toBeVisible();
    const key = within(dialog).getByLabelText("API key");
    expect(key).toHaveAttribute("type", "password");
    expect(within(dialog).getByRole("button", { name: "Choose file" })).toBeVisible();
    const connect = within(dialog).getByRole("button", { name: "Use API key" });
    await waitFor(() => expect(connect).toBeEnabled());
    fireEvent.change(key, { target: { value: "browser-demo-key" } });
    fireEvent.click(connect);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Connect an AI agent" })).not.toBeInTheDocument());
    expect(await screen.findByTestId("graph-canvas")).toBeVisible();
  });

  it("loads the dense IDE shell and keeps evidence classes explicit", async () => {
    await renderReadyApp();

    expect(document.querySelector(".titlebar .brand-mark")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Scan" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Run" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Debug" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Observe" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    expect(screen.getByRole("group", { name: /Workspace relationships with/ })).toBeVisible();
    expect(screen.getAllByRole("button", { name: /observed evidence/ }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /Missing retry boundary, risk, inferred evidence/ })).toBeVisible();
    expect(screen.getByText("Browser demo")).toBeVisible();
  });

  it("keeps graph and debug as independent activity views, not file tabs", async () => {
    await renderReadyApp();
    expect(screen.queryByRole("button", { name: "Getting Started" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Graph" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Debug" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cooperative Debugger" }));

    expect(screen.getByRole("button", { name: "Cooperative Debugger" })).toHaveAttribute("aria-pressed", "true");
    expect((await screen.findAllByRole("region", { name: "API trace workbench" })).length).toBeGreaterThan(0);
    expect(await screen.findByRole("region", { name: "API client" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Trace source editor" })).toBeVisible();
    expect(screen.getByRole("region", { name: "API execution path" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Trace data and values" })).toBeVisible();
    expect(document.querySelector(".ide-navigator-panel")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Runtime and API console" })).not.toBeInTheDocument();
  });

  it("switches immediately from Source Control to the requested activity page", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "Source Control" }));
    const sourceControl = await screen.findByRole("complementary", { name: "Source control" });
    expect(sourceControl).toBeVisible();
    expect(within(sourceControl).getByRole("heading", { name: "Source Control" })).toBeVisible();
    expect(within(sourceControl).getByText("Changes")).toBeVisible();
    expect(await screen.findByRole("button", { name: "Open working tree diff for src/api/services/CheckoutService.ts" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Source Control" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Code Graph" }));
    const graphSidebar = await screen.findByRole("complementary", { name: "Indexed code knowledge" });
    expect(graphSidebar).toBeVisible();
    expect(within(graphSidebar).getByText("Code knowledge")).toBeVisible();
    expect(within(graphSidebar).getByRole("tab", { name: "API call paths" })).toHaveAttribute("aria-selected", "true");
    expect(within(graphSidebar).getByRole("tab", { name: "Indexed files" })).toBeVisible();
    expect(screen.queryByRole("complementary", { name: "Source control" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Code Graph" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Explorer" }));
    expect(screen.getByRole("button", { name: "Explorer" })).toHaveAttribute("aria-pressed", "true");
  });

  it("uses the titlebar command center and layout controls as real workbench actions", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "Source Control" }));
    expect(await screen.findByRole("complementary", { name: "Source control" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Quick open files in checkout-platform" }));
    const quickFile = await screen.findByPlaceholderText("Filter files · ⌘K");
    await waitFor(() => expect(quickFile).toHaveFocus());
    expect(screen.getByRole("button", { name: "Explorer" })).toHaveAttribute("aria-pressed", "true");

    const primary = screen.getByRole("button", { name: "Toggle primary sidebar" });
    fireEvent.click(primary);
    expect(primary).toHaveAttribute("aria-pressed", "false");
    expect(screen.queryByRole("complementary", { name: "Workspace explorer" })).not.toBeInTheDocument();
    fireEvent.click(primary);
    expect(await screen.findByRole("complementary", { name: "Workspace explorer" })).toBeVisible();

    const secondary = screen.getByRole("button", { name: "Toggle secondary sidebar" });
    fireEvent.click(secondary);
    expect(secondary).toHaveAttribute("aria-pressed", "false");
    expect(screen.queryByRole("complementary", { name: "Selected node evidence and relationships" })).not.toBeInTheDocument();
    fireEvent.click(secondary);
    expect(await screen.findByRole("complementary", { name: "Selected node evidence and relationships" })).toBeVisible();
  });

  it("replaces a lazy Search sidebar when Relationships is selected", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "Search" }));
    const searchSidebar = await screen.findByRole("complementary", { name: "Search" });
    expect(searchSidebar).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Search" })).toBeVisible();
    expect(await screen.findByText("Results")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Code Graph" }));
    expect(await screen.findByRole("complementary", { name: "Indexed code knowledge" })).toBeVisible();
    expect(screen.queryByRole("complementary", { name: "Search" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Code Graph" })).toHaveAttribute("aria-pressed", "true");
  });

  it("traces a navigator endpoint and opens its exact handler source", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "Code Graph" }));
    const navigator = await screen.findByRole("complementary", { name: "Indexed code knowledge" });
    const checkout = await within(navigator).findByRole("button", {
      name: /POST\s*\/api\/checkout\s*checkoutRouter\.post/i,
    });
    fireEvent.click(checkout);

    expect(screen.getByRole("button", { name: "Cooperative Debugger" })).toHaveAttribute("aria-pressed", "true");
    expect((await screen.findAllByRole("region", { name: "API trace workbench" })).length).toBeGreaterThan(0);
    expect(screen.queryByRole("complementary", { name: "Indexed code knowledge" })).not.toBeInTheDocument();
    const editor = await screen.findByTestId("monaco-editor");
    await waitFor(() => {
      expect(editor).toHaveAttribute("data-path", "src/api/routes/checkout.ts");
      expect(editor).toHaveTextContent('checkoutRouter.post("/api/checkout"');
    });
    expect(await screen.findByText("Loaded execution flow for api:post:/api/checkout")).toBeVisible();
  });

  it("keeps Explorer, editable source, graph, API flow, data visualization, and terminal in one resizable workbench", async () => {
    await renderReadyApp();

    expect(screen.getByRole("complementary", { name: "Workspace explorer" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Indexed source and workspace relationships" })).toBeVisible();
    expect(screen.getByRole("search")).toContainElement(screen.getByLabelText("Search graph relationships"));
    expect(screen.getByRole("button", { name: "Export current graph view" })).toBeVisible();
    expect(screen.getByRole("complementary", { name: "Selected node evidence and relationships" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Runtime and API console" })).toBeVisible();
    expect(screen.getByRole("separator", { name: "Resize Explorer" })).toBeVisible();
    expect(screen.getByRole("separator", { name: "Resize source and graph panels" })).toBeVisible();
    expect(screen.getByRole("separator", { name: "Resize evidence inspector" })).toBeVisible();
    expect(screen.getByRole("separator", { name: "Resize terminal and runtime console" })).toBeVisible();
    expect(screen.getByRole("tab", { name: "Terminal" })).toBeVisible();

    const checkout = await screen.findByRole("button", { name: /POST \/api\/checkout, endpoint, declared evidence/ });
    fireEvent.click(checkout);
    expect(await screen.findByTestId("monaco-editor")).toHaveAttribute("data-path", "src/api/routes/checkout.ts");
  });

  it("shows indexing as a thin progress bar and reports completion in a side toast", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: "Scan" }));
    const progress = await screen.findByRole("progressbar", { name: /Indexing/i });
    expect(progress).toHaveTextContent("");
    expect(screen.getByRole("button", { name: "Scan" })).toBeDisabled();
    await waitFor(() => {
      expect(progress).toHaveAccessibleName(/committing graph and search index/i);
      expect(progress).toHaveClass("is-indeterminate");
      expect(progress).not.toHaveAttribute("aria-valuenow");
      expect(progress).toHaveAttribute(
        "aria-valuetext",
        "committing graph and search index",
      );
    });

    expect(
      await screen.findByText(
        "Indexing complete: checkout-platform (42 files)",
        {},
        { timeout: 3_000 },
      ),
    ).toBeVisible();
    await waitFor(() => expect(screen.queryByRole("progressbar", { name: /Indexing/i })).not.toBeInTheDocument());
  });

  it("filters the workspace and synchronizes a graph selection with Monaco source", async () => {
    await renderReadyApp();

    const filter = screen.getByPlaceholderText("Filter files · ⌘K");
    fireEvent.change(filter, { target: { value: "definitely-not-a-file" } });
    expect(screen.getByText(/No files match/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Clear file filter" }));

    const endpoint = await screen.findByRole("button", { name: /POST \/api\/checkout, endpoint, declared evidence/ });
    fireEvent.doubleClick(endpoint);

    const editor = await screen.findByTestId("monaco-editor");
    expect(editor).toHaveAttribute("data-path", "src/api/routes/checkout.ts");
    expect(editor).toHaveTextContent('checkoutRouter.post("/api/checkout"');
    expect(screen.getByRole("tab", { name: /checkout\.ts/ })).toBeVisible();
    expect(screen.queryByRole("tab", { name: "Graph" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Explorer" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Code Graph" })).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps AI provider and run key names in separate UI scopes", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: /Environment settings/ }));
    expect(screen.getByText("Secret values stay in native memory.")).toBeVisible();
    expect(screen.getByText(/Run values go only to the selected process.*each AI request requires separate approval/)).toBeVisible();
    expect(screen.getByRole("region", { name: "AI provider" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Run profile" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Choose AI .env file" }));
    expect(await screen.findByText("OPENAI_API_KEY")).toBeVisible();
    expect(screen.queryByText("DATABASE_URL")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Choose run .env file" }));
    expect(await screen.findByText("DATABASE_URL")).toBeVisible();
    expect(screen.getByText("STRIPE_SECRET_KEY")).toBeVisible();
    expect(screen.getAllByText("name only")).toHaveLength(6);
    expect(document.body).not.toHaveTextContent("sk-");
    expect(document.body).not.toHaveTextContent("postgres://");
  });

  it("keeps app-session AI configuration visible across workspace rescans", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: /Environment settings/ }));
    fireEvent.click(screen.getByRole("button", { name: "Choose AI .env file" }));
    expect(await screen.findByText("OPENAI_API_KEY")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Close environment panel" }));

    fireEvent.click(screen.getByRole("button", { name: "Scan" }));
    await waitFor(() => expect(screen.queryByRole("progressbar", { name: /Indexing/i })).not.toBeInTheDocument(), { timeout: 3_000 });
    await waitFor(() => expect(screen.getByRole("button", { name: /Environment settings \(2 AI, 0 run\)/ })).toBeVisible());
  });

  it("combines project readiness, approval, and AI guidance in one Project Agent", async () => {
    await renderReadyApp();

    expect(screen.queryByRole("button", { name: "Project Setup" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Project Agent" }));
    expect(await screen.findByText("Inspect once, then keep working")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));

    expect(await screen.findByText("TypeScript + Node.js")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Review IDE change" }));
    fireEvent.click(screen.getByRole("button", { name: "Approve profile" }));
    expect(screen.getByLabelText("Run profile")).toHaveValue("web-dev");
    expect(await screen.findByText("Project Agent selected Web · dev; no process was started")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "What must run before I can test this service?" }));
    expect(await screen.findByText(/Start with the registered Web · dev profile/)).toBeVisible();
    expect(screen.getByText("Inferred · browser-demo")).toBeVisible();
  });

  it("authors REST and GraphQL requests and shows a bounded response timeline", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("tab", { name: "API Client" }));
    expect(screen.getByLabelText("Request URL")).toHaveValue("http://127.0.0.1:3000/");
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    expect(await screen.findByText("201 Created")).toBeVisible();
    expect(screen.getByText(/"orderId": "ord_0189"/)).toBeVisible();
    expect(screen.getByLabelText("Request timeline")).toHaveTextContent("Total");
    expect(screen.getByLabelText("Request timeline")).toHaveTextContent("184 ms");
    expect(screen.getByLabelText("Response headers")).toHaveTextContent("content-type");
    expect(screen.getByLabelText("Response headers")).toHaveTextContent("application/json; charset=utf-8");

    fireEvent.click(screen.getByRole("tab", { name: "GraphQL" }));
    expect((screen.getByLabelText("GraphQL query") as HTMLTextAreaElement).value).toContain("__typename");
    expect(screen.getByLabelText("GraphQL variables")).toHaveValue("{}");
    expect(screen.getByLabelText("HTTP method")).toBeDisabled();
  });

  it("labels AI output as inferred and renders its evidence references", async () => {
    await renderReadyApp();

    fireEvent.click(screen.getByRole("button", { name: /POST \/api\/checkout, endpoint, declared evidence/ }));
    fireEvent.click(screen.getByRole("tab", { name: "AI Recommendation" }));
    expect(screen.getByText("AI output is inferred")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Explain selection" }));

    expect(await screen.findByText(/checkout request crosses the declared route/i)).toBeVisible();
    expect(screen.getByText("browser-demo")).toBeVisible();
    expect(screen.getByTitle("runtimeEvent / observed")).toBeDisabled();
  });

  it("does not auto-observe and keeps cooperative Debug native-only", async () => {
    await renderReadyApp();

    expect(screen.queryByRole("button", { name: "Observe" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Debug" }));
    expect(await screen.findByText(/Instrumented Debug requires the native desktop app/i)).toBeVisible();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });

  it("starts the selected profile with the advertised Command-R shortcut", async () => {
    await renderReadyApp();

    fireEvent.change(screen.getByLabelText("Run profile"), { target: { value: "web-dev" } });
    fireEvent.keyDown(window, { key: "r", metaKey: true });

    await waitFor(() => expect(screen.getByRole("button", { name: "Stop" })).toBeEnabled());
    expect(await screen.findByText("Running Web · dev")).toBeVisible();
  });
});
