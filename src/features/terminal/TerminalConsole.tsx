import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { Plus, TerminalWindow, Trash } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { WorkbenchSettings } from "../editor/model";
import { themeDescriptor } from "../editor/themes";
import {
  closeTerminal,
  listTerminalProfiles,
  openTerminal,
  resizeTerminal,
  subscribeTerminalEvents,
  writeTerminal,
} from "../../lib/terminalBridge";
import type { TerminalEvent, TerminalOpenResult, TerminalProfile } from "./model";

interface TerminalConsoleProps {
  settings: WorkbenchSettings;
  visible: boolean;
}

const MAX_PENDING_OPEN_EVENTS = 32;

function decodeBase64(data: string) {
  const binary = window.atob(data);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function failureMessage(failure: unknown, fallback: string) {
  if (failure instanceof Error && failure.message.trim()) return failure.message;
  if (typeof failure === "string" && failure.trim()) return failure;
  return fallback;
}

export function TerminalConsole({ settings, visible }: TerminalConsoleProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<TerminalOpenResult | null>(null);
  const openingRef = useRef(false);
  const pendingEventsRef = useRef<TerminalEvent[]>([]);
  const pendingDroppedRef = useRef(0);
  const [profiles, setProfiles] = useState<TerminalProfile[]>([]);
  const [profileId, setProfileId] = useState("");
  const [session, setSession] = useState<TerminalOpenResult | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    sessionRef.current = session;
  }, [session]);

  const applyTerminalEvent = useCallback((event: TerminalEvent) => {
    if (event.kind === "data" && event.data) terminalRef.current?.write(decodeBase64(event.data));
    if (event.kind === "error") {
      terminalRef.current?.writeln(`\r\n[Aone terminal error] ${event.detail ?? "Terminal failed"}`);
      setError(event.detail ?? "Terminal failed");
    }
    if (event.kind === "exit") {
      terminalRef.current?.writeln(`\r\n[process exited${event.exitCode === undefined ? "" : ` ${event.exitCode}`}]`);
      sessionRef.current = null;
      setSession(null);
    }
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let disposed = false;
    const reportActionFailure = (failure: unknown, fallback: string) => {
      if (!disposed) setError(failureMessage(failure, fallback));
    };
    const reportInactiveSession = (sessionId: string) => {
      if (disposed || sessionRef.current?.sessionId !== sessionId) return;
      void closeTerminal({ sessionId }).catch(() => undefined);
      sessionRef.current = null;
      setSession((current) => current?.sessionId === sessionId ? null : current);
      setError("Terminal session is no longer active");
    };
    const terminal = new Terminal({
      allowProposedApi: false,
      convertEol: false,
      cursorBlink: settings.terminalCursorBlink,
      fontFamily: settings.terminalFontFamily,
      fontSize: settings.terminalFontSize,
      scrollback: 5_000,
      theme: themeDescriptor(settings.theme).terminal,
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host);
    terminalRef.current = terminal;
    fitRef.current = fit;
    const data = terminal.onData((value) => {
      const active = sessionRef.current;
      if (active) {
        void writeTerminal({ sessionId: active.sessionId, data: value, encoding: "text" })
          .then((result) => {
            if (!result.accepted) reportInactiveSession(active.sessionId);
          })
          .catch((failure) => reportActionFailure(failure, "Could not write to terminal"));
      }
    });
    const observer = new ResizeObserver(() => {
      if (!host.isConnected || host.clientWidth === 0 || host.clientHeight === 0) return;
      fit.fit();
      const active = sessionRef.current;
      if (active) {
        void resizeTerminal({ sessionId: active.sessionId, columns: terminal.cols, rows: terminal.rows })
          .then((result) => {
            if (!result.accepted) reportInactiveSession(active.sessionId);
          })
          .catch((failure) => reportActionFailure(failure, "Could not resize terminal"));
      }
    });
    observer.observe(host);
    return () => {
      disposed = true;
      observer.disconnect();
      data.dispose();
      terminal.dispose();
      const active = sessionRef.current;
      if (active) void closeTerminal({ sessionId: active.sessionId });
      sessionRef.current = null;
      openingRef.current = false;
      pendingEventsRef.current = [];
      pendingDroppedRef.current = 0;
      terminalRef.current = null;
      fitRef.current = null;
    };
  }, []);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;
    terminal.options.fontFamily = settings.terminalFontFamily;
    terminal.options.fontSize = settings.terminalFontSize;
    terminal.options.cursorBlink = settings.terminalCursorBlink;
    terminal.options.theme = themeDescriptor(settings.theme).terminal;
    if (visible) window.requestAnimationFrame(() => fitRef.current?.fit());
  }, [settings, visible]);

  useEffect(() => {
    let mounted = true;
    let stop: () => void = () => undefined;
    void (async () => {
      try {
        const nextProfiles = await listTerminalProfiles();
        if (!mounted) return;
        const unsubscribe = await subscribeTerminalEvents((event: TerminalEvent) => {
          const active = sessionRef.current;
          if (active?.sessionId === event.sessionId) {
            applyTerminalEvent(event);
            return;
          }
          if (openingRef.current) {
            if (pendingEventsRef.current.length === MAX_PENDING_OPEN_EVENTS) {
              pendingEventsRef.current.shift();
              pendingDroppedRef.current += 1;
            }
            pendingEventsRef.current.push(event);
          }
        });
        if (!mounted) return unsubscribe();
        stop = unsubscribe;
        setProfiles(nextProfiles);
        setProfileId(nextProfiles[0]?.id ?? "");
        setReady(true);
      } catch (failure) {
        if (mounted) setError(failureMessage(failure, "Terminal setup failed"));
      }
    })();
    return () => { mounted = false; stop(); };
  }, [applyTerminalEvent]);

  const open = async () => {
    const terminal = terminalRef.current;
    if (!terminal || !profileId || openingRef.current || sessionRef.current) return;
    openingRef.current = true;
    pendingEventsRef.current = [];
    pendingDroppedRef.current = 0;
    setBusy(true);
    setError(null);
    try {
      fitRef.current?.fit();
      terminal.clear();
      const opened = await openTerminal({ profileId, columns: terminal.cols, rows: terminal.rows });
      sessionRef.current = opened;
      setSession(opened);
      openingRef.current = false;
      const pending = pendingEventsRef.current;
      const dropped = pendingDroppedRef.current;
      pendingEventsRef.current = [];
      pendingDroppedRef.current = 0;
      if (dropped > 0) terminal.writeln(`\r\n[Aone terminal omitted ${dropped} early events]`);
      for (const event of pending) {
        if (event.sessionId === opened.sessionId) applyTerminalEvent(event);
      }
      if (sessionRef.current?.sessionId === opened.sessionId) terminal.focus();
    } catch (failure) {
      setError(failureMessage(failure, "Could not open terminal"));
    } finally {
      openingRef.current = false;
      pendingEventsRef.current = [];
      pendingDroppedRef.current = 0;
      setBusy(false);
    }
  };

  const close = async () => {
    if (!session) return;
    setBusy(true);
    try {
      await closeTerminal({ sessionId: session.sessionId });
      sessionRef.current = null;
      setSession(null);
    }
    catch (failure) { setError(failureMessage(failure, "Could not close terminal")); }
    finally { setBusy(false); }
  };

  return (
    <section className="terminal-console" aria-label="Integrated terminal">
      <div className="terminal-toolbar">
        <TerminalWindow size={13} />
        <select aria-label="Terminal profile" value={profileId} onChange={(event) => setProfileId(event.target.value)} disabled={Boolean(session) || busy}>{profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.label}</option>)}</select>
        <span aria-live="polite" title={session?.cwd}>{session ? `${session.shellLabel} · ${session.cwd}` : "No terminal session"}</span>
        <button type="button" onClick={() => void open()} disabled={!ready || !profileId || Boolean(session) || busy} title="New terminal"><Plus size={13} /> New</button>
        <button type="button" onClick={() => void close()} disabled={!session || busy} title="Close terminal"><Trash size={13} /> Close</button>
      </div>
      {error && <div className="terminal-error" role="alert">{error}</div>}
      <div ref={hostRef} className="terminal-host" />
    </section>
  );
}
