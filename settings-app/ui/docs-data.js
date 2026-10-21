(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const categories = Object.freeze([
    Object.freeze({ id: "getting-started", name: "User guide", icon: "book" }),
    Object.freeze({ id: "development", name: "Development", icon: "code" }),
  ]);
  const imagePaths = Object.freeze(Object.assign(Object.create(null), {
    "blobui-mark": "./cdn/blobui-mark.svg",
    "theme-swatches": "./cdn/theme-swatches.png",
  }));
  const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
  const articleFields = new Set(["category", "parent", "icon", "name", "index"]);
  const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

  function fail(id, message) {
    throw new TypeError(`Documentation article ${id}: ${message}`);
  }

  function validateId(id) {
    const value = String(id || "").replace(/\.md$/i, "");
    if (!slugPattern.test(value)) throw new TypeError(`Invalid documentation article id "${value}"; use a lowercase filename slug.`);
    return value;
  }

  function scalar(value, id, field) {
    const text = value.trim();
    if (!text) fail(id, `frontmatter field "${field}" must not be empty.`);
    if (text.startsWith('"')) {
      try {
        const parsed = JSON.parse(text);
        if (typeof parsed !== "string" || !parsed.trim()) fail(id, `frontmatter field "${field}" must be a non-empty string.`);
        return parsed.trim();
      } catch (error) {
        if (error instanceof TypeError && error.message.startsWith("Documentation article")) throw error;
        fail(id, `frontmatter field "${field}" has invalid quoted text.`);
      }
    }
    if (text.startsWith("'")) {
      if (text.length < 2 || !text.endsWith("'")) fail(id, `frontmatter field "${field}" has invalid quoted text.`);
      return text.slice(1, -1).replace(/''/g, "'").trim();
    }
    if (/#|[\[\]{}]/.test(text)) fail(id, `frontmatter field "${field}" must be a simple string.`);
    return text;
  }

  function parseArticle(source, id) {
    const articleId = validateId(id);
    const text = String(source || "").replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
    const lines = text.split("\n");
    if (lines[0] !== "---") fail(articleId, "frontmatter must start with ---.");
    const end = lines.indexOf("---", 1);
    if (end < 0) fail(articleId, "frontmatter is missing its closing ---.");

    const fields = Object.create(null);
    for (const line of lines.slice(1, end)) {
      if (!line.trim() || /^\s*#/.test(line)) continue;
      const match = /^([a-z]+):\s*(.*?)\s*$/.exec(line);
      if (!match) fail(articleId, `invalid frontmatter line "${line}".`);
      const [, key, rawValue] = match;
      if (!articleFields.has(key)) fail(articleId, `unknown frontmatter field "${key}".`);
      if (Object.prototype.hasOwnProperty.call(fields, key)) fail(articleId, `frontmatter repeats "${key}".`);
      fields[key] = scalar(rawValue, articleId, key);
    }
    for (const key of ["category", "icon", "name"]) {
      if (!fields[key]) fail(articleId, `frontmatter needs a "${key}" field.`);
    }
    if (fields.index && (!/^[1-9]\d*$/.test(fields.index) || !Number.isSafeInteger(Number(fields.index)))) {
      fail(articleId, 'frontmatter field "index" must be a positive integer starting at 1.');
    }
    if (fields.parent) {
      const parent = fields.parent.replace(/\.md$/i, "");
      if (!slugPattern.test(parent) && !parent.trim()) fail(articleId, "parent must be an article slug or name.");
      fields.parent = parent;
    }
    const markdown = lines.slice(end + 1).join("\n").replace(/^\n+|\n+$/g, "");
    if (!markdown) fail(articleId, "article body must not be empty.");
    return Object.freeze({
      id: articleId,
      category: fields.category,
      index: fields.index ? Number(fields.index) : null,
      parent: fields.parent || null,
      icon: fields.icon,
      name: fields.name,
      markdown,
    });
  }

  function buildCatalog(entries) {
    if (!Array.isArray(entries)) throw new TypeError("Documentation entries must be an array.");
    const categoryById = new Map(categories.map((category) => [category.id, category]));
    const parsed = [];
    const articleById = new Map();

    for (const entry of entries) {
      if (!entry || typeof entry !== "object") throw new TypeError("Each documentation entry needs an id and source.");
      const article = parseArticle(entry.source, entry.id);
      if (articleById.has(article.id)) fail(article.id, "duplicate article id.");
      if (!categoryById.has(article.category)) fail(article.id, `unknown category "${article.category}".`);
      articleById.set(article.id, article);
      parsed.push(article);
    }

    const byName = new Map();
    for (const article of parsed) {
      const key = article.name.toLocaleLowerCase();
      if (!byName.has(key)) byName.set(key, []);
      byName.get(key).push(article);
    }
    const parentById = new Map();
    for (const article of parsed) {
      if (!article.parent) continue;
      let parent = articleById.get(article.parent);
      if (!parent) {
        const named = byName.get(article.parent.toLocaleLowerCase()) || [];
        if (named.length > 1) fail(article.id, `parent name "${article.parent}" is ambiguous; use its slug.`);
        parent = named[0];
      }
      if (!parent) fail(article.id, `unknown parent "${article.parent}".`);
      if (parent.category !== article.category) fail(article.id, `parent "${parent.id}" belongs to a different category.`);
      parentById.set(article.id, parent);
    }
    const visitState = new Map();
    function visit(article) {
      const state = visitState.get(article.id) || 0;
      if (state === 1) fail(article.id, "parent relationships create a cycle.");
      if (state === 2) return;
      visitState.set(article.id, 1);
      const parent = parentById.get(article.id);
      if (parent) visit(parent);
      visitState.set(article.id, 2);
    }
    parsed.forEach(visit);
    const resolved = parsed.map((article) => {
      const parent = parentById.get(article.id);
      if (!parent) return article;
      if (parent.parent) fail(article.id, `parent "${parent.id}" already has a parent; documentation supports one parent level.`);
      return Object.freeze({ ...article, parent: parent.id });
    });

    const articles = resolved.sort((a, b) => (a.index ?? Number.MAX_SAFE_INTEGER) - (b.index ?? Number.MAX_SAFE_INTEGER)
      || collator.compare(a.name, b.name) || collator.compare(a.id, b.id));
    return Object.freeze({
      categories,
      articles: Object.freeze(articles),
    });
  }

  async function load() {
    const ids = ["introduction", "features", "installation-and-removal", "updating", "setup-and-architecture", "theme-and-documentation-development", "testing-and-releases"];
    const entries = await Promise.all(ids.map(async (id) => {
      const response = await root.fetch(`./docs/${id}.md`);
      if (!response.ok) throw new Error(`Could not load documentation article ${id} (${response.status}).`);
      return { id, source: await response.text() };
    }));
    return buildCatalog(entries);
  }

  function resolveImage(alias) {
    const key = String(alias || "");
    return Object.prototype.hasOwnProperty.call(imagePaths, key) ? imagePaths[key] : "";
  }

  BlobUI.DocsData = Object.freeze({ load, parseArticle, buildCatalog, resolveImage, imageAliases: imagePaths });
})(typeof window !== "undefined" ? window : globalThis);
