import { ArrowCounterClockwise, GearSix, X } from "@phosphor-icons/react";
import { useEffect, useRef } from "react";
import type { WorkbenchSettings } from "./model";
import { themeDescriptor, WORKBENCH_THEME_OPTIONS } from "./themes";

interface SettingsPanelProps {
  open: boolean;
  settings: WorkbenchSettings;
  onChange: (patch: Partial<WorkbenchSettings>) => void;
  onReset: () => void;
  onClose: () => void;
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (value: boolean) => void; label: string }) {
  return (
    <button type="button" className={`settings-toggle${checked ? " is-on" : ""}`} onClick={() => onChange(!checked)} aria-pressed={checked}>
      <i /><span>{label}</span>
    </button>
  );
}

export function SettingsPanel({ open, settings, onChange, onReset, onClose }: SettingsPanelProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const doneButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();
    const containKeyboard = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey;
      const blocksWorkbenchAction = (
        command && ["k", "r", "1", "2", "enter"].includes(event.key.toLowerCase())
      ) || (command && event.shiftKey && event.key.toLowerCase() === "f")
        || (event.shiftKey && event.key === "F5");
      if (blocksWorkbenchAction) {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (event.key === "Tab") {
        const first = closeButtonRef.current;
        const last = doneButtonRef.current;
        if (!first || !last) return;
        if (event.shiftKey && (document.activeElement === first || !dialogRef.current?.contains(document.activeElement))) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && (document.activeElement === last || !dialogRef.current?.contains(document.activeElement))) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", containKeyboard, true);
    return () => {
      window.removeEventListener("keydown", containKeyboard, true);
      const previous = returnFocusRef.current;
      if (previous?.isConnected) previous.focus();
    };
  }, [open]);

  if (!open) return null;
  return (
    <div className="settings-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section ref={dialogRef} className="settings-panel" role="dialog" aria-modal="true" aria-label="Workbench settings">
        <header>
          <div><GearSix size={18} weight="duotone" /><div><strong>Settings</strong><span>Saved locally on this Mac</span></div></div>
          <button ref={closeButtonRef} type="button" className="icon-button" onClick={onClose} aria-label="Close settings"><X size={14} /></button>
        </header>
        <div className="settings-content">
          <section>
            <h3>Appearance</h3>
            <label><span>Color theme</span><select value={settings.theme} onChange={(event) => onChange({ theme: event.target.value as WorkbenchSettings["theme"] })}>
              {WORKBENCH_THEME_OPTIONS.map((theme) => <option key={theme.id} value={theme.id}>{theme.label}</option>)}
            </select></label>
            <p className="settings-theme-description">{themeDescriptor(settings.theme).description}</p>
            <label><span>Editor font family</span><input value={settings.fontFamily} onChange={(event) => onChange({ fontFamily: event.target.value })} spellCheck={false} /></label>
            <div className="settings-pair">
              <label><span>Font size</span><input type="number" min={10} max={24} value={settings.fontSize} onChange={(event) => onChange({ fontSize: Number(event.target.value) })} /></label>
              <label><span>Line height</span><input type="number" min={14} max={40} value={settings.lineHeight} onChange={(event) => onChange({ lineHeight: Number(event.target.value) })} /></label>
            </div>
            <Toggle label="Show minimap" checked={settings.minimap} onChange={(minimap) => onChange({ minimap })} />
          </section>
          <section>
            <h3>Editor</h3>
            <div className="settings-pair">
              <label><span>Word wrap</span><select value={settings.wordWrap} onChange={(event) => onChange({ wordWrap: event.target.value as WorkbenchSettings["wordWrap"] })}><option value="off">Off</option><option value="on">Viewport</option><option value="bounded">Bounded</option></select></label>
              <label><span>Wrap column</span><input type="number" min={40} max={240} value={settings.wordWrapColumn} onChange={(event) => onChange({ wordWrapColumn: Number(event.target.value) })} /></label>
            </div>
            <div className="settings-pair">
              <label><span>Tab size</span><input type="number" min={1} max={8} value={settings.tabSize} onChange={(event) => onChange({ tabSize: Number(event.target.value) })} /></label>
              <Toggle label="Insert spaces" checked={settings.insertSpaces} onChange={(insertSpaces) => onChange({ insertSpaces })} />
            </div>
            <Toggle label="Format on save" checked={settings.formatOnSave} onChange={(formatOnSave) => onChange({ formatOnSave })} />
          </section>
          <section>
            <h3>Terminal</h3>
            <label><span>Terminal font family</span><input value={settings.terminalFontFamily} onChange={(event) => onChange({ terminalFontFamily: event.target.value })} spellCheck={false} /></label>
            <label><span>Terminal font size</span><input type="number" min={10} max={22} value={settings.terminalFontSize} onChange={(event) => onChange({ terminalFontSize: Number(event.target.value) })} /></label>
            <Toggle label="Blinking cursor" checked={settings.terminalCursorBlink} onChange={(terminalCursorBlink) => onChange({ terminalCursorBlink })} />
          </section>
        </div>
        <footer><button type="button" className="secondary-button" onClick={onReset}><ArrowCounterClockwise size={13} /> Reset defaults</button><button ref={doneButtonRef} type="button" className="primary-button" onClick={onClose}>Done</button></footer>
      </section>
    </div>
  );
}
