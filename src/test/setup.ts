import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

class TestResizeObserver implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

Object.defineProperty(globalThis, "ResizeObserver", {
  configurable: true,
  value: TestResizeObserver,
});

Object.defineProperty(window, "matchMedia", {
  configurable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  }),
});

Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
  configurable: true,
  get() { return 82; },
});

Object.defineProperty(HTMLElement.prototype, "offsetLeft", {
  configurable: true,
  get() {
    return this.parentElement ? Array.from(this.parentElement.children).indexOf(this) * 85 : 0;
  },
});

if (!(SVGElement.prototype as SVGElement & { setPointerCapture?: (pointerId: number) => void }).setPointerCapture) {
  Object.defineProperty(SVGElement.prototype, "setPointerCapture", {
    configurable: true,
    value: () => undefined,
  });
}

afterEach(() => {
  cleanup();
  delete window.__TAURI_INTERNALS__;
});
