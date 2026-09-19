import {
  Group,
  Panel,
  Separator,
  usePanelRef,
  type GroupProps,
  type PanelProps,
  type SeparatorProps,
} from "react-resizable-panels";

function classes(...values: Array<string | undefined | false>): string {
  return values.filter(Boolean).join(" ");
}

export function ResizablePanelGroup({ className, resizeTargetMinimumSize, ...props }: GroupProps) {
  return (
    <Group
      className={classes("resizable-panel-group", className)}
      resizeTargetMinimumSize={resizeTargetMinimumSize ?? { fine: 7, coarse: 18 }}
      {...props}
    />
  );
}

export function ResizablePanel({ className, ...props }: PanelProps) {
  return <Panel className={classes("resizable-panel", className)} {...props} />;
}

export function ResizableHandle({
  className,
  withHandle = true,
  ...props
}: SeparatorProps & { withHandle?: boolean }) {
  return (
    <Separator className={classes("resizable-handle", className)} {...props}>
      {withHandle && <span className="resizable-handle-grip" aria-hidden="true" />}
    </Separator>
  );
}

export { usePanelRef };
