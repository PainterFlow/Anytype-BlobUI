import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { DisclosurePanel } from "../shared/disclosure.jsx";
import { createPortal } from "react-dom";

const categoryIcons = {
  "getting-started": "M3 4.5h14v11H3zM6 8h8M6 11h5",
  development: "m6 5-4 5 4 5m8-10 4 5-4 5M11.5 3l-3 14",
};
const articleIcons = {
  book: "M10 5C8 3 5 3 2 3.5v12c3-.5 6-.5 8 1.5 2-2 5-2 8-1.5v-12C15 3 12 3 10 5Zm0 0v12",
  introduction: "M10 2.5a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15ZM10 9v5",
  features: "m8 3 1.8 4.7L14.5 9.5l-4.7 1.8L8 16l-1.8-4.7L1.5 9.5 6.2 7.7ZM15.5 2v4M13.5 4h4M16 13.5v4M14 15.5h4",
  installation: "M10 2.5v10m-4-4 4 4 4-4M3 13.5v3a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-3",
  updating: "M16.5 7A7 7 0 1 0 17 12M16.5 2.5V7H12",
  setup: "M2 3h16v14H2zM5 7l3 3-3 3m6 0h4",
  code: "m6 5-4 5 4 5m8-10 4 5-4 5M11.5 3l-3 14",
  testing: "M7 2.5h6M8 2.5v5l-4.7 8a1.3 1.3 0 0 0 1.1 2h11.2a1.3 1.3 0 0 0 1.1-2l-4.7-8v-5M6 12h8",
  experiment: "M7 2.5h6M8 2.5v5.2l-4.3 7.1A2 2 0 0 0 5.4 18h9.2a2 2 0 0 0 1.7-3.2L12 7.7V2.5M6 13h8",
  "settings-ui": "M2.5 3h15v14h-15zM7.5 3v14M10.5 7h4m-4 3h4m-4 3h4",
};
const documentIcon = "M5 2.5h7l4 4v11H5zM12 2.5v4h4M8 10h5M8 13h5";
const chevronIcon = "m7 4 6 6-6 6";
const aliasPattern = /@([A-Za-z0-9][A-Za-z0-9_-]{0,63})(?![A-Za-z0-9_-])/g;
const imageCache = new Map();
const videoCache = new Map();

function SvgIcon({ path, className }) {
  return <span className={className}><svg viewBox="0 0 20 20" aria-hidden="true"><path d={path} /></svg></span>;
}

function AnyblobIcon({ className }) {
  return <span className={className}><svg viewBox="0 0 24 24" aria-hidden="true" style={{ strokeWidth: 1.5 }}>
    {Array.from({ length: 6 }, (_, index) => <path key={index} transform={`rotate(${index * 60} 12 12)`} d="M8.5 13V6l5-2.9a5.3 5.3 0 0 1 7.2 1.9" />)}
  </svg></span>;
}

function articleIconSource(item) {
  const source = window.BlobUI.DocsData.resolveImage(item.icon || "");
  return typeof source === "string" && /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:svg|png|jpe?g|gif|webp)$/i.test(source) ? source : "";
}

function Icon({ item, category = false }) {
  const className = category ? "documentation-category-icon" : "documentation-item-icon";
  if (!category && item.icon === "anyblob") return <AnyblobIcon className={className} />;
  const source = articleIconSource(item);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [source]);
  if (source && !failed) return <span className={className}><img src={source} alt="" aria-hidden="true" onError={() => setFailed(true)} /></span>;
  const paths = category ? categoryIcons : articleIcons;
  return <SvgIcon className={className} path={paths[item.icon] || articleIcons[item.icon] || paths[item.id] || documentIcon} />;
}

function compareArticles(left, right) {
  return (left.index ?? Number.MAX_SAFE_INTEGER) - (right.index ?? Number.MAX_SAFE_INTEGER)
    || String(left.name || "").localeCompare(String(right.name || ""), undefined, { sensitivity: "base", numeric: true })
    || String(left.id || "").localeCompare(String(right.id || ""));
}

