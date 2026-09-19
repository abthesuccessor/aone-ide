import { CheckCircle, Info, Warning, XCircle } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

export type ToastTone = "success" | "info" | "warning" | "error";

export interface ToastMessage {
  id: number;
  text: string;
  tone: ToastTone;
}

interface ToastProps {
  message: ToastMessage | null;
  onDismiss: () => void;
}

export function Toast({ message, onDismiss }: ToastProps) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!message) {
      setOpen(false);
      return;
    }
    const frame = requestAnimationFrame(() => setOpen(true));
    const closeTimer = window.setTimeout(() => setOpen(false), 2_800);
    const removeTimer = window.setTimeout(onDismiss, 3_100);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(closeTimer);
      window.clearTimeout(removeTimer);
    };
  }, [message, onDismiss]);

  if (!message) return null;
  const Icon = message.tone === "success"
    ? CheckCircle
    : message.tone === "warning"
      ? Warning
      : message.tone === "error"
        ? XCircle
        : Info;
  return (
    <div className={`t-toast toast toast-${message.tone}${open ? " is-open" : ""}`} role="status" aria-live="polite">
      <Icon size={16} weight="fill" />
      <span>{message.text}</span>
    </div>
  );
}
