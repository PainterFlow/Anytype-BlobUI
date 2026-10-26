(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const allowedTags = new Set([
    "a", "abbr", "b", "blockquote", "br", "caption", "code", "del", "details", "div",
    "em", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "input", "ins", "kbd", "li",
    "ol", "p", "pre", "q", "s", "samp", "small", "span", "strike", "strong", "sub", "summary",
    "sup", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "u", "ul",
  ]);
  const removedTags = new Set([
    "audio", "base", "button", "embed", "form", "iframe", "link", "meta", "object",
    "script", "style", "svg", "template", "textarea", "video",
  ]);
  const imageDataUrl = /^data:image\/(png|jpeg|gif|webp);base64,[A-Za-z0-9+/]+={0,2}$/i;
  const aliasPattern = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;
  const sourcePattern = /^assets\/changelogs\/[A-Za-z0-9._/-]+$/;
  const repositoryUrl = "https://github.com/PainterFlow/Anytype-BlobUI";

  function unquote(value) {
    const text = value.trim();
    if (text.startsWith('"')) {
      try {
        return JSON.parse(text);
      } catch {
        throw new TypeError("Changelogs.yaml contains an invalid quoted value.");
      }
    }
    if (text.startsWith("'")) {
      if (!text.endsWith("'") || text.length < 2) throw new TypeError("Changelogs.yaml contains an invalid quoted value.");
      return text.slice(1, -1).replace(/''/g, "'");
    }
    if (/\s+#/.test(text)) return text.split(/\s+#/, 1)[0].trim();
    return text;
  }

  function validateImageSource(source, video = false) {
    if (!sourcePattern.test(source) || source.includes("\\")
      || source.split("/").some((part) => !part || part === "." || part === "..")) {
      throw new TypeError("Changelogs.yaml image paths must stay inside assets/changelogs/.");
    }
    if (!(video ? /\.mp4$/i : /\.(png|jpe?g|gif|webp)$/i).test(source)) {
      throw new TypeError("Changelogs.yaml supports PNG, JPEG, GIF, and WebP images.");
    }
    return source;
  }

  function parseManifestSection(yaml, video = false) {
    const entries = Object.create(null);
    let sectionSeen = false;
    let emptyList = false;
    let current = null;
    const lines = String(yaml || "").replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n").split("\n");

    function saveCurrent() {
      if (!current) return;
      if (!current.id || !current.src) throw new TypeError("Each Changelogs.yaml image needs an id and src.");
      if (Object.prototype.hasOwnProperty.call(entries, current.id)) throw new TypeError("Changelogs.yaml contains a duplicate image id.");
      entries[current.id] = {
        src: validateImageSource(current.src, video),
        alt: current.alt || current.id.replace(/[-_]+/g, " "),
      };
      current = null;
    }

    for (const line of lines) {
      if (!line.trim() || /^\s*#/.test(line)) continue;
      if (/\t/.test(line)) throw new TypeError("Changelogs.yaml must use spaces for indentation.");
      if (/^images:\s*(?:#.*)?$/.test(line)) {
        if (sectionSeen) throw new TypeError("Changelogs.yaml contains more than one images section.");
        sectionSeen = true;
        continue;
      }
      if (/^images:\s*\[\s*\]\s*(?:#.*)?$/.test(line)) {
        if (sectionSeen) throw new TypeError("Changelogs.yaml contains more than one images section.");
        sectionSeen = true;
        emptyList = true;
        continue;
      }
      if (!sectionSeen) throw new TypeError("Changelogs.yaml must begin with an images section.");
      if (emptyList) throw new TypeError("Changelogs.yaml cannot add entries after images: [].");
      const item = /^  - id:\s*(.+?)\s*$/.exec(line);
      if (item) {
        saveCurrent();
        current = { id: unquote(item[1]) };
        if (!aliasPattern.test(current.id)) throw new TypeError("Changelogs.yaml image ids may use letters, numbers, hyphens, and underscores.");
        continue;
      }
      const field = /^    (src|alt):\s*(.*?)\s*$/.exec(line);
      if (field && current) {
        if (Object.prototype.hasOwnProperty.call(current, field[1])) throw new TypeError(`Changelogs.yaml repeats ${field[1]} for an image.`);
        current[field[1]] = unquote(field[2]);
        continue;
      }
      throw new TypeError("Changelogs.yaml supports an images list with id, src, and optional alt fields.");
    }
    saveCurrent();
    if (!sectionSeen) throw new TypeError("Changelogs.yaml must contain an images section.");
    return entries;
  }

  function parseMediaManifest(yaml) {
    const sections = Object.create(null);
    let kind = "";
    for (const line of String(yaml || "").replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n").split("\n")) {
      const match = /^(images|videos):\s*(?:\[\s*\])?\s*(?:#.*)?$/.exec(line);
      if (match) {
        kind = match[1];
        if (sections[kind]) throw new TypeError("Changelogs.yaml contains more than one " + kind + " section.");
        sections[kind] = [line.replace(/^(images|videos):/, "images:")];
      } else if (kind) sections[kind].push(line);
      else if (line.trim() && !/^\s*#/.test(line)) throw new TypeError("Changelogs.yaml must begin with an images or videos section.");
    }
    if (!sections.images && !sections.videos) throw new TypeError("Changelogs.yaml must contain an images section.");
    const images = parseManifestSection((sections.images || ["images: []"]).join("\n"));
    const videos = parseManifestSection((sections.videos || ["images: []"]).join("\n"), true);
    if (Object.keys(videos).some(id => Object.hasOwn(images, id))) throw new TypeError("Changelogs.yaml contains a duplicate media id.");
    return { images, videos };
  }

  function parseImageManifest(yaml) { return parseMediaManifest(yaml).images; }
  function videoPlaceholder(alias, alt) {
    return '<span data-blobui-video="' + escapeAttribute(alias) + '" aria-label="' + escapeAttribute(alt || alias) + '"></span>';
  }

  function normalizedImages(images) {
    const result = Object.create(null);
    for (const [alias, image] of Object.entries(images || {})) {
      if (!aliasPattern.test(alias) || !image || typeof image !== "object") continue;
      const src = String(image.src || "");
      if (!imageDataUrl.test(src)) continue;
      result[alias] = { src, alt: String(image.alt || alias.replace(/[-_]+/g, " ")) };
    }
    return result;
  }

  function escapeAttribute(value) {
    return String(value).replace(/[&"<>]/g, (character) => ({
      "&": "&amp;", '"': "&quot;", "<": "&lt;", ">": "&gt;",
    })[character]);
  }

  function safeLink(value) {
    const href = String(value || "").trim();
    return /^(?:https?:\/\/|#[A-Za-z0-9_-]+)/i.test(href) ? href : "";
  }

  function renderImage(image, title) {
    if (!image) return "";
    const safeTitle = title ? ` title="${escapeAttribute(title)}"` : "";
    return `<img src="${image.src}" alt="${escapeAttribute(image.alt)}"${safeTitle}>`;
  }

  function allowAttribute(element, name, value, images, videos) {
    if (element.localName === "span" && name === "data-blobui-video") return Object.hasOwn(videos, value);
    if (element.localName === "span" && name === "aria-label") return true;
    const tag = element.localName;
    if (name.startsWith("on") || name === "style" || name === "srcdoc") return false;
    if (tag === "a" && name === "href") {
      const href = safeLink(value);
      if (!href) return false;
      element.setAttribute("href", href);
      return true;
    }
    if (tag === "a" && name === "title") return true;
    if (tag === "input" && name === "type") return value.toLowerCase() === "checkbox";
    if (tag === "input" && name === "disabled") return true;
    if (tag === "input" && name === "checked") return true;
    if (tag === "img" && name === "src") {
      let src = String(value || "");
      if (src.startsWith("@")) src = images[src.slice(1)]?.src || "";
      if (!imageDataUrl.test(src)) return false;
      element.setAttribute("src", src);
      return true;
    }
    if (tag === "img" && (name === "alt" || name === "title")) return true;
    if (tag === "img" && (name === "width" || name === "height")) return /^\d{1,4}$/.test(value);
    if ((tag === "td" || tag === "th") && (name === "colspan" || name === "rowspan")) return /^[1-9]\d{0,2}$/.test(value);
    if (tag === "ol" && name === "start") return /^-?\d{1,6}$/.test(value);
    return false;
  }

  function sanitizeHtml(html, images, documentImpl, videos = {}) {
    const doc = documentImpl || root.document;
    if (!doc || typeof doc.createElement !== "function") throw new TypeError("Markdown rendering needs a document.");
    const template = doc.createElement("template");
    template.innerHTML = String(html || "");

    function clean(parent) {
      for (const child of Array.from(parent.childNodes)) {
        if (child.nodeType === 3) continue;
        if (child.nodeType !== 1) {
          child.remove();
          continue;
        }
        const tag = child.localName;
        if (removedTags.has(tag)) {
          child.remove();
          continue;
        }
        clean(child);
        if (tag === "input") {
          const safeCheckbox = child.getAttribute("type") === "checkbox" && child.hasAttribute("disabled");
          if (!safeCheckbox) {
            child.remove();
            continue;
          }
        }
        if (!allowedTags.has(tag)) {
          while (child.firstChild) parent.insertBefore(child.firstChild, child);
          child.remove();
          continue;
        }
        for (const attribute of Array.from(child.attributes)) {
          const name = attribute.name.toLowerCase();
          if (!allowAttribute(child, name, attribute.value, images, videos)) child.removeAttribute(name);
        }
        if (tag === "img" && !imageDataUrl.test(child.getAttribute("src") || "")) {
          const replacement = doc.createTextNode(child.getAttribute("alt") || "");
          parent.replaceChild(replacement, child);
        }
      }
    }

    clean(template.content);
    return template.innerHTML;
  }

  function createParser(images, videos) {
    const renderer = new root.marked.Renderer();
    renderer.image = (token) => {
      let alias = String(token.href || "");
      if (alias.startsWith("@")) alias = alias.slice(1);
      if (Object.hasOwn(videos, alias)) return videoPlaceholder(alias, token.text || videos[alias].alt);
      const image = images[alias];
      if (!image) return escapeAttribute(token.text || "");
      const safeTitle = token.title ? ` title="${escapeAttribute(token.title)}"` : "";
      return `<img src="${image.src}" alt="${escapeAttribute(token.text || image.alt)}"${safeTitle}>`;
    };
    renderer.link = function (token) {
      const href = safeLink(token.href);
      const title = token.title ? ` title="${escapeAttribute(token.title)}"` : "";
      const text = token.tokens ? this.parser.parseInline(token.tokens) : token.text;
      return href ? `<a href="${escapeAttribute(href)}"${title}>${text}</a>` : text;
    };
    const parser = new root.marked.Marked({
      gfm: true,
      breaks: true,
      pedantic: false,
      renderer,
      extensions: [{
        name: "blobuiCheckboxLine",
        level: "block",
        start(source) {
          const match = /\n {0,3}\[[ xX]\](?:[\t ]+|(?=\n|$))/.exec(source);
          return match ? match.index + 1 : undefined;
        },
        tokenizer(source) {
          const match = /^ {0,3}\[([ xX])\](?:[\t ]+([^\n]*)|(?=\n|$))(?:\n|$)/.exec(source);
          if (!match) return undefined;
          return {
            type: "blobuiCheckboxLine",
            raw: match[0],
            checked: match[1].toLowerCase() === "x",
            tokens: this.lexer.inlineTokens(match[2] || ""),
          };
        },
        renderer(token) {
          return `<p><input type="checkbox" disabled${token.checked ? " checked" : ""}> ${this.parser.parseInline(token.tokens)}</p>\n`;
        },
      }, {
        name: "blobuiImageMention",
        level: "inline",
        start(source) {
          const pattern = /(^|[^A-Za-z0-9_+.-])@([A-Za-z0-9][A-Za-z0-9_-]{0,63})(?![A-Za-z0-9_-])/g;
          const match = pattern.exec(source);
          return match ? match.index + match[1].length : undefined;
        },
        tokenizer(source) {
          const match = /^@([A-Za-z0-9][A-Za-z0-9_-]{0,63})(?![A-Za-z0-9_-])/.exec(source);
          if (!match || !images[match[1]] && !Object.hasOwn(videos, match[1])) return undefined;
          return { type: "blobuiImageMention", raw: match[0], alias: match[1] };
        },
        renderer(token) {
          return Object.hasOwn(videos, token.alias) ? videoPlaceholder(token.alias, videos[token.alias].alt) : renderImage(images[token.alias]);
        },
      }, {
        name: "blobuiGitMention",
        level: "inline",
        start(source) {
          const pattern = /(^|[^A-Za-z0-9_-])(?:Issue|PR)#[1-9][0-9]*(?![A-Za-z0-9_-])|(^|[^A-Za-z0-9_-])User#[A-Za-z0-9](?:[A-Za-z0-9]|-(?=[A-Za-z0-9])){0,38}(?![A-Za-z0-9_-])/g;
          const match = pattern.exec(source);
          if (!match) return undefined;
          return match.index + (match[1] || match[2] || "").length;
        },
        tokenizer(source, tokens) {
          if (this.lexer.state.inLink || this.lexer.state.inRawBlock) return undefined;
          const previous = tokens?.[tokens.length - 1];
          const previousText = previous?.type === "text" ? String(previous.text || previous.raw || "") : "";
          if (/[A-Za-z0-9_-]$/.test(previousText)) return undefined;
          const issue = /^(Issue|PR)#([1-9][0-9]*)(?![A-Za-z0-9_-])/.exec(source);
          if (issue) {
            const kind = issue[1] === "Issue" ? "issues" : "pull";
            return {
              type: "blobuiGitMention",
              raw: issue[0],
              href: `${repositoryUrl}/${kind}/${issue[2]}`,
              text: issue[0],
            };
          }
          const user = /^User#([A-Za-z0-9](?:[A-Za-z0-9]|-(?=[A-Za-z0-9])){0,38})(?![A-Za-z0-9_-])/.exec(source);
          if (!user || user[1].includes("--")) return undefined;
          return {
            type: "blobuiGitMention",
            raw: user[0],
            href: `https://github.com/${user[1]}`,
            text: `@${user[1]}`,
          };
        },
        renderer(token) {
          return `<a href="${escapeAttribute(token.href)}">${escapeAttribute(token.text)}</a>`;
        },
      }],
    });
    return parser;
  }

  BlobUI.Markdown = Object.freeze({
    parseImageManifest,
    parseMediaManifest,
    mount(container, markdown, options = {}) {
      BlobUI.VideoPlayer?.destroyTree(container);
      container.innerHTML = this.render(markdown, options);
      for (const slot of container.querySelectorAll("[data-blobui-video]")) {
        const media = options.videos?.[slot.getAttribute("data-blobui-video")];
        if (media && BlobUI.VideoPlayer) BlobUI.VideoPlayer.attach(slot, media);
      }
    },
    render(markdown, options = {}) {
      const images = normalizedImages(options.images);
      const videos = Object.fromEntries(Object.entries(options.videos || {}).filter(([alias, media]) => aliasPattern.test(alias) && media && typeof media === "object"));
      const parser = createParser(images, videos);
      const html = parser.parse(String(markdown || ""));
      return sanitizeHtml(html, images, options.document, videos);
    },
  });
})(typeof window !== "undefined" ? window : globalThis);
