(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const doc = root.document;
  const owners = new WeakMap();
  function destroyTree(node) {
    for (const child of node.querySelectorAll("*")) owners.get(child)?.destroy();
    owners.get(node)?.destroy();
  }

  function element(tag, attributes = {}, children = []) {
    const node = doc.createElement(tag);
    Object.entries(attributes).forEach(([name, value]) => {
      if (value !== false && value != null) node.setAttribute(name, value === true ? "" : String(value));
    });
    node.append(...children.filter(child => child != null));
    return node;
  }

  // A component owns its listeners and controllers, never the application's draft.
  function component(node, refs = {}, render = () => {}, cleanup = () => {}) {
    let destroyed = false;
    const listeners = [];
    const result = {
      element: node, ...refs,
      update(props = {}) { if (!destroyed) render(props); return result; },
      listen(target, event, action, options) {
        if (destroyed) return result;
        target.addEventListener(event, action, options);
        listeners.push(() => target.removeEventListener(event, action, options));
        return result;
      },
      destroy() {
        if (destroyed) return;
        destroyed = true;
        for (const child of node.querySelectorAll("*")) owners.get(child)?.destroy();
        listeners.splice(0).forEach(remove => remove());
        cleanup();
        if (owners.get(node) === result) owners.delete(node);
      },
    };
    owners.set(node, result);
    return result;
  }

  function Button(props = {}) {
    const node = element("button", { type: "button", ...props.attributes }, props.content || []);
    let onClick = props.onClick;
    let listening = false;
    const result = component(node, {}, next => {
      if ("label" in next) node.textContent = next.label;
      if ("disabled" in next) node.disabled = next.disabled;
      if ("hidden" in next) node.hidden = next.hidden;
      if ("onClick" in next) onClick = next.onClick;
      if ("accessibleLabel" in next) node.setAttribute("aria-label", next.accessibleLabel);
      if (!listening && onClick) {
        result.listen(node, "click", event => onClick?.(event));
        listening = true;
      }
    });
    return result.update(props);
  }

  function PropertyPickerShell({ label, onClick, showValue = false, accessibleLabel, menuLabel = label } = {}) {
    const trigger = Button({ label, onClick, attributes: { class: "formula-insert-property", "aria-haspopup": "listbox", "aria-expanded": "false", "aria-label": accessibleLabel } }).element;
    if (showValue) { const value = element("span", { "data-pick-value": "" }, [label]); trigger.replaceChildren(value); }
    const menu = element("div", { class: "kanban-select-menu", role: "listbox", "aria-label": menuLabel, hidden: true });
    const root = element("div", { class: "kanban-select formula-property-picker" }, [trigger, menu]);
    return component(root, { trigger, menu });
  }

  function DeleteIcon() {
    const ns = "http://www.w3.org/2000/svg", icon = doc.createElementNS(ns, "svg");
    icon.setAttribute("viewBox", "0 0 20 20"); icon.setAttribute("aria-hidden", "true");
    for (const [d, cap] of [["M8 4.5h9.5v11H8L3.5 10 8 4.5Z", "round"], ["m11 8 4 4m0-4-4 4", "round"]]) {
      const path = doc.createElementNS(ns, "path"); path.setAttribute("d", d); path.setAttribute("fill", "none"); path.setAttribute("stroke", "currentColor"); path.setAttribute("stroke-width", "1.5"); path.setAttribute("stroke-linecap", cap); path.setAttribute("stroke-linejoin", "round"); icon.append(path);
    }
    return icon;
  }

  function IconButton(props = {}) {
    return Button({ ...props, attributes: { class: "neutral-icon-button", ...props.attributes } });
  }

  function OnboardingChoice({ attributes, title, recommendedLabel, features }) {
    const copy = [element("span", { class: "onboarding-option-title" }, [title])];
    const list = element("span", { class: "onboarding-features", role: "list" });
    for (const feature of features) {
      const icon = doc.createElementNS("http://www.w3.org/2000/svg", "svg");
      icon.setAttribute("class", "feature-mark " + (feature.included ? "available" : "unavailable"));
      icon.setAttribute("viewBox", feature.included ? "0 0 20 20" : "0 0 12 12");
      icon.setAttribute("aria-hidden", "true");
      const path = doc.createElementNS(icon.namespaceURI, "path");
      path.setAttribute("d", feature.included ? "m4 10 4 4 8-8" : "m2 2 8 8m0-8-8 8");
      icon.append(path);
      list.append(element("span", { class: "onboarding-feature", role: "listitem" }, [
        element("span", {}, [feature.label]), icon,
        element("span", { class: "sr-only" }, [feature.included ? "Included" : "Not included"]),
      ]));
    }
    copy.push(list);
    const content = recommendedLabel ? [
      element("span", { class: "recommended-label" }, [recommendedLabel]),
      element("span", { class: "recommended-panel" }, copy),
    ] : copy;
    return Button({ attributes, content });
  }

  function ToggleSetting(props) {
    const label = element("span", {}, [props.label]);
    const input = element("input", { id: props.id, type: "checkbox", role: "switch", ...props.inputAttributes });
    const track = element("span", { class: "switch-track", "aria-hidden": "true" });
    const node = element("label", { class: props.className || "native-accent-control settings-row", for: props.id }, [label, input, track]);
    if (props.description) node.append(element("span", { class: "sr-only", id: props.description.id }, [props.description.text]));
    let onChange = props.onChange;
    let listening = false;
    const result = component(node, { input }, next => {
      if ("label" in next) label.textContent = next.label;
      if ("checked" in next) input.checked = next.checked;
      if ("disabled" in next) input.disabled = next.disabled;
      if ("onChange" in next) onChange = next.onChange;
      if (!listening && onChange) {
        result.listen(input, "change", event => onChange?.(input.checked, event));
        listening = true;
      }
    });
    return result.update(props);
  }

  function TextField(props) {
    const input = element("input", props.inputAttributes);
    const label = element(props.labelTag || "label", props.labelAttributes || { for: props.inputAttributes.id }, props.labelContent || [props.label]);
    const node = element(props.tag || "div", { class: "input-field", ...props.attributes }, [label]);
    let field = null, button = null, visibility = null;
    if (props.visibility) {
      field = element("div", { id: input.id + "-field", class: "visibility-field", "data-filled": "false" }, [input]);
      button = Button({ attributes: props.visibility.buttonAttributes, content: [root.VisibilityIcon.create()] });
      field.append(button.element);
      node.append(field);
    } else node.append(input);
    let onInput = props.onInput;
    let listening = false;
    const result = component(node, { input, field, button: button?.element,
      bindVisibility(options) {
        if (!field) throw new TypeError("This field has no visibility toggle.");
        visibility?.destroy();
        visibility = root.VisibilityField.bind({ ...options, input, field, button: button.element });
        return visibility;
      },
    }, next => {
      if ("value" in next && input.value !== String(next.value)) {
        visibility?.invalidate();
        input.value = next.value;
      }
      if ("disabled" in next) {
        input.disabled = next.disabled;
        if (button) button.element.disabled = next.disabled;
      }
      if ("onInput" in next) onInput = next.onInput;
      if ("label" in next) label.textContent = next.label;
      visibility?.sync();
      if (!listening && onInput) {
        result.listen(input, "input", event => onInput?.(input.value, event));
        listening = true;
      }
    }, () => { visibility?.destroy(); button?.destroy(); });
    return result.update(props);
  }

  function Heading(props) {
    const title = element("h2", { id: props.id }, [props.title]);
    const description = props.description == null ? null : element("p", {}, [props.description]);
    return component(element("div", { class: "section-heading" }, [title, description]), { title, description }, next => {
      if ("title" in next) title.textContent = next.title;
      if (description && "description" in next) description.textContent = next.description;
    });
  }

  function Container({ tag = "div", attributes = {}, content = [] } = {}) {
    return component(element(tag, attributes, content));
  }
  function Group(props = {}) { return Container({ ...props, attributes: { class: "settings-list", ...props.attributes } }); }
  function Section(props = {}) { return Container({ tag: "section", ...props, attributes: { class: "settings-group", ...props.attributes } }); }
  function SettingsRow(props = {}) {
    const content = [...(props.content || [])];
    if (props.label != null) content.unshift(element("div", { class: "kanban-setting-copy" }, [
      element("span", { class: "kanban-setting-title" }, [props.label]),
      props.description ? element("span", props.description.attributes, [props.description.text]) : null,
    ]));
    return Container({ ...props, content, attributes: { class: "settings-row", ...props.attributes } });
  }

  function DialogShell(props) {
    const dialog = element("section", { class: "confirm-dialog", role: "dialog", "aria-modal": "true", tabindex: "-1", ...props.dialogAttributes }, props.content);
    const backdrop = element("div", { class: "confirm-backdrop", hidden: true, ...props.attributes }, [dialog]);
    let controller;
    return component(backdrop, { dialog,
      attach(options) {
        controller?.destroy();
        controller = UI.createModalController({ ...options, backdrop, dialog });
        return controller;
      },
    }, next => { if ("accessibleLabel" in next) dialog.setAttribute("aria-label", next.accessibleLabel); }, () => controller?.destroy());
  }

  // Existing nodes can be attached without replacing them or their focused controls.
  function attach(node, controller, render = () => {}) {
    const result = component(node, {}, render, () => controller.destroy?.());
    Object.entries(controller).forEach(([name, value]) => { if (name !== "destroy") result[name] = value; });
    return result;
  }
  function Tabs(options) {
    const controller = UI.Tabs.attach(options);
    return attach(options.container, controller, next => {
      if ("enabled" in next) controller.setEnabled(next.enabled);
      if ("selectedIndex" in next) controller.select(next.selectedIndex);
    });
  }
  function Disclosure(options) { return attach(options.container, UI.Disclosure.attach(options.container, options)); }
  function Picker(options) {
    const controller = UI.PickControl.attach(options);
    return attach(options.root, controller, next => {
      if ("options" in next) controller.setOptions(next.options);
      if ("value" in next) controller.setValue(next.value);
    });
  }
  function Search(options) {
    const controller = UI.ExpandableSearch.attach(options);
    return attach(options.container, controller, next => {
      if ("expanded" in next) controller.setExpanded(next.expanded, next.focus === true);
    });
  }
  function Modal(options) {
    const shell = UI.Screens?.get(options.backdrop.id);
    if (!shell) return attach(options.backdrop, UI.createModalController(options));
    return { ...shell.attach(options), element: shell.element, update: shell.update, destroy: shell.destroy };
  }
  function Visibility(options) {
    const field = UI.Screens?.get(options.input.id);
    if (!field) return attach(options.field, root.VisibilityField.bind(options));
    return { ...field.bindVisibility(options), element: field.element, update: field.update, destroy: field.destroy };
  }

  function NavigationItem({ label, icon, attributes = {}, onClick }) {
    return Button({ attributes: { class: "documentation-item", ...attributes },
      content: [icon, element("span", { class: "documentation-item-label" }, [label])].filter(Boolean), onClick });
  }

  UI.Components = { element, component, destroyTree, Button, IconButton, OnboardingChoice, ToggleSetting, TextField, Heading,
    PropertyPickerShell, DeleteIcon, Container, Section, Group, SettingsRow, DialogShell, Tabs, Disclosure, Picker, Search, Modal, Visibility, NavigationItem };
})(window);
