import { render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { AppWorkbench } from "./AppWorkbench";

describe("AppWorkbench empty editor", () => {
  it("shows the welcome actions without the A1 watermark", () => {
    const controller = {
      workspace: null,
      openingWorkspace: false,
      scanProgress: null,
      setActivityView: vi.fn(),
      setPrimarySidebarVisible: vi.fn(),
      setMainView: vi.fn(),
      handleOpenFile: vi.fn(),
      handleOpenFolder: vi.fn(),
    } as unknown as ComponentProps<typeof AppWorkbench>["controller"];

    render(<AppWorkbench controller={controller} />);

    expect(screen.getByRole("heading", { name: "Aone IDE" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Open Folder..." })).toBeVisible();
    expect(screen.queryByText("A1")).not.toBeInTheDocument();
    expect(document.querySelector(".welcome-watermark")).not.toBeInTheDocument();
  });
});
