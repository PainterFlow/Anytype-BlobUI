(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const doc = root.document;
  const dataUrlPattern = /^data:image\/(png|jpeg|gif|webp);base64,((?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?)$/i;
  const base64Pattern = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

  function hasImageSignature(mime, encoded) {
    if (typeof root.atob !== "function" || encoded.length < 16) return false;
    let prefix;
    try { prefix = root.atob(encoded.slice(0, 16)); } catch { return false; }
    if (mime === "png") return prefix.charCodeAt(0) === 0x89 && prefix.slice(1, 4) === "PNG";
    if (mime === "jpeg") return prefix.charCodeAt(0) === 0xff && prefix.charCodeAt(1) === 0xd8 && prefix.charCodeAt(2) === 0xff;
    if (mime === "gif") return prefix.slice(0, 6) === "GIF87a" || prefix.slice(0, 6) === "GIF89a";
    if (mime === "webp") return prefix.slice(0, 4) === "RIFF" && prefix.slice(8, 12) === "WEBP";
    return false;
  }

  function safeImageDataUrl(value) {
    const match = dataUrlPattern.exec(String(value || ""));
    if (!match || !match[2] || match[2].length % 4 !== 0 || !base64Pattern.test(match[2])) return "";
    const mime = match[1].toLowerCase();
    return hasImageSignature(mime, match[2]) ? match[0] : "";
  }

  const layer = doc.createElement("div");
  layer.className = "markdown-image-viewer-layer";
  const backdrop = doc.createElement("div");
  backdrop.className = "confirm-backdrop markdown-image-viewer-backdrop";
  backdrop.hidden = true;
  const dialog = doc.createElement("section");
  dialog.className = "confirm-dialog markdown-image-viewer-dialog";
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", "Image preview");
  dialog.tabIndex = -1;

  const stage = doc.createElement("div");
  stage.className = "markdown-image-viewer-stage";
  stage.setAttribute("role", "region");
  stage.setAttribute("aria-label", "Image preview; scroll to inspect the actual-size image");
  stage.tabIndex = 0;
  const preview = doc.createElement("img");
  preview.className = "markdown-image-viewer-image";
  preview.alt = "";
  preview.draggable = false;
  preview.tabIndex = 0;
  preview.setAttribute("role", "button");
  stage.append(preview);
  dialog.append(stage);
  backdrop.append(dialog);
  layer.append(backdrop);
  doc.body.append(layer);

  const inertTargets = Array.from(doc.body.children).filter(element => element !== layer);
  let sourceImage = null;
  let actualSize = false;
  const modal = BlobUI.createModalController({
    backdrop,
    dialog,
    initialFocus: preview,
    fallbackFocus: () => {
      const documentation = doc.querySelector("#documentation:not([hidden]) #documentation-content");
      if (documentation) return documentation;
      const releaseNotes = doc.querySelector("#release-notes-backdrop:not([hidden]) #release-notes-content");
      return releaseNotes || doc.getElementById("help-button");
    },
    inertTargets,
    onClose() {
      sourceImage = null;
      preview.removeAttribute("src");
      preview.alt = "";
      setActualSize(false);
    },
  });

  function setActualSize(value) {
    actualSize = value === true;
    stage.classList.toggle("is-actual-size", actualSize);
    preview.setAttribute("aria-pressed", String(actualSize));
    preview.setAttribute("aria-label", actualSize ? "Fit image to window" : "Show image at actual size");
    stage.scrollTop = 0;
    stage.scrollLeft = 0;
  }

  function open(image) {
    if (modal.isOpen() || !image.isConnected) return;
    const src = safeImageDataUrl(image.getAttribute("src"));
    if (!src) return;

    sourceImage = image;
    const alt = String(image.getAttribute("alt") || "").trim();
    preview.alt = alt;
    dialog.setAttribute("aria-label", alt || "Image preview");
    setActualSize(false);
    preview.removeAttribute("src");
    preview.onerror = () => { if (sourceImage) modal.close(); };
    preview.src = src;
    try { image.focus({ preventScroll: true }); } catch { image.focus(); }
    modal.open();
  }

  function handleClick(event) {
    const image = event.target?.closest?.(".markdown-content img.markdown-image-view-trigger");
    if (!image || image.closest("a") || !doc.contains(image)) return;
    event.preventDefault();
    open(image);
  }

  function handleKeydown(event) {
    if (event.key !== "Enter" && event.key !== " ") return;
    const image = event.target?.closest?.(".markdown-content img.markdown-image-view-trigger");
    if (!image || image.closest("a") || !doc.contains(image)) return;
    event.preventDefault();
    open(image);
  }

  doc.addEventListener("click", handleClick);
  doc.addEventListener("keydown", handleKeydown);
  preview.addEventListener("click", () => setActualSize(!actualSize));
  preview.addEventListener("keydown", event => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    setActualSize(!actualSize);
  });
  dialog.addEventListener("pointerdown", event => {
    if (event.target === dialog || event.target === stage) modal.close();
  });
  root.addEventListener("unload", () => {
    modal.destroy();
    doc.removeEventListener("click", handleClick);
    doc.removeEventListener("keydown", handleKeydown);
    layer.remove();
  }, { once: true });

  BlobUI.MarkdownImageViewer = Object.freeze({
    mount(container) {
      if (!container?.querySelectorAll) return;
      for (const image of container.querySelectorAll("img")) {
        if (image.closest("a")) continue;
        if (!safeImageDataUrl(image.getAttribute("src"))) continue;
        image.classList.add("markdown-image-view-trigger");
        image.setAttribute("role", "button");
        image.setAttribute("tabindex", "0");
        image.setAttribute("aria-label", `Expand image: ${image.alt || "Documentation image"}`);
        image.title = image.title || "Expand image";
        image.draggable = false;
      }
    },
    destroyTree(container) {
      if (sourceImage && container?.contains?.(sourceImage)) modal.close({ restoreFocus: false });
    },
    isOpen: () => modal.isOpen(),
  });
})(window);
