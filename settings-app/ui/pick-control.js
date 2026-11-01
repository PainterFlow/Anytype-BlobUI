(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  UI.PickControl = { attach({ root: container, menu: suppliedMenu, options, value, onChange, multiple = false, closeOnSelect = true, focusOnOpen = true, restoreFocusOnSelect = true, emptyMessage = "" }) {
    const trigger = container.querySelector('[aria-haspopup="listbox"]');
    const menu = suppliedMenu || container.querySelector('[role="listbox"]');
    const label = trigger.querySelector('[data-pick-value]') || trigger.querySelector('span:not(.sr-only)');
    const events = [], listen = (node, type, action) => { node.addEventListener(type, action); events.push(() => node.removeEventListener(type, action)); };
    let timer, frame, selectedValues = [];
    if (multiple) menu.setAttribute('aria-multiselectable', 'true');
    const reduced = () => document.documentElement.dataset.reducedMotion === "true" || root.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    const items = () => Array.from(menu.querySelectorAll('[role="option"]'));
    function cancel() { root.clearTimeout(timer); root.cancelAnimationFrame?.(frame); }
    function close(restoreFocus = false) {
      const focusInside = menu.contains(document.activeElement);
      if (trigger.getAttribute('aria-expanded') !== 'true') { if (restoreFocus || focusInside) (trigger.disabled ? document.getElementById('page-title') : trigger)?.focus(); return; }
      trigger.setAttribute('aria-expanded', 'false'); menu.inert = true; cancel();
      menu.dataset.state = reduced() ? 'closed' : 'closing';
      if (reduced()) menu.hidden = true;
      else timer = root.setTimeout(() => { menu.hidden = true; menu.dataset.state = 'closed'; }, 100);
      if (restoreFocus || focusInside) (trigger.disabled ? document.getElementById('page-title') : trigger)?.focus();
    }
    function setValue(next) {
      if (multiple) {
        selectedValues = Array.isArray(next) ? [...new Set(next.map(String))] : [];
        items().forEach(item => item.setAttribute('aria-selected', String(selectedValues.includes(item.dataset.value))));
        return selectedValues.slice();
      }
      const option = items().find(item => item.dataset.value === String(next));
      trigger.value = String(next);
      if (label && option) label.textContent = option.querySelector('.kanban-option-label')?.textContent || option.textContent;
      items().forEach(item => item.setAttribute('aria-selected', String(item === option)));
      return trigger.value;
    }
    function focus(index) {
      const list = items(); if (!list.length) return;
      const item = list[(index + list.length) % list.length];
      item.focus({preventScroll:true});
      const bounds=menu.getBoundingClientRect(), rect=item.getBoundingClientRect();
      if (rect.top < bounds.top) menu.scrollTop -= bounds.top-rect.top;
      else if (rect.bottom > bounds.bottom) menu.scrollTop += rect.bottom-bounds.bottom;
    }
    function open() {
      if (trigger.disabled) return;
      cancel(); menu.hidden = false; menu.inert = false; trigger.setAttribute('aria-expanded', 'true');
      menu.dataset.state = reduced() ? 'open' : 'opening';
      if (!reduced()) frame = root.requestAnimationFrame(() => { menu.dataset.state = 'open'; });
      if (focusOnOpen) focus(Math.max(0, items().findIndex(item => item.dataset.value === trigger.value)));
    }
    function setOptions(next) {
      close(); menu.replaceChildren();
      if (!next.length && emptyMessage) {
        const empty = document.createElement('div');
        empty.className = 'kanban-select-empty';
        empty.setAttribute('role', 'status');
        empty.textContent = emptyMessage;
        menu.append(empty);
      }
      next.forEach(item => {
        const option = document.createElement('button');
        option.type = 'button';
        option.className = 'kanban-select-option';
        option.setAttribute('role', 'option');
        option.setAttribute('aria-label', item.label);
        option.tabIndex = -1;
        option.dataset.value = item.value;
        const label = document.createElement('span');
        label.className = 'kanban-option-label';
        label.textContent = item.label;
        if (item.description) {
          const copy = document.createElement('span');
          copy.className = 'kanban-option-copy';
          copy.append(label);
          const description = document.createElement('span');
          description.className = 'kanban-select-option-description';
          description.textContent = item.description;
          copy.append(description);
          option.append(copy);
        } else option.append(label);
        const check = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        check.setAttribute("viewBox", "0 0 16 16"); check.setAttribute("aria-hidden", "true");
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute("d", "m3.5 8 3 3 6-6"); check.append(path); option.append(check);
        menu.append(option);
      });
      setValue(multiple ? selectedValues : trigger.value);
    }
    listen(trigger, 'click', () => trigger.getAttribute('aria-expanded') === 'true' ? close() : open());
    listen(trigger, 'keydown', event => { if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) { event.preventDefault(); if (trigger.getAttribute('aria-expanded') !== 'true') open(); else { const selected = items().findIndex(item => item.dataset.value === trigger.value); focus(Math.max(0, selected - (event.key === 'ArrowUp' ? 1 : 0))); } } else if (event.key === 'Escape') close(true); });
    listen(menu, 'click', event => { const item = event.target.closest('[role="option"]'); if (!item || !menu.contains(item)) return;
      const next = multiple ? selectedValues.includes(item.dataset.value) ? selectedValues.filter(value => value !== item.dataset.value) : [...selectedValues,item.dataset.value] : item.dataset.value;
      const result = setValue(next); trigger.dispatchEvent(new Event('change', { bubbles: true })); onChange?.(result); if (closeOnSelect) close(restoreFocusOnSelect); else if (item.isConnected && focusOnOpen) item.focus({preventScroll:true}); });
    listen(menu, 'keydown', event => {
      const index = items().indexOf(document.activeElement);
      if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) { event.preventDefault(); focus(event.key === 'Home' ? 0 : event.key === 'End' ? items().length - 1 : index + (event.key === 'ArrowDown' ? 1 : -1)); }
      else if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); document.activeElement?.click(); }
      else if (event.key === 'Escape') { event.preventDefault(); close(true); }
      else if (event.key === 'Tab') close(true);
    });
    listen(document, 'pointerdown', event => { if (!container.contains(event.target) && !menu.contains(event.target)) close(); });
    listen(document, 'focusin', event => { if (!container.contains(event.target) && !menu.contains(event.target)) close(); });
    if (options) setOptions(options);
    setValue(value === undefined ? trigger.value : value);
    return { setValue, setOptions, open, close, destroy() { cancel(); events.forEach(remove => remove()); } };
  } };
})(window);
