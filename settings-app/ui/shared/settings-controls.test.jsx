import React, { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CopyTextField } from "./copy-text-field.jsx";
import { HelpTooltip } from "./help-tooltip.jsx";
import { ExpandableSearch } from "./search.jsx";
import { ReadOnlyTextInput } from "./settings-controls.jsx";
import { Tabs } from "./tabs.jsx";

describe("Settings controls", () => {
  it("renders a reusable non-editable text input", () => {
    render(<ReadOnlyTextInput inputAttributes={{ id: "locked-value", "aria-label": "Locked value", defaultValue: "fixed" }} />);
    const input = screen.getByRole("textbox", { name: "Locked value" });
    expect(input.value).toBe("fixed");
    expect(input.readOnly).toBe(true);
    expect(input.classList.contains("read-only-text-input")).toBe(true);
  });

  it("renders a read-only copy field with the shared Logs icons", () => {
    const onCopy = vi.fn();
    const { rerender } = render(<CopyTextField
      value="C:/Anytype"
      inputAttributes={{ id: "anytype-folder", "aria-label": "Anytype Folder" }}
      copyLabel="Copy folder path"
      copiedLabel="Folder path copied"
      onCopy={onCopy}
    />);

    const input = screen.getByRole("textbox", { name: "Anytype Folder" });
    expect(input.value).toBe("C:/Anytype");
    expect(input.readOnly).toBe(true);

    const copyButton = screen.getByRole("button", { name: "Copy folder path" });
    expect(copyButton.querySelector(".copy-icon")).not.toBeNull();
    expect(copyButton.querySelector(".copy-success-icon")).not.toBeNull();
    fireEvent.click(copyButton);
    expect(onCopy).toHaveBeenCalledTimes(1);

    rerender(<CopyTextField
      value="C:/Anytype"
      inputAttributes={{ id: "anytype-folder", "aria-label": "Anytype Folder" }}
      copied
      copyLabel="Copy folder path"
      copiedLabel="Folder path copied"
      onCopy={onCopy}
    />);
    expect(screen.getByRole("button", { name: "Folder path copied" }).parentElement.dataset.copied).toBe("true");
  });

  it("keeps the same input and its draft when switching tabs with the keyboard", () => {
    function Example() {
      const [selected, setSelected] = useState(0);
      return <Tabs label="Preferences" selected={selected} onSelect={setSelected} items={[
        { tabId: "type-tab", panelId: "type-panel", title: "Typography" },
        { tabId: "accent-tab", panelId: "accent-panel", title: "Accent" },
      ]}>
        <input aria-label="Font family" defaultValue="Inter" />
        <p>Accent color</p>
      </Tabs>;
    }
    render(<Example />);
    const input = screen.getByRole("textbox", { name: "Font family" });
    fireEvent.change(input, { target: { value: "Segoe UI" } });
    fireEvent.keyDown(screen.getByRole("tab", { name: "Typography" }), { key: "ArrowRight" });
    const accent = screen.getByRole("tab", { name: "Accent", selected: true });
    expect(document.activeElement).toBe(accent);
    expect(screen.queryByRole("textbox", { name: "Font family" })).toBeNull();
    fireEvent.keyDown(accent, { key: "Home" });
    expect(screen.getByRole("textbox", { name: "Font family" })).toBe(input);
    expect(input.value).toBe("Segoe UI");
  });

  it("clears and closes a search on Escape and returns focus to its button", () => {
    function Example() {
      const [value, setValue] = useState("");
      return <ExpandableSearch id="view-search" label="Search views" value={value} onChange={setValue} />;
    }
    render(<Example />);
    const button = screen.getByRole("button", { name: "Search views" });
    expect(button.querySelector("svg path")).not.toBeNull();
    const input = screen.getByRole("searchbox", { name: "Search views" });
    fireEvent.click(button);
    expect(document.activeElement).toBe(input);
    fireEvent.change(input, { target: { value: "Tasks" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input.value).toBe("");
    expect(input.tabIndex).toBe(-1);
    expect(document.activeElement).toBe(button);
    expect(button.getAttribute("aria-expanded")).toBe("false");
  });

  it("portals Anyblob help popovers out of clipped settings groups", () => {
    render(<div className="settings-list" style={{ overflow: "hidden" }}>
      <HelpTooltip label="Anyblob help" text="This guide explains how to set up Anyblob." />
    </div>);
    const trigger = screen.getByRole("button", { name: "Anyblob help" });
    fireEvent.focus(trigger);
    const popover = document.getElementById(trigger.getAttribute("aria-describedby"));
    expect(popover.parentElement).toBe(document.body);
    expect(popover.dataset.open).toBe("true");
    expect(popover.textContent).toBe("This guide explains how to set up Anyblob.");
  });

  it("keeps question mark help on the shared focus and hover tooltip behavior", () => {
    render(<HelpTooltip label="Space permission help" text="Choose the spaces this integration can access." />);
    const trigger = screen.getByRole("button", { name: "Space permission help" });
    expect(trigger.querySelector("svg circle")).not.toBeNull();
    expect(trigger.querySelector("svg path")).not.toBeNull();
    const popover = document.getElementById(trigger.getAttribute("aria-describedby"));
    expect(popover.getAttribute("role")).toBe("tooltip");

    fireEvent.focus(trigger);
    expect(popover.dataset.open).toBe("true");
    fireEvent.blur(trigger);
    expect(popover.dataset.open).toBe("false");
    fireEvent.pointerEnter(trigger.parentElement);
    expect(popover.dataset.open).toBe("true");
    fireEvent.pointerLeave(trigger.parentElement);
    expect(popover.dataset.open).toBe("false");
  });

  it("closes a tooltip when its question mark is clicked", () => {
    render(<HelpTooltip label="Space permission help" text="Choose the spaces this integration can access." />);
    const trigger = screen.getByRole("button", { name: "Space permission help" });
    const popover = document.getElementById(trigger.getAttribute("aria-describedby"));

    fireEvent.pointerEnter(trigger.parentElement);
    fireEvent.focus(trigger);
    expect(popover.dataset.open).toBe("true");
    fireEvent.click(trigger);
    expect(popover.dataset.open).toBe("false");
  });

  it("keeps action help buttons without tooltip text", () => {
    const onClick = vi.fn();
    render(<HelpTooltip label="Anyblob help" onClick={onClick} dialogId="anyblob-guide-dialog" />);
    const trigger = screen.getByRole("button", { name: "Anyblob help" });

    expect(trigger.getAttribute("aria-describedby")).toBeNull();
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(trigger.getAttribute("aria-haspopup")).toBe("dialog");
    fireEvent.click(trigger);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
