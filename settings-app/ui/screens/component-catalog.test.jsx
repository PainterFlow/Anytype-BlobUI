import React from "react";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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
    const help = screen.getByRole("button", { name: "About color rules" });
    expect(document.getElementById(help.getAttribute("aria-describedby")).getAttribute("role")).toBe("tooltip");
    expect(help.querySelector("svg").getAttribute("viewBox")).toBe("0 0 20 20");
    expect(screen.getByRole("tablist", { name: "Showcase examples" })).toBeTruthy();
  });

  it("shows the remaining sample calendar events when expanded", () => {
    setMotionApis();
    render(<ComponentCatalog onClose={() => {}} />);

    const calendar = screen.getByRole("grid", { name: "October 2026 sample calendar" });
    expect(within(calendar).getAllByRole("columnheader")).toHaveLength(7);
    expect(within(calendar).getAllByRole("gridcell")).toHaveLength(42);
    expect(within(calendar).getByRole("gridcell", { name: "September 28, 2026" })).toBeTruthy();
    expect(within(calendar).getByRole("gridcell", { name: "November 8, 2026" })).toBeTruthy();

    const overflow = within(calendar).getByRole("button", { name: "Show 2 more events for October 12, 2026" });
    const eventList = document.getElementById(overflow.getAttribute("aria-controls"));
    expect(overflow.getAttribute("aria-expanded")).toBe("false");
    expect(within(eventList).queryByText("Stamp stars")).toBeNull();
    overflow.focus();
    fireEvent.click(overflow);

    expect(within(eventList).getByText("Stamp stars")).toBeTruthy();
    expect(within(eventList).getByText("Dry herbs")).toBeTruthy();
    expect(within(calendar).getByRole("button", { name: "Show fewer events for October 12, 2026" })).toBe(overflow);
    expect(document.activeElement).toBe(overflow);
    expect(overflow.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(overflow);
    expect(within(eventList).queryByText("Stamp stars")).toBeNull();
  });

  it("uses the selected Showcase tab to display the Kanban preview", () => {
    setMotionApis();
    render(<ComponentCatalog onClose={() => {}} />);

    fireEvent.click(screen.getByRole("tab", { name: "Kanban" }));

    const board = document.querySelector(".kanban-showcase-board");
    expect(board.dataset.layout).toBe("balanced");
    expect(board.dataset.formulaDisplay).toBe("balanced");
    expect(board.dataset.collapsed).toBe("false");
    const collect = screen.getByRole("region", { name: "Collect category" });
    expect(within(collect).getByText("35 min")).toBeTruthy();
    expect(within(collect).getByText("Gather pebbles")).toBeTruthy();
    const help = screen.getByRole("button", { name: "Kanban showcase help" });
    expect(help.parentElement.previousElementSibling).toBe(screen.getByRole("heading", { name: "Craft board" }));
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
