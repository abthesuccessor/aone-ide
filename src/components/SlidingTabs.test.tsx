import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { SlidingTabs } from "./SlidingTabs";

function Harness() {
  const [value, setValue] = useState<"graph" | "source">("graph");
  return (
    <SlidingTabs
      value={value}
      onChange={setValue}
      label="View"
      options={[
        { value: "graph", label: "Graph" },
        { value: "source", label: "Source" },
      ]}
    />
  );
}

describe("SlidingTabs", () => {
  it("supports arrow, Home, and End keyboard navigation", () => {
    render(<Harness />);
    const tablist = screen.getByRole("tablist", { name: "View" });

    fireEvent.keyDown(tablist, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Source" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(tablist, { key: "Home" });
    expect(screen.getByRole("tab", { name: "Graph" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(tablist, { key: "End" });
    expect(screen.getByRole("tab", { name: "Source" })).toHaveAttribute("aria-selected", "true");
  });
});
