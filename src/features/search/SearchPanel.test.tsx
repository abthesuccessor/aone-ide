import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { searchWorkspace } from "../../lib/searchBridge";
import type { SearchMatch, SearchWorkspaceResult } from "./model";
import { SearchPanel } from "./SearchPanel";

vi.mock("../../lib/searchBridge", () => ({ searchWorkspace: vi.fn() }));

const searchWorkspaceMock = vi.mocked(searchWorkspace);

function match(
  key: string,
  relativePath: string,
  startLine: number,
  preview: string,
  matchText = "checkout",
  kind: SearchMatch["kind"] = "content",
): SearchMatch {
  return {
    key,
    relativePath,
    startLine,
    startColumn: 3,
    endLine: startLine,
    endColumn: 3 + matchText.length,
    preview,
    matchText,
    kind,
  };
}

function result(matches: SearchMatch[], overrides: Partial<SearchWorkspaceResult> = {}): SearchWorkspaceResult {
  return {
    query: "checkout",
    matches,
    totalMatches: matches.length,
    truncated: false,
    indexedFileCount: 42,
    ...overrides,
  };
}

function renderSearch(onOpenMatch = vi.fn()) {
  render(
    <SearchPanel
      workspaceId="workspace:test"
      workspaceName="checkout-platform"
      workspaceGeneration={4}
      onOpenMatch={onOpenMatch}
      onOpenFolder={vi.fn()}
    />,
  );
  return onOpenMatch;
}

