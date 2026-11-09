import React from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ComponentCatalog } from "./component-catalog.jsx";

const originalMatchMedia = window.matchMedia;
const originalRequestAnimationFrame = window.requestAnimationFrame;
const originalCancelAnimationFrame = window.cancelAnimationFrame;

function setMotionApis(matches = false) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: () => ({ matches }),
  });
  Object.defineProperty(window, "requestAnimationFrame", {
    configurable: true,
    writable: true,
    value: callback => window.setTimeout(() => callback(0), 0),
  });
  Object.defineProperty(window, "cancelAnimationFrame", {
    configurable: true,
    writable: true,
    value: timer => window.clearTimeout(timer),
  });
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  Object.defineProperty(window, "matchMedia", { configurable: true, value: originalMatchMedia });
  Object.defineProperty(window, "requestAnimationFrame", { configurable: true, value: originalRequestAnimationFrame });
  Object.defineProperty(window, "cancelAnimationFrame", { configurable: true, value: originalCancelAnimationFrame });
});

describe("Component catalog", () => {
  it("renders the shared settings controls and application sidebar", () => {
    setMotionApis();
    render(<ComponentCatalog onClose={() => {}} />);

    expect(screen.getByRole("main", { name: "Component catalog" })).toBeTruthy();
    expect(screen.getByRole("tablist", { name: "Component examples" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: /Enable sample setting/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Board layout/ })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Catalog preview navigation" })).toBeTruthy();
  });

  it("plays the leaving state before closing the catalog window", async () => {
    setMotionApis();
    vi.useFakeTimers();
    const onClose = vi.fn();
    render(<ComponentCatalog onClose={onClose} />);

    fireEvent.click(screen.getByRole("button", { name: "Close component catalog" }));
    expect(document.querySelector(".component-catalog-page").classList.contains("leaving")).toBe(true);
    expect(onClose).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(320);
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
