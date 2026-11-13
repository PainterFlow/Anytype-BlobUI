import React, { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ExpandableSearch } from "./search.jsx";
import { DialogSwitch } from "./settings-controls.jsx";
import { Tabs } from "./tabs.jsx";

describe("Settings controls", () => {
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

  it("portals help popovers out of clipped settings groups", () => {
    render(<div className="settings-list" style={{ overflow: "hidden" }}>
      <DialogSwitch id="remote-toggle" dialogId="remote-dialog" label="Remote access" help="This text stays above the row." checked={false} onChange={() => {}} onOpen={() => {}} />
    </div>);
    const trigger = screen.getByRole("button", { name: "Remote access information" });
    fireEvent.focus(trigger);
    const popover = document.getElementById(trigger.getAttribute("aria-describedby"));
    expect(popover.parentElement).toBe(document.body);
    expect(popover.dataset.open).toBe("true");
    expect(popover.textContent).toBe("This text stays above the row.");
  });
});