async function imageAsset(alias) {
  if (!imageCache.has(alias)) {
    const source = window.BlobUI.DocsData.resolveImage(alias);
    const promise = (async () => {
      if (typeof source !== "string" || !/^\.\/cdn\/[A-Za-z0-9_-]+\.(?:png|jpe?g|gif|webp)$/i.test(source)) return null;
      const response = await fetch(source);
      const mimeType = (response.headers.get("content-type") || "").split(";", 1)[0].toLowerCase();
      if (!response.ok || !["image/png", "image/jpeg", "image/gif", "image/webp"].includes(mimeType)) return null;
      const bytes = new Uint8Array(await response.arrayBuffer());
      let binary = "";
      for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
      return { src: `data:${mimeType};base64,${window.btoa(binary)}` };
    })().catch(() => null);
    imageCache.set(alias, promise);
  }
  return imageCache.get(alias);
}

async function mediaForMarkdown(markdown) {
  const source = String(markdown || "");
  const aliases = new Set([...source.matchAll(aliasPattern)].map(match => match[1]));
  const altByAlias = new Map([...source.matchAll(/!\[([^\]]*)\]\(@([A-Za-z0-9][A-Za-z0-9_-]{0,63})\)/g)].map(match => [match[2], match[1].trim()]));
  const images = Object.create(null);
  const videos = Object.create(null);
  await Promise.all([...aliases].map(async alias => {
    const image = await imageAsset(alias);
    if (image) images[alias] = { ...image, alt: altByAlias.get(alias) || alias.replace(/[-_]+/g, " ") };
    const videoSource = window.BlobUI.DocsData.videoAliases?.[alias];
    if (!videoSource) return;
    if (!videoCache.has(alias)) {
      videoCache.set(alias, (async () => {
        try {
          const response = await fetch(videoSource);
          if (!response.ok || Number(response.headers.get("content-length")) > 64 * 1024 * 1024) return null;
          const blob = await response.blob();
          return blob.size > 0 && blob.size <= 64 * 1024 * 1024 ? { blob: new Blob([blob], { type: "video/mp4" }), alt: alias.replace(/-/g, " ") } : null;
        } catch { return null; }
      })());
    }
    const video = await videoCache.get(alias);
    if (video) videos[alias] = video;
  }));
  return { images, videos };
}

/** Render sanitized Markdown in React and attach the shared accessible media controllers. */
export function MarkdownView({ markdown = "", images, videos, id, className = "markdown-content", ...regionProps }) {
  const containerRef = useRef(null);
  const [videoSlots, setVideoSlots] = useState([]);
  const html = useMemo(() => window.BlobUI.Markdown.render(markdown, { images, videos }), [markdown, images, videos]);
  useLayoutEffect(() => {
    const container = containerRef.current;
    const slots = Array.from(container?.querySelectorAll("[data-blobui-video]") || []);
    setVideoSlots(slots.map(slot => ({ slot, alias: slot.getAttribute("data-blobui-video") })));
  }, [html]);
  useEffect(() => {
    const container = containerRef.current;
    window.BlobUI.Markdown.attachImages(container);
    return () => {
      window.BlobUI.VideoPlayer?.destroyTree(container);
      window.BlobUI.MarkdownImageViewer?.destroyTree(container);
    };
  }, [html]);
  return <>
    <div {...regionProps} id={id} ref={containerRef} className={className} dangerouslySetInnerHTML={{ __html: html }} />
    {videoSlots.map(({ slot, alias }) => createPortal(<MarkdownVideo media={videos?.[alias]} label={slot.getAttribute("aria-label")} />, slot, alias))}
  </>;
}

export function DeveloperGuideReader({ visible = false }) {
  const contentRef = useRef(null);
  const [attempt, setAttempt] = useState(0);
  const [payload, setPayload] = useState(null);
  const [loadError, setLoadError] = useState(false);

  useEffect(() => {
    if (!visible || payload) return undefined;
    let active = true;
    fetch("./dev-guide.md").then(response => {
      if (!response.ok) throw new Error(`Dev Guide request failed (${response.status}).`);
      return response.text();
    }).then(async markdown => {
      const text = String(markdown || "").trim();
      if (!text) throw new Error("Dev Guide is empty.");
      const media = await mediaForMarkdown(text);
      if (!active) return;
      setPayload({ markdown: text, ...media });
      setLoadError(false);
    }).catch(() => {
      if (active) setLoadError(true);
    });
    return () => { active = false; };
  }, [visible, attempt, payload]);

  useEffect(() => {
    if (!visible || !contentRef.current) return undefined;
    const scrollbar = window.BlobUI.Scrollbar.attach(contentRef.current);
    return () => scrollbar.destroy();
  }, [visible]);

  return <div className="documentation-layout dev-guide-layout">
    <main ref={contentRef} id="documentation-content" className="documentation-content" aria-label="Developer guide" tabIndex={-1}>
      {loadError ? <div id="documentation-load-error" className="documentation-load-error" role="alert">
        <p>Dev Guide could not be loaded.</p>
        <button id="documentation-retry" type="button" className="banner-image-button" onClick={() => {
          setPayload(null);
          setLoadError(false);
          setAttempt(value => value + 1);
        }}>Try again</button>
      </div> : payload ? <MarkdownView id="documentation-article" className="documentation-article dev-guide-article markdown-content"
        aria-label="Developer guide" markdown={payload.markdown} images={payload.images} videos={payload.videos} />
        : <p className="documentation-article-loading" role="status">Loading Dev Guide…</p>}
    </main>
  </div>;
}

