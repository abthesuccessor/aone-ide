import { Bug, Files, GearSix, GitBranch, Graph, MagnifyingGlass, PlugsConnected, Robot } from "@phosphor-icons/react";
import type { ActivityView } from "../app/useDeveloperWorkbench";
import { Button } from "./ui/button";

interface ActivityBarProps {
  active: ActivityView;
  onChange: (view: ActivityView) => void;
  onIntent?: (view: ActivityView) => void;
  onSettings: () => void;
}

const views = [
  { id: "explorer" as const, label: "Explorer", icon: Files },
  { id: "search" as const, label: "Search", icon: MagnifyingGlass },
  { id: "source-control" as const, label: "Source Control", icon: GitBranch },
  { id: "graph" as const, label: "Code Graph", icon: Graph },
  { id: "debugger" as const, label: "Cooperative Debugger", icon: Bug },
  { id: "agent-setup" as const, label: "Project Agent", icon: Robot },
  { id: "tools" as const, label: "MCP and CLI", icon: PlugsConnected },
];

export function ActivityBar({ active, onChange, onIntent, onSettings }: ActivityBarProps) {
  return (
    <nav className="activity-bar" aria-label="Primary side bar">
      <div>
        {views.map(({ id, label, icon: Icon }) => (
          <Button
            key={id}
            id={`activity-${id}`}
            type="button"
            variant="ghost"
            size="icon-lg"
            className={active === id ? "is-active" : ""}
            aria-label={label}
            aria-pressed={active === id}
            title={label}
            onFocus={() => onIntent?.(id)}
            onPointerEnter={() => onIntent?.(id)}
            onPointerDown={() => onIntent?.(id)}
            onClick={() => onChange(id)}
          ><Icon className="activity-icon" size={24} weight="regular" /></Button>
        ))}
      </div>
      <Button type="button" variant="ghost" size="icon-lg" aria-label="Settings" title="Settings" onClick={onSettings}>
        <GearSix className="activity-icon" size={24} />
      </Button>
    </nav>
  );
}
