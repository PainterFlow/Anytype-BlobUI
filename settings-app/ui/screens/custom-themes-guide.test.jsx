import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import markedSource from "../vendor/marked.umd.js?raw";
import markdownSource from "../markdown.js?raw";
import guideMarkdown from "../../markdowns/custom-themes-guide.md?raw";
import mediaManifest from "../../markdowns/md-images.yaml?raw";

let Preferences;

function modalController({ backdrop, onClose }) {
  let open = false;
  return {
    open() {
      open = true;
      if (backdrop) backdrop.hidden = false;
    },
    close() {
      if (!open) return;
      open = false;
      if (backdrop) backdrop.hidden = true;
      onClose?.();
    },
    isOpen: () => open,
    destroy() {},
  };
}

function markdownResponse(markdown) {
  return { ok: true, text: async () => markdown };
}

beforeAll(async () => {
  window.BlobUI = window.BlobUI || {};
  window.BlobUI.createModalController = modalController;
  window.BlobUI.Scrollbar = { attach: () => ({ destroy() {} }) };
  window.eval(markedSource);
  window.eval(markdownSource);
  ({ Preferences } = await import("./preferences-accessibility.jsx"));
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("Custom themes guide", () => {
  it("opens the Markdown guide from the Custom Themes heading", async () => {
    vi.stubGlobal("fetch", vi.fn(async url => {
      if (String(url).includes("custom-themes-guide.md")) return markdownResponse(guideMarkdown);
      if (String(url).includes("md-images.yaml")) return markdownResponse(mediaManifest);
      return { ok: false, status: 404, text: async () => "" };
    }));

    render(<Preferences values={{
      "enable-custom-themes": false,
      "based-theme": "auto",
      "custom-theme": "",
      "theme-color-value": "#191918",
    }} onChange={() => {}} selectedTab={0} onSelectTab={() => {}} customThemes={[]} />);

    const trigger = screen.getByRole("button", { name: "Custom Themes help" });
    expect(trigger.getAttribute("aria-haspopup")).toBe("dialog");
    expect(trigger.getAttribute("aria-controls")).toBe("custom-themes-guide-dialog");
    fireEvent.click(trigger);

    const dialog = await screen.findByRole("dialog", { name: "Custom themes guide" });
    await waitFor(() => expect(dialog.textContent).toContain("Create a theme file"));
    expect(dialog.textContent).toContain("Settings/themes/built-in-themes");
    expect(dialog.textContent).toContain("accent-color-selection");
    expect(dialog.querySelector("table")).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Close Custom themes guide" }));
    await waitFor(() => expect(document.querySelector(".confirm-backdrop").hidden).toBe(true));
  });
});