const videoIconPaths = {
  play: "m7 4 10 6-10 6Z",
  pause: "M7 4v12M13 4v12",
  mute: "M9 4 5 7H2v6h3l4 3ZM13 7l5 6m0-6-5 6",
  volume: "M9 4 5 7H2v6h3l4 3ZM13 6a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14",
  fullscreen: "M3 8V3h5M12 3h5v5M17 12v5h-5M8 17H3v-5",
};

function VideoIcon({ name }) {
  return <svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d={videoIconPaths[name]} /></svg>;
}

function MarkdownVideo({ media, label }) {
  const hostRef = useRef(null);
  useEffect(() => {
    const host = hostRef.current;
    if (!media || !host) return undefined;
    window.BlobUI.VideoPlayer.attachExisting(host, media);
    return () => window.BlobUI.VideoPlayer.destroyTree(host);
  }, [media]);
  return <span ref={hostRef} className="markdown-video" tabIndex={0} role="group" aria-label={label || media?.alt || "Video"}>
    <video preload="metadata" muted playsInline controls={false} controlsList="nofullscreen" aria-label={label || media?.alt || "Video"} />
    <button type="button" className="markdown-video-overlay" aria-label="Play" title="Play"><VideoIcon name="play" /></button>
    <span className="markdown-video-controls">
      <button type="button" className="markdown-video-fullscreen" aria-label="Enter fullscreen" title="Enter fullscreen"><VideoIcon name="fullscreen" /></button>
      <input type="range" min="0" max="0" step="0.01" defaultValue="0" aria-label="Seek video" className="markdown-video-seek" />
      <span className="markdown-video-time">0:00 / 0:00</span>
      <button type="button" className="markdown-video-mute" aria-label="Unmute" title="Unmute"><VideoIcon name="mute" /></button>
      <input type="range" min="0" max="1" step="0.05" defaultValue="1" aria-label="Volume" className="markdown-video-volume" />
    </span>
    <span className="markdown-video-status" role="status">Loading video…</span>
  </span>;
}

