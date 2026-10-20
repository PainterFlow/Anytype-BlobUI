(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const nav = root.document.getElementById("documentation-navigation");
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
  const chevronIcon = '<path d="m7 4 6 6-6 6"/>';

  if (!nav || !article || !errorPanel || !retryButton) return;

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
    if (!source) return svgIcon(documentIcon, "documentation-item-icon");
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
    if (!source) return svgIcon(categoryIconPaths[categoryData.id] || documentIcon, "documentation-category-icon");
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
    return String(left.name || "").localeCompare(String(right.name || ""), undefined, { sensitivity: "base", numeric: true });
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
