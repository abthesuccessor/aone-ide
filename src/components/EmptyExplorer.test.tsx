import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { EmptyExplorer } from "./EmptyExplorer";

describe("EmptyExplorer", () => {
  it("shows only native file and folder entry actions", () => {
    const onOpenFile = vi.fn();
    const onOpenFolder = vi.fn();
    render(
      <EmptyExplorer busy={false} onOpenFile={onOpenFile} onOpenFolder={onOpenFolder} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open File" }));
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));
    expect(screen.getByRole("heading", { name: "Explorer" })).toBeVisible();
    expect(screen.getByText("No folder opened")).toBeVisible();
    expect(onOpenFile).toHaveBeenCalledTimes(1);
    expect(onOpenFolder).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/clone|create project/i)).not.toBeInTheDocument();
  });
});