const safeImageDataUrlPattern = /^data:image\/(png|jpeg|gif|webp);base64,((?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?)$/i;
function safeImageDataUrl(value) {
  const match = safeImageDataUrlPattern.exec(String(value || ""));
  if (!match || match[2].length < 16 || match[2].length % 4 !== 0) return "";
  let header;
  try { header = window.atob(match[2].slice(0, 16)); } catch { return ""; }
  const mime = match[1].toLowerCase();
  const valid = mime === "png" ? header.charCodeAt(0) === 0x89 && header.slice(1, 4) === "PNG"
    : mime === "jpeg" ? header.charCodeAt(0) === 0xff && header.charCodeAt(1) === 0xd8 && header.charCodeAt(2) === 0xff
      : mime === "gif" ? header.startsWith("GIF87a") || header.startsWith("GIF89a")
        : header.startsWith("RIFF") && header.slice(8, 12) === "WEBP";
  return valid ? match[0] : "";
}

function markMarkdownImages(container) {
  for (const image of container?.querySelectorAll?.("img") || []) {
    if (image.closest("a") || !safeImageDataUrl(image.getAttribute("src"))) continue;
    image.classList.add("markdown-image-view-trigger");
    image.setAttribute("role", "button");
    image.setAttribute("tabindex", "0");
    image.setAttribute("aria-label", `Expand image: ${image.alt || "Documentation image"}`);
    image.title = image.title || "Expand image";
    image.draggable = false;
  }
}

let activeImageViewer = null;
function destroyImageViewerTree(container) {
  if (activeImageViewer?.sourceImage && container?.contains?.(activeImageViewer.sourceImage)) activeImageViewer.modal.close({ restoreFocus: false });
}
window.BlobUI.MarkdownImageViewer = Object.freeze({ mount: markMarkdownImages, destroyTree: destroyImageViewerTree, isOpen: () => Boolean(activeImageViewer?.modal.isOpen()) });

/** A shared fullscreen image preview. Render once beside the application's dialogs. */
export function MarkdownImageViewer() {
  const [image, setImage] = useState(null);
  const [actualSize, setActualSize] = useState(false);
  const layerRef = useRef(null);
  const backdropRef = useRef(null);
  const dialogRef = useRef(null);
  const stageRef = useRef(null);
  const previewRef = useRef(null);
  const modalRef = useRef(null);
  const sourceRef = useRef(null);
  const zoomAnchorRef = useRef(null);
  useLayoutEffect(() => {
    const layer = layerRef.current;
    const backdrop = backdropRef.current;
    const dialog = dialogRef.current;
    const modal = window.BlobUI.createModalController({ backdrop, dialog, initialFocus: () => previewRef.current,
      fallbackFocus: () => document.querySelector("#documentation:not([hidden]) #documentation-content")
        || document.querySelector("#release-notes-backdrop:not([hidden]) #release-notes-content")
        || document.getElementById("help-button"),
      inertTargets: Array.from(document.body.children).filter(element => element !== layer),
      onClose() {
        sourceRef.current = null;
        activeImageViewer = null;
        setImage(null);
        setActualSize(false);
      },
    });
    modalRef.current = modal;
    const open = source => {
      if (sourceRef.current || modal.isOpen() || !source?.isConnected || source.closest("a")) return;
      const src = safeImageDataUrl(source.getAttribute("src"));
      if (!src) return;
      sourceRef.current = source;
      activeImageViewer = { modal, sourceImage: source };
      setImage({ src, alt: String(source.getAttribute("alt") || "").trim() });
      setActualSize(false);
      try { source.focus({ preventScroll: true }); } catch { source.focus(); }
    };
    const click = event => {
      const source = event.target?.closest?.(".markdown-content img.markdown-image-view-trigger");
      if (!source || source.closest("a") || !document.contains(source)) return;
      event.preventDefault();
      open(source);
    };
    const keydown = event => {
      if (event.key !== "Enter" && event.key !== " ") return;
      const source = event.target?.closest?.(".markdown-content img.markdown-image-view-trigger");
      if (!source || source.closest("a") || !document.contains(source)) return;
      event.preventDefault();
      open(source);
    };
    const toggleSize = event => {
      if (event.target === dialog || event.target === stageRef.current) modal.close();
    };
    document.addEventListener("click", click);
    document.addEventListener("keydown", keydown);
    dialog.addEventListener("pointerdown", toggleSize);
    return () => {
      modal.destroy();
      document.removeEventListener("click", click);
      document.removeEventListener("keydown", keydown);
      dialog.removeEventListener("pointerdown", toggleSize);
      if (activeImageViewer?.modal === modal) activeImageViewer = null;
      modalRef.current = null;
    };
  }, []);
  useLayoutEffect(() => {
    if (image && !modalRef.current?.isOpen()) modalRef.current?.open();
  }, [image]);
  const toggleActualSize = event => {
    const stage = stageRef.current;
    const preview = previewRef.current;
    if (stage && preview && !actualSize) {
      const stageBounds = stage.getBoundingClientRect();
      const imageBounds = preview.getBoundingClientRect();
      const usePointer = event?.type === "click" && event.detail > 0;
      const clientX = usePointer ? event.clientX : imageBounds.left + imageBounds.width / 2;
      const clientY = usePointer ? event.clientY : imageBounds.top + imageBounds.height / 2;
      const sourceWidth = preview.naturalWidth || imageBounds.width;
      const sourceHeight = preview.naturalHeight || imageBounds.height;
      zoomAnchorRef.current = {
        imageX: Math.max(0, Math.min(sourceWidth, (clientX - imageBounds.left) * sourceWidth / imageBounds.width)),
        imageY: Math.max(0, Math.min(sourceHeight, (clientY - imageBounds.top) * sourceHeight / imageBounds.height)),
        viewportX: clientX - stageBounds.left,
        viewportY: clientY - stageBounds.top,
      };
    } else {
      zoomAnchorRef.current = null;
    }
    setActualSize(value => !value);
  };
  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const anchor = zoomAnchorRef.current;
    if (image && actualSize && anchor) {
      const stageBounds = stage.getBoundingClientRect();
      const imageBounds = previewRef.current?.getBoundingClientRect();
      if (imageBounds) {
        const imageOriginX = imageBounds.left - stageBounds.left + stage.scrollLeft;
        const imageOriginY = imageBounds.top - stageBounds.top + stage.scrollTop;
        stage.scrollLeft = imageOriginX + anchor.imageX - anchor.viewportX;
        stage.scrollTop = imageOriginY + anchor.imageY - anchor.viewportY;
      }
    } else {
      stage.scrollTop = 0;
      stage.scrollLeft = 0;
    }
    zoomAnchorRef.current = null;
  }, [actualSize, image]);
  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage || !actualSize || typeof ResizeObserver === "undefined") return;
    let width = stage.clientWidth;
    let height = stage.clientHeight;
    const observer = new ResizeObserver(() => {
      const nextWidth = stage.clientWidth;
      const nextHeight = stage.clientHeight;
      stage.scrollLeft += (width - nextWidth) / 2;
      stage.scrollTop += (height - nextHeight) / 2;
      width = nextWidth;
      height = nextHeight;
    });
    observer.observe(stage);
    return () => observer.disconnect();
  }, [actualSize]);
  const layer = <div ref={layerRef} className="markdown-image-viewer-layer">
    <div ref={backdropRef} className="confirm-backdrop markdown-image-viewer-backdrop" hidden>
      <section ref={dialogRef} className="confirm-dialog markdown-image-viewer-dialog" role="dialog" aria-modal="true" aria-label={image?.alt || "Image preview"} tabIndex={-1}>
        <div ref={stageRef} className={`markdown-image-viewer-stage${actualSize ? " is-actual-size" : ""}`} role="region" aria-label="Image preview; scroll to inspect the actual-size image" tabIndex={0}>
          {image && <img ref={previewRef} className="markdown-image-viewer-image" src={image.src} alt={image.alt} draggable={false} tabIndex={0} onError={() => modalRef.current?.close()}
            role="button" aria-pressed={String(actualSize)} aria-label={actualSize ? "Fit image to window" : "Show image at actual size"} onClick={toggleActualSize}
            onKeyDown={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); toggleActualSize(event); } }} />}
        </div>
      </section>
    </div>
  </div>;
  return createPortal(layer, document.body);
}

