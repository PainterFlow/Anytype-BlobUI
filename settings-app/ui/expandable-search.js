(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  UI.ExpandableSearch = { attach({ container, button, input }) {
    function setExpanded(expanded, focus = false) { container.dataset.expanded = String(expanded); button.setAttribute('aria-expanded', String(expanded)); input.tabIndex = expanded ? 0 : -1; if (focus) (expanded ? input : button).focus(); }
    const click = () => setExpanded(true, true);
    const keydown = event => { if (event.key === 'Escape') { event.preventDefault(); input.value = ''; input.dispatchEvent(new Event('input', { bubbles: true })); setExpanded(false, true); } };
    const blur = event => { if (!container.contains(event.relatedTarget) && !input.value) setExpanded(false); };
    button.addEventListener('click', click); input.addEventListener('keydown', keydown); container.addEventListener('focusout', blur);
    setExpanded(Boolean(input.value));
    return { setExpanded, destroy() { button.removeEventListener('click', click); input.removeEventListener('keydown', keydown); container.removeEventListener('focusout', blur); } };
  } };
})(window);
