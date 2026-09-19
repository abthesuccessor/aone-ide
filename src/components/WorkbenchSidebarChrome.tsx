import { CaretDown } from "@phosphor-icons/react";
import type { ReactNode } from "react";

interface WorkbenchSidebarHeaderProps {
  title: string;
  actions?: ReactNode;
}

interface WorkbenchSidebarSectionProps {
  title: string;
  detail?: string;
  count?: number | string;
}

export function WorkbenchSidebarHeader({ title, actions }: WorkbenchSidebarHeaderProps) {
  return (
    <header className="workbench-sidebar-header">
      <h2>{title}</h2>
      {actions && <div className="workbench-sidebar-actions">{actions}</div>}
    </header>
  );
}

export function WorkbenchSidebarSection({ title, detail, count }: WorkbenchSidebarSectionProps) {
  return (
    <div className="workbench-sidebar-section" title={detail ? `${title} · ${detail}` : title}>
      <CaretDown size={12} weight="bold" aria-hidden="true" />
      <strong>{title}</strong>
      {detail && <span>{detail}</span>}
      {count !== undefined && <small>{count}</small>}
    </div>
  );
}