function ArticleNavigation({ item, children, expanded, selectedId, onSelect }) {
  const toggle = useRef(null);
  const hasChildren = children.length > 0;
  const childListId = `documentation-children-${item.id}`;
  return <>
    <button ref={toggle} type="button" className="documentation-item" data-article-id={item.id} aria-label={item.name}
      aria-current={selectedId === item.id ? "page" : undefined}
      aria-expanded={hasChildren ? expanded : undefined} aria-controls={hasChildren ? childListId : undefined}
      data-disclosure-toggle={hasChildren ? "" : undefined} onClick={() => onSelect(item, hasChildren)}>
      <Icon item={item} /><span className="documentation-item-label">{item.name}</span>
      {hasChildren && <SvgIcon className="documentation-chevron" path={chevronIcon} />}
    </button>
    {hasChildren && <DisclosurePanel id={childListId} expanded={expanded} toggle={toggle}>
      {children.map(child => <button type="button" key={child.id}
        className="documentation-item documentation-child" data-article-id={child.id} aria-label={child.name}
        aria-current={selectedId === child.id ? "page" : undefined} onClick={() => onSelect(child, false)}>
        <Icon item={child} /><span className="documentation-item-label">{child.name}</span>
      </button>)}
    </DisclosurePanel>}
  </>;
}

