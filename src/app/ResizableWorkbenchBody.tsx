import { useEffect, type ReactNode } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  usePanelRef,
} from "../components/ui/Resizable";

interface ResizableWorkbenchBodyProps {
  stage: ReactNode;
  console: ReactNode;
  consoleCollapsed: boolean;
  onConsoleCollapsedChange?: (collapsed: boolean) => void;
}

export function ResizableWorkbenchBody({
  stage,
  console,
  consoleCollapsed,
  onConsoleCollapsedChange,
}: ResizableWorkbenchBodyProps) {
  const consolePanelRef = usePanelRef();
  useEffect(() => {
    if (!consolePanelRef.current) return;
    if (consoleCollapsed) consolePanelRef.current.collapse();
    else consolePanelRef.current.expand();
  }, [consoleCollapsed, consolePanelRef]);
  return (
    <ResizablePanelGroup orientation="vertical" id="aone-workbench-body" className="workbench-resizable">
      <ResizablePanel id="workbench-stage-panel" minSize="220px" defaultSize="72%" className="workbench-stage-panel">
        {stage}
      </ResizablePanel>
      <ResizableHandle aria-label="Resize terminal and runtime console" />
      <ResizablePanel
        id="workbench-console-panel"
        panelRef={consolePanelRef}
        minSize="145px"
        maxSize="62%"
        defaultSize="28%"
        collapsedSize="38px"
        collapsible
        onResize={(size, _id, previous) => {
          if (!previous || !onConsoleCollapsedChange) return;
          const collapsed = size.inPixels <= 40;
          if (collapsed !== consoleCollapsed) onConsoleCollapsedChange(collapsed);
        }}
        className="workbench-console-panel"
      >
        {console}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
