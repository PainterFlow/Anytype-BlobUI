(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const nav = root.document.getElementById("documentation-navigation");
  const content = root.document.getElementById("documentation-content");
  const article = root.document.getElementById("documentation-article");
  const errorPanel = root.document.getElementById("documentation-load-error");
  const retryButton = root.document.getElementById("documentation-retry");
  const categoryIconPaths = {
    "getting-started": '<path d="M3 4.5h14v11H3zM6 8h8M6 11h5"/>',
    getting_started: '<path d="M3 4.5h14v11H3zM6 8h8M6 11h5"/>',
    features: '<path d="m10 2 1.8 5.2L17 9l-5.2 1.8L10 16l-1.8-5.2L3 9l5.2-1.8zM16 13l1 2.5 2.5 1-2.5 1-1 2.5-1-2.5-2.5-1 2.5-1z"/>',
    reference: '<path d="M4 3.5h12v13H4zM7 7h6M7 10h6M7 13h4"/>',
  };
  const documentIcon = '<path d="M5 2.5h7l4 4v11H5zM12 2.5v4h4M8 10h5M8 13h5"/>';
  const iconPaths = {
    book: '<path d="M10 5C8 3 5 3 2 3.5v12c3-.5 6-.5 8 1.5 2-2 5-2 8-1.5v-12C15 3 12 3 10 5Zm0 0v12"/>',
    introduction: '<circle cx="10" cy="10" r="7.5"/><path d="M10 9v5"/><circle cx="10" cy="6" r=".9" fill="currentColor" stroke="none"/>',
    features: '<path d="m8 3 1.8 4.7L14.5 9.5l-4.7 1.8L8 16l-1.8-4.7L1.5 9.5l4.7-1.8ZM15.5 2v4M13.5 4h4M16 13.5v4M14 15.5h4"/>',
    installation: '<path d="M10 2.5v10m-4-4 4 4 4-4M3 13.5v3a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-3"/>',
    updating: '<path d="M16.5 7A7 7 0 1 0 17 12M16.5 2.5V7H12"/>',
    setup: '<rect x="2" y="3" width="16" height="14" rx="2"/><path d="m5 7 3 3-3 3M11 13h4"/>',
    code: '<path d="m6 5-4 5 4 5m8-10 4 5-4 5M11.5 3l-3 14"/>',
    testing: '<path d="M7 2.5h6M8 2.5v5l-4.7 8a1.3 1.3 0 0 0 1.1 2h11.2a1.3 1.3 0 0 0 1.1-2l-4.7-8v-5M6 12h8"/>',
  };
  const chevronIcon = '<path d="m7 4 6 6-6 6"/>';

  if (!nav || !content || !article || !errorPanel || !retryButton) return;

  function setupScrollbar(container, contentSelector) {
    let hideTimer = null;
    let resizeObserver = null;
    function update() {
      const canScroll = container.scrollHeight > container.clientHeight;
      container.classList.toggle("has-overflow", canScroll);
      if (canScroll) return;
      container.classList.remove("is-scrolling");
      if (hideTimer !== null) {
        root.clearTimeout(hideTimer);
        hideTimer = null;
      }
    }
    function observeContent() {
      if (!resizeObserver || !contentSelector) return;
      for (const element of container.querySelectorAll(contentSelector)) resizeObserver.observe(element);
    }
    container.addEventListener("scroll", () => {
      update();
      if (!container.classList.contains("has-overflow")) return;
      container.classList.add("is-scrolling");
      if (hideTimer !== null) root.clearTimeout(hideTimer);
      hideTimer = root.setTimeout(() => {
        container.classList.remove("is-scrolling");
        hideTimer = null;
      }, 900);
    }, { passive: true });
    if (typeof root.ResizeObserver === "function") {
      resizeObserver = new root.ResizeObserver(update);
      resizeObserver.observe(container);
      observeContent();
    }
    if (typeof root.MutationObserver === "function") {
      const mutationObserver = new root.MutationObserver(() => {
        observeContent();
        update();
      });
      mutationObserver.observe(container, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["data-disclosure-state", "hidden"],
      });
    }
    root.addEventListener("resize", update);
    update();
  }

  setupScrollbar(nav, ".documentation-category");
  setupScrollbar(content, "#documentation-article");

  let catalog = null;
  let loadPromise = null;
  let selectedId = null;
  let disclosureController = null;
  const expanded = new Set();
  const buttonsById = new Map();
  const imageCache = new Map();

  function svgIcon(path, className) {
    const wrapper = root.document.createElement("span");
    wrapper.className = className;
    const svg = root.document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 20 20");
    svg.setAttribute("aria-hidden", "true");
    svg.innerHTML = path;
    wrapper.append(svg);
    return wrapper;
  }

  function safeArticleIcon(articleData) {
    const source = BlobUI.DocsData.resolveImage(articleData.icon || "");
    return typeof source === "string" && /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:svg|png|jpe?g|gif|webp)$/i.test(source) ? source : "";
  }

  function makeArticleIcon(articleData) {
    const source = safeArticleIcon(articleData);
    if (!source) return svgIcon(iconPaths[articleData.icon] || documentIcon, "documentation-item-icon");
    const wrapper = root.document.createElement("span");
    wrapper.className = "documentation-item-icon";
    const image = root.document.createElement("img");
    image.src = source;
    image.alt = "";
    image.setAttribute("aria-hidden", "true");
    image.addEventListener("error", () => image.replaceWith(svgIcon(documentIcon, "documentation-item-icon").firstChild));
    wrapper.append(image);
    return wrapper;
  }

  function makeCategoryIcon(categoryData) {
    const source = safeArticleIcon(categoryData);
    if (!source) return svgIcon(iconPaths[categoryData.icon] || categoryIconPaths[categoryData.id] || documentIcon, "documentation-category-icon");
    const wrapper = root.document.createElement("span");
    wrapper.className = "documentation-category-icon";
    const image = root.document.createElement("img");
    image.src = source;
    image.alt = "";
    image.setAttribute("aria-hidden", "true");
    image.addEventListener("error", () => image.replaceWith(svgIcon(categoryIconPaths[categoryData.id] || documentIcon, "documentation-category-icon").firstChild));
    wrapper.append(image);
    return wrapper;
  }

  function compareArticles(left, right) {
    return (left.index ?? Number.MAX_SAFE_INTEGER) - (right.index ?? Number.MAX_SAFE_INTEGER)
      || String(left.name || "").localeCompare(String(right.name || ""), undefined, { sensitivity: "base", numeric: true })
      || String(left.id || "").localeCompare(String(right.id || ""));
  }

  function makeArticleButton(articleData, hasChildren, child) {
    const button = root.document.createElement("button");
    button.type = "button";
    button.className = `documentation-item${child ? " documentation-child" : ""}`;
    button.dataset.articleId = articleData.id;
    button.setAttribute("aria-label", articleData.name);
    button.append(makeArticleIcon(articleData));
    const label = root.document.createElement("span");
    label.className = "documentation-item-label";
    label.textContent = articleData.name;
    button.append(label);
    if (hasChildren) {
      button.dataset.disclosureToggle = "";
      button.setAttribute("aria-expanded", "false");
      button.append(svgIcon(chevronIcon, "documentation-chevron"));
    }
    if (!hasChildren) button.addEventListener("click", () => {
      selectedId = articleData.id;
      updateActiveNavigation();
      renderArticle(articleData);
    });
    buttonsById.set(articleData.id, button);
    return button;
  }

  function updateActiveNavigation() {
    for (const [id, button] of buttonsById) {
      if (id === selectedId) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  function renderNavigation() {
    disclosureController?.destroy();
    disclosureController = null;
    nav.replaceChildren();
    buttonsById.clear();
    for (const categoryData of catalog.categories) {
      const group = root.document.createElement("section");
      group.className = "documentation-category";
      const heading = root.document.createElement("h2");
      heading.className = "documentation-category-label";
      heading.append(makeCategoryIcon(categoryData));
      const name = root.document.createElement("span");
      name.textContent = categoryData.name;
      heading.append(name);
      group.append(heading);

      const list = root.document.createElement("div");
      list.className = "documentation-article-list";
      const articles = catalog.articles.filter((item) => item.category === categoryData.id);
      const childrenByParent = new Map();
      const roots = [];
      for (const item of articles) {
        if (item.parent) {
          if (!childrenByParent.has(item.parent)) childrenByParent.set(item.parent, []);
          childrenByParent.get(item.parent).push(item);
        } else roots.push(item);
      }
      roots.sort(compareArticles);
      for (const item of roots) {
        const children = (childrenByParent.get(item.id) || []).sort(compareArticles);
        list.append(makeArticleButton(item, children.length > 0, false));
        if (children.length) {
          const childList = root.document.createElement("div");
          childList.id = `documentation-children-${item.id}`;
          childList.className = "documentation-children";
          childList.dataset.disclosurePanel = "";
          childList.hidden = !expanded.has(item.id);
          const parentButton = buttonsById.get(item.id);
          parentButton.setAttribute("aria-controls", childList.id);
          parentButton.setAttribute("aria-expanded", String(expanded.has(item.id)));
          const content = root.document.createElement("div");
          content.className = "documentation-children-inner";
          content.dataset.disclosureContent = "";
          for (const child of children) content.append(makeArticleButton(child, false, true));
          childList.append(content);
          list.append(childList);
        }
      }
      group.append(list);
      nav.append(group);
    }

    updateActiveNavigation();
    disclosureController = BlobUI.Disclosure.attach(nav, {
      onToggle({ toggle, expanded: isExpanded }) {
        const id = toggle.dataset.articleId;
        if (isExpanded) expanded.add(id);
        else expanded.delete(id);
        selectedId = id;
        updateActiveNavigation();
        const articleData = catalog.articles.find((item) => item.id === id);
        if (articleData) renderArticle(articleData);
      },
    });
  }

  async function imagesForMarkdown(markdown) {
    const images = Object.create(null);
    const source = String(markdown);
    const aliases = new Set([...source.matchAll(/@([A-Za-z0-9][A-Za-z0-9_-]{0,63})(?![A-Za-z0-9_-])/g)].map((match) => match[1]));
    const altByAlias = new Map([...source.matchAll(/!\[([^\]]*)\]\(@([A-Za-z0-9][A-Za-z0-9_-]{0,63})\)/g)]
      .map((match) => [match[2], match[1].trim()]));
    for (const alias of aliases) {
      const source = BlobUI.DocsData.resolveImage(alias);
      if (typeof source !== "string" || !/^\.\/cdn\/[A-Za-z0-9_-]+\.(?:png|jpe?g|gif|webp)$/i.test(source)) continue;
      if (!imageCache.has(alias)) {
        imageCache.set(alias, fetch(source).then(async (response) => {
          const mimeType = (response.headers.get("content-type") || "").split(";", 1)[0].toLowerCase();
          if (!response.ok || !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(mimeType)) return null;
          const bytes = new Uint8Array(await response.arrayBuffer());
          let binary = "";
          for (let offset = 0; offset < bytes.length; offset += 0x8000) {
            binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
          }
          return { src: `data:${mimeType};base64,${root.btoa(binary)}` };
        }).catch(() => null));
      }
      const image = await imageCache.get(alias);
      if (image) images[alias] = { ...image, alt: altByAlias.get(alias) || alias.replace(/[-_]+/g, " ") };
    }
    return images;
  }

  async function renderArticle(articleData) {
    const markdown = typeof articleData.markdown === "string" ? articleData.markdown : String(articleData.source || "");
    const images = await imagesForMarkdown(markdown);
    if (selectedId !== articleData.id) return;
    article.innerHTML = BlobUI.Markdown.render(markdown, { images });
    article.setAttribute("aria-label", articleData.name);
    errorPanel.hidden = true;
    root.document.getElementById("documentation-content").scrollTop = 0;
  }

  async function load() {
    if (!loadPromise) {
      loadPromise = BlobUI.DocsData.load().then(async (loaded) => {
        if (!loaded || !Array.isArray(loaded.categories) || !Array.isArray(loaded.articles)) {
          throw new TypeError("Documentation catalog has an invalid shape.");
        }
        catalog = loaded;
        const firstArticle = catalog.categories.map((categoryData) => catalog.articles
          .filter((item) => item.category === categoryData.id && !item.parent)
          .sort(compareArticles)[0]).find(Boolean) || catalog.articles[0];
        if (firstArticle) {
          selectedId = firstArticle.id;
          if (catalog.articles.some((item) => item.parent === firstArticle.id)) expanded.add(firstArticle.id);
        }
        renderNavigation();
        if (firstArticle) await renderArticle(firstArticle);
        else article.textContent = "No documentation articles are available.";
        errorPanel.hidden = true;
        return catalog;
      }).catch((error) => {
        loadPromise = null;
        article.replaceChildren();
        errorPanel.hidden = false;
        root.console.error("Could not load documentation.", error);
        return null;
      });
    }
    return loadPromise;
  }

  retryButton.addEventListener("click", () => { load(); });
  BlobUI.DocsUI = { load };
})(window);
