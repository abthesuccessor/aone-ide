import type { ReactNode } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../components/ui/Resizable";

interface ResizableIdeLayoutProps {
  navigator?: () => ReactNode;
  navigatorKey?: string;
  workbench: ReactNode;
  inspector?: ReactNode;
  focus?: boolean;
}

export function ResizableIdeLayout({ navigator, navigatorKey, workbench, inspector, focus = false }: ResizableIdeLayoutProps) {
  if (focus || (!navigator && !inspector)) {
    return <div className="ide-resizable-single">{workbench}</div>;
  }
  // react-resizable-panels validates layout values in rendered panel order.
  // Keep this record in the same navigator -> workbench -> inspector order.
  const defaultLayout: Record<string, number> = {};
  if (navigator) defaultLayout["ide-navigator"] = 22;
  defaultLayout["ide-workbench"] = inspector ? 56 : 78;
  if (inspector) defaultLayout["ide-inspector"] = 22;
  return (
    <ResizablePanelGroup orientation="horizontal" id="aone-ide-layout" className="ide-resizable" defaultLayout={defaultLayout}>
      {navigator && (
        <>
          <ResizablePanel key={navigatorKey} id="ide-navigator" defaultSize="22%" minSize="18%" maxSize="32%" className="ide-navigator-panel">
            <div key={navigatorKey} className="ide-navigator-content">{navigator()}</div>
          </ResizablePanel>
          <ResizableHandle aria-label="Resize Explorer" />
        </>
      )}
      <ResizablePanel id="ide-workbench" minSize="42%" className="ide-workbench-panel">
        {workbench}
      </ResizablePanel>
      {inspector && (
        <>
          <ResizableHandle aria-label="Resize evidence inspector" />
          <ResizablePanel id="ide-inspector" defaultSize="22%" minSize="20%" maxSize="36%" className="ide-inspector-panel">
            {inspector}
          </ResizablePanel>
        </>
      )}
    </ResizablePanelGroup>
  );
}
