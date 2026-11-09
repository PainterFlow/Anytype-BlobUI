import React from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.reducedMotion;
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

  it("requests a native window close when the X is clicked", () => {
    setMotionApis();
    const close = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("__TAURI__", { window: { getCurrentWindow: () => ({ close }) } });
    render(<ComponentCatalog />);

    fireEvent.click(screen.getByRole("button", { name: "Close component catalog" }));

    expect(close).toHaveBeenCalledTimes(1);
  });

  it("binds the sample secret visibility control to the shared controller", () => {
    setMotionApis();
    render(<ComponentCatalog onClose={() => {}} />);

    const input = document.getElementById("catalog-secret");
    const toggle = screen.getByRole("button", { name: "Show sample value" });
    expect(input.type).toBe("password");

    fireEvent.click(toggle);

    expect(input.type).toBe("text");
    expect(toggle.getAttribute("aria-pressed")).toBe("true");
    expect(toggle.getAttribute("aria-label")).toBe("Hide sample value");
  });

  it("keeps multi-select picker menus outside the clipped settings group", () => {
    setMotionApis();
    render(<ComponentCatalog onClose={() => {}} />);

    const sources = screen.getByRole("button", { name: /Source categories/ });
    fireEvent.click(sources);

    const listbox = screen.getByRole("listbox", { name: "Source categories" });
    expect(listbox.parentElement.classList.contains("component-catalog-picker-surface")).toBe(true);
    fireEvent.click(screen.getByRole("option", { name: "Collections" }));

    expect(sources.textContent).toContain("2 selected");
    expect(screen.getByRole("option", { name: "Collections" }).getAttribute("aria-selected")).toBe("true");
  });

  it("filters example results and leaves the catalog open while a dialog handles Escape", () => {
    setMotionApis(true);
    document.documentElement.dataset.reducedMotion = "true";
    const onClose = vi.fn();
    render(<ComponentCatalog onClose={onClose} />);

    fireEvent.click(document.getElementById("catalog-search-toggle"));
    fireEvent.change(document.getElementById("catalog-search"), { target: { value: "tabs" } });
    expect(screen.getByRole("button", { name: /Tabs/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Buttons/ })).toBeNull();
    expect(screen.getByText("1 matches")).toBeTruthy();

    const page = document.querySelector(".component-catalog-page");
    page.inert = false;
    fireEvent.click(screen.getByRole("button", { name: "Open sample dialog" }));
    const dialog = screen.getByRole("dialog", { name: "Apply sample change?" });
    fireEvent.keyDown(dialog, { key: "Escape", bubbles: true });

    expect(dialog.closest(".confirm-backdrop").hidden).toBe(true);
    expect(page.inert).toBe(false);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("isolates the page while the sample dialog is open", () => {
    setMotionApis(true);
    document.documentElement.dataset.reducedMotion = "true";
    render(<ComponentCatalog onClose={() => {}} />);
    const page = document.querySelector(".component-catalog-page");
    page.inert = false;

    fireEvent.click(screen.getByRole("button", { name: "Open sample dialog" }));

    expect(screen.getByRole("dialog", { name: "Apply sample change?" })).toBeTruthy();
    expect(page.inert).toBe(true);
  });
});
