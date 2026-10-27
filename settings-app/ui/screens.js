(function (root) {
  "use strict";
  const UI = root.BlobUI;
  const instances = new Map();
  function mount(container = root.document) {
    for (const marker of container.querySelectorAll('template[data-ui-mount]')) {
      mount(marker.content);
      const key = marker.dataset.uiMount;
      const create = UI.ScreenDefinitions[key];
      if (!create) throw new Error("Unknown UI component " + key);
      const instance = create([...marker.content.childNodes]);
      marker.replaceWith(instance.element);
      instances.set(key, instance);
    }
  }
  UI.Screens = { mount, get: id => instances.get(id), destroy() {
    [...instances.values()].reverse().forEach(instance => instance.destroy());
    instances.clear();
  } };
  mount();
  root.addEventListener("unload", () => { UI.Components.destroyTree(root.document.body); instances.clear(); });
})(window);
