import { useCallback, useLayoutEffect, useRef } from "react";

export interface SlidingTabsController {
  barRef: React.RefObject<HTMLDivElement | null>;
  pillRef: React.RefObject<HTMLSpanElement | null>;
  registerTab: (value: string) => (element: HTMLButtonElement | null) => void;
  moveToValue: (value: string, animate?: boolean) => void;
}

export function useSlidingTabs(activeValue: string): SlidingTabsController {
  const barRef = useRef<HTMLDivElement>(null);
  const pillRef = useRef<HTMLSpanElement>(null);
  const tabsRef = useRef(new Map<string, HTMLButtonElement>());

  const moveToValue = useCallback((value: string, animate = true) => {
    const pill = pillRef.current;
    const tab = tabsRef.current.get(value);
    if (!pill || !tab) return;

    if (!animate) {
      const previous = pill.style.transition;
      pill.style.transition = "none";
      pill.style.transform = `translateX(${tab.offsetLeft}px)`;
      pill.style.width = `${tab.offsetWidth}px`;
      void pill.offsetWidth;
      pill.style.transition = previous;
      return;
    }

    pill.style.transform = `translateX(${tab.offsetLeft}px)`;
    pill.style.width = `${tab.offsetWidth}px`;
  }, []);

  const registerTab = useCallback(
    (value: string) => (element: HTMLButtonElement | null) => {
      if (element) tabsRef.current.set(value, element);
      else tabsRef.current.delete(value);
    },
    [],
  );

  useLayoutEffect(() => {
    const frame = requestAnimationFrame(() => moveToValue(activeValue, false));
    const resize = () => moveToValue(activeValue, false);
    window.addEventListener("resize", resize);

    const observer = typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(resize);
    if (barRef.current) observer?.observe(barRef.current);

    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", resize);
      observer?.disconnect();
    };
  }, [activeValue, moveToValue]);

  useLayoutEffect(() => {
    moveToValue(activeValue, true);
  }, [activeValue, moveToValue]);

  return { barRef, pillRef, registerTab, moveToValue };
}
