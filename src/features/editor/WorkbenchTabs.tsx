import { BracketsCurly, PushPin, X } from "@phosphor-icons/react";
import { useEffect, useRef } from "react";
import type { EditorDocument } from "./model";

interface WorkbenchTabsProps {
  documents: EditorDocument[];
  activePath: string | null;
  onDocument: (path: string) => void;
  onPin: (path: string) => void;
  onClose: (path: string) => boolean;
  closeDisabled?: boolean;
}

function fileName(path: string) {
  return path.split("/").pop() ?? path;
}

function moveTabFocus(event: React.KeyboardEvent<HTMLButtonElement>) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  const tabs = Array.from(
    event.currentTarget.closest("[role='tablist']")?.querySelectorAll<HTMLButtonElement>(
      "[role='tab']",
    ) ?? [],
  );
  const index = tabs.indexOf(event.currentTarget);
  const next =
    event.key === "Home"
      ? tabs[0]
      : event.key === "End"
        ? tabs[tabs.length - 1]
        : tabs[
            (index + (event.key === "ArrowLeft" ? -1 : 1) + tabs.length) %
              tabs.length
          ];
  event.preventDefault();
  next?.focus();
  next?.click();
}

export function WorkbenchTabs({
  documents,
  activePath,
  onDocument,
  onPin,
  onClose,
  closeDisabled = false,
}: WorkbenchTabsProps) {
  const documentRefs = useRef(new Map<string, HTMLButtonElement>());
  const pendingFocus = useRef<string | null>(null);
  const activeDocumentOpen = documents.some(
    (document) => document.relativePath === activePath,
  );

  useEffect(() => {
    const target = pendingFocus.current;
    if (!target) return;
    pendingFocus.current = null;
    documentRefs.current.get(target)?.focus();
  }, [documents]);

  const closeAndPreserveFocus = (relativePath: string) => {
    if (closeDisabled) return;
    const index = documents.findIndex(
      (document) => document.relativePath === relativePath,
    );
    const nextTarget =
      documents[index - 1]?.relativePath ??
      documents[index + 1]?.relativePath;
    if (onClose(relativePath) && nextTarget) pendingFocus.current = nextTarget;
  };

  return (
    <div className="editor-tabs" role="tablist" aria-label="Open editors">
      {documents.map((document, index) => {
        const active = activePath === document.relativePath;
        const name = fileName(document.relativePath);
        return (
          <div
            role="presentation"
            className={`editor-tab-shell${active ? " is-active" : ""}${document.preview ? " is-preview" : ""}${document.dirty ? " is-dirty" : ""}`}
            key={document.relativePath}
          >
            <button
              type="button"
              role="tab"
              aria-selected={active}
              tabIndex={active || (!activeDocumentOpen && index === 0) ? 0 : -1}
              aria-label={`${name}${document.dirty ? ", unsaved" : document.preview ? ", preview" : ""}`}
              aria-keyshortcuts="Delete"
              className="editor-tab-target"
              ref={(element) => {
                if (element) documentRefs.current.set(document.relativePath, element);
                else documentRefs.current.delete(document.relativePath);
              }}
              onClick={() => onDocument(document.relativePath)}
              onDoubleClick={() => {
                onDocument(document.relativePath);
                onPin(document.relativePath);
              }}
              onKeyDown={(event) => {
                moveTabFocus(event);
                if (event.key === "Delete") {
                  event.preventDefault();
                  closeAndPreserveFocus(document.relativePath);
                }
              }}
              title={document.preview
                ? `${document.relativePath}: preview; double-click, edit, or pin to keep open`
                : document.relativePath}
            >
              <BracketsCurly size={13} weight="duotone" aria-hidden="true" />
              <span className="editor-tab-name">{name}</span>
            </button>
            {document.preview && (
              <button
                type="button"
                className="editor-tab-action editor-tab-pin"
                onClick={() => onPin(document.relativePath)}
                disabled={closeDisabled}
                aria-label={`Keep ${name} open`}
                title="Keep open"
              >
                <PushPin size={11} aria-hidden="true" />
              </button>
            )}
            {document.dirty && <i className="editor-tab-dirty" aria-hidden="true" />}
            <button
              type="button"
              className="editor-tab-action editor-tab-close"
              onClick={() => closeAndPreserveFocus(document.relativePath)}
              disabled={closeDisabled}
              aria-label={`Close ${name}`}
              title={closeDisabled ? "Close unavailable while opening workspace" : "Close"}
            >
              <X size={11} aria-hidden="true" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
