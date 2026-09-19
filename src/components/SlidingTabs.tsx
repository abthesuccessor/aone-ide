import type { KeyboardEvent } from "react";
import { useSlidingTabs } from "../hooks/useSlidingTabs";

export interface TabOption<T extends string> {
  value: T;
  label: string;
  count?: number;
}

interface SlidingTabsProps<T extends string> {
  value: T;
  options: readonly TabOption<T>[];
  onChange: (value: T) => void;
  label: string;
  className?: string;
}

export function SlidingTabs<T extends string>({
  value,
  options,
  onChange,
  label,
  className = "",
}: SlidingTabsProps<T>) {
  const { barRef, pillRef, registerTab } = useSlidingTabs(value);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const currentIndex = options.findIndex((option) => option.value === value);
    if (currentIndex < 0) return;

    let nextIndex = currentIndex;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (currentIndex + 1) % options.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (currentIndex - 1 + options.length) % options.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = options.length - 1;
    } else {
      return;
    }

    event.preventDefault();
    const next = options[nextIndex];
    if (next) {
      onChange(next.value);
      const tabs = event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]');
      tabs[nextIndex]?.focus();
    }
  };

  return (
    <div
      ref={barRef}
      className={`t-tabs ${className}`.trim()}
      role="tablist"
      aria-label={label}
      onKeyDown={onKeyDown}
    >
      <span ref={pillRef} className="t-tabs-pill" aria-hidden="true" />
      {options.map((option) => (
        <button
          key={option.value}
          ref={registerTab(option.value)}
          className="t-tab"
          role="tab"
          type="button"
          aria-selected={option.value === value}
          tabIndex={option.value === value ? 0 : -1}
          onClick={() => onChange(option.value)}
        >
          <span>{option.label}</span>
          {option.count !== undefined && <span className="tab-count">{option.count}</span>}
        </button>
      ))}
    </div>
  );
}