async function submitQuery(query = "checkout") {
  fireEvent.change(screen.getByPlaceholderText("Search · ⇧⌘F"), { target: { value: query } });
  await act(async () => {
    vi.advanceTimersByTime(180);
    await Promise.resolve();
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  searchWorkspaceMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SearchPanel", () => {
  it("autofocuses and groups relevance-ordered results by file", async () => {
    searchWorkspaceMock.mockResolvedValue(result([
      match("first", "src/api/CheckoutService.ts", 8, "class CheckoutService", "Checkout", "symbol"),
      match("second", "src/api/CheckoutService.ts", 14, "return checkout(cart)", "checkout"),
      match("third", "src/web/CheckoutPage.tsx", 5, "await checkout()", "checkout", "endpoint"),
    ]));
    renderSearch();

    expect(screen.getByPlaceholderText("Search · ⇧⌘F")).toHaveFocus();
    await submitQuery();

    expect(searchWorkspaceMock).toHaveBeenCalledWith({
      workspaceId: "workspace:test",
      query: "checkout",
      limit: 500,
    });
    expect(screen.getAllByRole("treeitem")).toHaveLength(5);
    expect(screen.getByTitle("src/api/CheckoutService.ts")).toHaveTextContent("2");
    expect(screen.getByText("Checkout", { selector: "mark" })).toBeVisible();
    expect(screen.getByText("3 matches in 2 files")).toBeVisible();
  });

  it("opens a match through the shared exact source-location callback", async () => {
    const selected = match("exact", "src/routes/checkout.ts", 23, "router.post('/checkout')", "checkout", "endpoint");
    searchWorkspaceMock.mockResolvedValue(result([selected]));
    const onOpenMatch = renderSearch();
    await submitQuery();

    fireEvent.click(screen.getByTitle("src/routes/checkout.ts:23:3"));

    expect(onOpenMatch).toHaveBeenCalledWith(selected, "preview");
    expect(onOpenMatch.mock.calls[0]?.[0]).toMatchObject({
      relativePath: "src/routes/checkout.ts",
      startLine: 23,
      startColumn: 3,
      endLine: 23,
      endColumn: 11,
    });

    const matchRow = screen.getByTitle("src/routes/checkout.ts:23:3");
    fireEvent.doubleClick(matchRow);
    expect(onOpenMatch).toHaveBeenLastCalledWith(selected, "pinned");
    expect(matchRow).toHaveAttribute("aria-keyshortcuts", "Meta+Enter Control+Enter");
  });

  it("prefers an exact preview match and folds only ASCII case without shifting Unicode offsets", async () => {
    searchWorkspaceMock.mockResolvedValue(result([
      match("exact-case", "src/exact.ts", 3, "İ CHECKOUT then checkout", "checkout"),
      match("ascii-case", "src/fallback.ts", 7, "İstanbul CHECKOUT", "checkout"),
    ]));
    renderSearch();
    await submitQuery();

    const highlights = screen.getAllByText(/checkout/i, { selector: "mark" });
    expect(highlights.map((highlight) => highlight.textContent)).toEqual(["checkout", "CHECKOUT"]);
  });

  it("ignores an older response after a newer query completes", async () => {
    let resolveOlder!: (value: SearchWorkspaceResult) => void;
    let resolveNewer!: (value: SearchWorkspaceResult) => void;
    searchWorkspaceMock.mockImplementation(({ query }) => new Promise((resolve) => {
      if (query === "older") resolveOlder = resolve;
      else resolveNewer = resolve;
    }));
    renderSearch();
    await submitQuery("older");
    await submitQuery("newer");

    await act(async () => resolveNewer(result([
      match("new", "src/current.ts", 7, "const newer = true", "newer"),
    ], { query: "newer" })));
    expect(screen.getByTitle("src/current.ts")).toBeVisible();

    await act(async () => resolveOlder(result([
      match("old", "src/stale.ts", 2, "const older = true", "older"),
    ], { query: "older" })));
    expect(screen.queryByTitle("src/stale.ts")).not.toBeInTheDocument();
    expect(screen.getByTitle("src/current.ts")).toBeVisible();
  });

  it("exposes truncation and error states without retaining stale results", async () => {
    searchWorkspaceMock.mockResolvedValueOnce(result([
      match("bounded", "src/bounded.ts", 9, "checkout()"),
    ], { totalMatches: 905, truncated: true }));
    renderSearch();
    await submitQuery();
    expect(screen.getByText("905+ matches in 1 file")).toBeVisible();
    expect(screen.getByText("First 1")).toBeVisible();

    searchWorkspaceMock.mockRejectedValueOnce(new Error("The local search index is rebuilding"));
    await submitQuery("failure");
    expect(screen.getByRole("alert")).toHaveTextContent("The local search index is rebuilding");
    expect(screen.queryByTitle("src/bounded.ts")).not.toBeInTheDocument();
  });

  it("does not claim a complete negative when a bounded subset has no observed match", async () => {
    searchWorkspaceMock.mockResolvedValue(result([], { totalMatches: 0, truncated: true }));
    renderSearch();
    await submitQuery("xy");

    expect(screen.getByText(/No match observed.*bounded search window/)).toBeVisible();
    expect(screen.getByText("0 observed matches in 0 files")).toBeVisible();
    expect(screen.getByText("Bounded subset")).toBeVisible();
    expect(screen.queryByText("First 0")).not.toBeInTheDocument();
    expect(screen.queryByText(/No indexed matches/)).not.toBeInTheDocument();
  });

  it("uses one tree tab stop and supports parent-aware arrow navigation", async () => {
    searchWorkspaceMock.mockResolvedValue(result([
      match("one", "src/a.ts", 1, "checkout one"),
      match("two", "src/a.ts", 4, "checkout two"),
    ]));
    renderSearch();
    await submitQuery();
    const items = screen.getAllByRole("treeitem") as HTMLButtonElement[];
    expect(items.filter((item) => item.tabIndex === 0)).toHaveLength(1);
    items[0]?.focus();
    expect(items[0]).toHaveFocus();

    fireEvent.keyDown(items[0]!, { key: "ArrowRight" });
    expect(items[1]).toHaveFocus();
    fireEvent.keyDown(items[1]!, { key: "ArrowDown" });
    expect(items[2]).toHaveFocus();
    fireEvent.keyDown(items[2]!, { key: "ArrowLeft" });
    expect(items[0]).toHaveFocus();
    fireEvent.keyDown(items[0]!, { key: "ArrowLeft" });
    expect(screen.getAllByRole("treeitem")).toHaveLength(1);
  });

  it("does not surface a rejection from the previously open workspace", async () => {
    let rejectPrevious!: (reason: Error) => void;
    searchWorkspaceMock.mockImplementation(() => new Promise((_resolve, reject) => {
      rejectPrevious = reject;
    }));
    const shared = {
      workspaceName: "first",
      disabled: false,
      onOpenMatch: vi.fn(),
      onOpenFolder: vi.fn(),
    };
    const { rerender } = render(
      <SearchPanel workspaceId="workspace:first" workspaceGeneration={1} {...shared} />,
    );
    await submitQuery();

    rerender(
      <SearchPanel workspaceId="workspace:second" workspaceGeneration={2} {...shared} />,
    );
    await act(async () => rejectPrevious(new Error("stale workspace error")));

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("Search safe indexed source, paths, symbols, APIs, events, headings, and sentences.")).toBeVisible();
  });
});
