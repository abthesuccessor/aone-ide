import { useCallback, useEffect, useRef, useState } from "react";
import type { GraphLens } from "../types";
import type { MainView } from "./model";

export function useGraphFocus(lens: GraphLens, mainView: MainView) {
  const [active, setActive] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (mainView !== "graph") setActive(false);
    else if (lens === "api-data") setActive(true);
  }, [lens, mainView]);

  useEffect(() => {
    if (!active) return;
    const exitOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      setActive(false);
      buttonRef.current?.focus();
    };
    window.addEventListener("keydown", exitOnEscape);
    return () => window.removeEventListener("keydown", exitOnEscape);
  }, [active]);

  const toggle = useCallback(() => setActive((current) => !current), []);
  return { active, buttonRef, toggle };
}