export function DocumentationReader({ visible = false }) {
  const navigationRef = useRef(null);
  const contentRef = useRef(null);
  const [attempt, setAttempt] = useState(0);
  const [catalog, setCatalog] = useState(null);
  const [loadError, setLoadError] = useState(false);
  const [selectedId, setSelectedId] = useState(null);
  const [expandedArticles, setExpandedArticles] = useState(() => new Set());
  const [articlePayload, setArticlePayload] = useState(null);
  const [articleBusy, setArticleBusy] = useState(false);

  useEffect(() => {
    if (!visible || catalog || loadError) return undefined;
    let active = true;
    window.BlobUI.DocsUI.load().then(loaded => {
      if (!loaded || !Array.isArray(loaded.categories) || !Array.isArray(loaded.articles)) throw new TypeError("Documentation catalog has an invalid shape.");
      if (!active) return;
      const first = loaded.categories.map(category => loaded.articles.filter(item => item.category === category.id && !item.parent).sort(compareArticles)[0]).find(Boolean) || loaded.articles[0];
      setCatalog(loaded);
      setSelectedId(first?.id || null);
      if (first && loaded.articles.some(item => item.parent === first.id)) setExpandedArticles(new Set([first.id]));
      setLoadError(false);
    }).catch(error => {
      if (!active) return;
      window.console.error("Could not load documentation.", error);
      setLoadError(true);
    });
    return () => { active = false; };
  }, [visible, attempt, catalog, loadError]);

  const selectedArticle = catalog?.articles.find(item => item.id === selectedId) || null;
  useEffect(() => {
    if (!selectedArticle) { setArticlePayload(null); return undefined; }
    let active = true;
    const markdown = selectedArticle.markdown;
    setArticleBusy(true);
    mediaForMarkdown(markdown).then(payload => {
      if (active) setArticlePayload({ articleId: selectedArticle.id, markdown, ...payload });
    }).finally(() => { if (active) setArticleBusy(false); });
    contentRef.current?.scrollTo({ top: 0, behavior: "instant" });
    return () => { active = false; };
  }, [selectedArticle]);

  useEffect(() => {
    const nav = navigationRef.current;
    if (!nav) return undefined;
    const scrollbar = window.BlobUI.Scrollbar.attach(nav, ".documentation-category");
    const contentScrollbar = window.BlobUI.Scrollbar.attach(contentRef.current, "#documentation-article");
    return () => {
      scrollbar.destroy();
      contentScrollbar.destroy();
    };
  }, [catalog]);

  function chooseArticle(item, toggle) {
    setSelectedId(item.id);
    if (toggle) setExpandedArticles(previous => {
      const next = new Set(previous);
      if (next.has(item.id)) next.delete(item.id);
      else next.add(item.id);
      return next;
    });
  }

  const categories = catalog?.categories.map(category => {
    const articles = catalog.articles.filter(item => item.category === category.id);
    const childrenByParent = new Map();
    const roots = [];
    for (const item of articles) {
      if (item.parent) {
        if (!childrenByParent.has(item.parent)) childrenByParent.set(item.parent, []);
        childrenByParent.get(item.parent).push(item);
      } else roots.push(item);
    }
    roots.sort(compareArticles);
    return <section className="documentation-category" key={category.id}>
      <h2 className="documentation-category-label"><Icon item={category} category /><span>{category.name}</span></h2>
      <div className="documentation-article-list">{roots.map(item => {
        const children = (childrenByParent.get(item.id) || []).sort(compareArticles);
        return <ArticleNavigation key={item.id} item={item} expanded={expandedArticles.has(item.id)}
          selectedId={selectedId} onSelect={chooseArticle}>{children}</ArticleNavigation>;
      })}</div>
    </section>;
  });

  const articleHtmlReady = Boolean(articlePayload && selectedArticle && articlePayload.articleId === selectedArticle.id);
  const currentMarkdown = articleHtmlReady ? articlePayload.markdown : "";
  return <div className="documentation-layout">
    <nav ref={navigationRef} id="documentation-navigation" className="documentation-navigation" aria-label="Documentation articles">{categories}</nav>
    <main ref={contentRef} id="documentation-content" className="documentation-content" aria-label="Article" tabIndex={-1}>
      {loadError ? <div id="documentation-load-error" className="documentation-load-error" role="alert"><p>Documentation could not be loaded.</p><button id="documentation-retry" type="button" className="banner-image-button" onClick={() => { setLoadError(false); setAttempt(value => value + 1); }}>Try again</button></div>
        : selectedArticle ? <>
          {articleHtmlReady && <MarkdownView id="documentation-article" className="documentation-article markdown-content"
            aria-label={selectedArticle.name} markdown={currentMarkdown} images={articlePayload.images} videos={articlePayload.videos} />}
          {(!articleHtmlReady || articleBusy) && <p className="documentation-article-loading" role="status">Loading article…</p>}
        </> : catalog ? <p className="documentation-article">No documentation articles are available.</p>
          : visible ? <p className="documentation-article-loading" role="status">Loading documentation…</p> : null}
    </main>
  </div>;
}
