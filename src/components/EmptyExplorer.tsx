import { FileArrowUp, FolderOpen } from "@phosphor-icons/react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "./WorkbenchSidebarChrome";

interface EmptyExplorerProps {
  busy: boolean;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  title?: string;
  sectionTitle?: string;
}

export function EmptyExplorer({ busy, onOpenFile, onOpenFolder, title = "Explorer", sectionTitle = "No folder opened" }: EmptyExplorerProps) {
  return (
    <aside className="workspace-panel empty-explorer" aria-label={title}>
      <WorkbenchSidebarHeader title={title} />
      <WorkbenchSidebarSection title={sectionTitle} />
      <div className="empty-explorer-actions">
        <p>No folder is open.</p>
        <button type="button" onClick={onOpenFile} disabled={busy}>
          <FileArrowUp size={15} /> Open File
        </button>
        <button type="button" onClick={onOpenFolder} disabled={busy}>
          <FolderOpen size={15} /> Open Folder
        </button>
      </div>
    </aside>
  );
}
