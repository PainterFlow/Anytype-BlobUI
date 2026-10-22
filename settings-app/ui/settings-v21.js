(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const doc = root.document;
  let enabled = false, controller, api, preferenceTabs, preferenceTabController;
  const panels = [], navButtons = [];
  const preferenceLabels = ["Accent", "Kanban", "Calendar", "Typography", "Space banner"];
  const sectionIcons = {
    Accent: '<circle cx="10" cy="10" r="7"/><circle cx="10" cy="10" r="3" fill="currentColor"/>',
    Kanban: '<rect x="2" y="3" width="4" height="14" rx="1"/><rect x="8" y="3" width="4" height="9" rx="1"/><rect x="14" y="3" width="4" height="12" rx="1"/>',
    Calendar: '<rect x="3" y="4" width="14" height="13" rx="2"/><path d="M6 2v4m8-4v4M3 8h14M6 11h2m4 0h2m-8 3h2"/>',
    Typography: '<path d="M3 4h14M10 4v13m-4 0h8M3 4v3m14-3v3"/>',
    "Space banner": '<rect x="2" y="4" width="16" height="12" rx="2"/><circle cx="7" cy="8" r="1"/><path d="m3 15 5-5 3 3 3-4 4 6"/>',
    "API key": '<circle cx="7" cy="7" r="4"/><path d="m10 10 7 7m-3-3 2-2m-4 0 2-2"/>',
    "Ap Settings": '<path d="M3 5h14M3 10h14M3 15h14"/><circle cx="7" cy="5" r="2" fill="var(--sidebar)"/><circle cx="13" cy="10" r="2" fill="var(--sidebar)"/><circle cx="8" cy="15" r="2" fill="var(--sidebar)"/>',
    Experiments: '<path d="M7 2h6M8 2v6l-5 8a1 1 0 0 0 1 2h12a1 1 0 0 0 1-2l-5-8V2M6 12h8"/>',
  };
  function make(tag, className, text) {
    const node = doc.createElement(tag);
    node.className = className || "";
    if (text) node.textContent = text;
    return node;
  }
  function icon(label) {
    const wrapper = make("span", "documentation-item-icon");
    const svg = doc.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 20 20"); svg.setAttribute("aria-hidden", "true");
    svg.innerHTML = sectionIcons[label]; wrapper.append(svg); return wrapper;
  }
  function selectPreference(index) { preferenceTabController.select(index); }
  function navigate(page, target, tab) {
    api.showSettingsPage(page);
    navButtons.forEach(button => controller.setExpanded(button, button.dataset.settingsPage === page));
    if (page === "preferences") selectPreference(tab);
    else api.selectAdvancedTab(tab);
    target.scrollIntoView({ block: "start", behavior: "instant" });
    const heading = target.querySelector("h2");
    if (heading) { heading.tabIndex = -1; heading.focus({ preventScroll: true }); }
  }
  function init(options) {
    api = options;
    const preferences = doc.getElementById("preferences-page");
    const bar = make("div", "advanced-tabs preference-tabs"); bar.setAttribute("role", "tablist"); bar.setAttribute("aria-label", "Preferences");
    preferenceTabs = [...preferences.children].filter(node => node.matches("section.settings-group")).map((panel, index) => {
      panels.push(panel); panel.id = `preference-panel-${index}`;
      const button = make("button", "", preferenceLabels[index]); button.type = "button";
      button.id = `preference-tab-${index}`; button.setAttribute("role", "tab"); button.setAttribute("aria-controls", panel.id);
      bar.append(button); return button;
    });
    preferences.prepend(bar); bar.hidden = true;
    preferenceTabController = UI.Tabs.attach({ container: bar, buttons: preferenceTabs, panels });
    const navigation = doc.getElementById("preferences-nav").parentElement;
    // Use the Docs hierarchy and shared disclosure controller without its chevron.
    for (const page of ["preferences", "advanced"]) {
      const button = doc.getElementById(`${page}-nav`); navButtons.push(button);
      const links = make("div", "documentation-children v21-section-links"); links.id = `${page}-section-links`; links.dataset.disclosurePanel = "";
      links.hidden = true; button.after(links);
      const inner = make("div", "documentation-children-inner"); inner.dataset.disclosureContent = ""; links.append(inner);
      button.dataset.disclosureToggle = ""; button.setAttribute("aria-controls", links.id); button.setAttribute("aria-expanded", "false");
      const groups = page === "preferences" ? panels : [doc.getElementById("advanced-api-panel"), doc.getElementById("advanced-app-panel"), doc.getElementById("advanced-experiments-panel")];
      const labels = page === "preferences" ? preferenceLabels : ["API key", "Ap Settings", "Experiments"];
      groups.forEach((target, tab) => {
        const label = labels[tab];
        const link = make("button", "documentation-item documentation-child v21-section-link"); link.type = "button";
        link.append(icon(label), make("span", "documentation-item-label", label));
        link.addEventListener("click", () => navigate(page, target, tab)); inner.append(link);
      });
    }
    controller = UI.Disclosure.attach(navigation, { onToggle({ toggle, expanded }) {
      if (!enabled) { if (expanded) controller.setExpanded(toggle, false); return; }
      if (expanded) navButtons.forEach(button => { if (button !== toggle) controller.setExpanded(button, false); });
    } });
    doc.getElementById("accessibility-nav").addEventListener("click", () => navButtons.forEach(button => controller.setExpanded(button, false)));
    setEnabled(false);
  }
  function setEnabled(value) {
    enabled = value === true;
    if (!api) return;
    const active = doc.activeElement;
    doc.documentElement.dataset.settingsV21 = String(enabled);
    doc.querySelector(".preference-tabs").hidden = !enabled;
    panels.forEach((panel, index) => {
      if (enabled) { panel.setAttribute("role", "tabpanel"); panel.setAttribute("aria-labelledby", preferenceTabs[index].id); }
      else { panel.removeAttribute("role"); panel.setAttribute("aria-labelledby", panel.querySelector("h2").id); }
    });
    preferenceTabController.setEnabled(enabled);
    if (!enabled) {
      navButtons.forEach(button => controller.setExpanded(button, false));
      if (active?.closest(".preference-tabs,.v21-section-links")) doc.querySelector('.nav-item.active').focus();
    }
  }
  UI.SettingsV21 = { init, setEnabled, refreshTabs() { preferenceTabController?.refresh(); } };
})(window);
