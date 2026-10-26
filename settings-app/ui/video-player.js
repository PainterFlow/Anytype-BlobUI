(function (root) {
  "use strict";
  const BlobUI = root.BlobUI = root.BlobUI || {};
  const players = new Map();
  const maxBytes = 64 * 1024 * 1024;
  let autoplay = true;
  const time = seconds => Number.isFinite(seconds) ? `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, "0")}` : "0:00";

  const icons = {
    play: '<path d="m7 4 10 6-10 6Z" fill="currentColor" stroke="none"/>',
    pause: '<path d="M7 4v12M13 4v12"/>',
    mute: '<path d="M9 4 5 7H2v6h3l4 3ZM13 7l5 6m0-6-5 6"/>',
    volume: '<path d="M9 4 5 7H2v6h3l4 3ZM13 6a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/>',
    fullscreen: '<path d="M7 3H3v4m10-4h4v4M3 13v4h4m10-4v4h-4"/>',
  };
  const icon = name => '<svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">' + icons[name] + '</svg>';

  function attach(slot, media) {
    const doc = slot.ownerDocument;
    const host = doc.createElement("span");
    host.className = "markdown-video";
    host.tabIndex = 0;
    host.setAttribute("role", "group");
    host.setAttribute("aria-label", slot.getAttribute("aria-label") || media.alt || "Video");
    const video = doc.createElement("video");
    video.preload = "metadata";
    video.muted = true;
    video.playsInline = true;
    video.setAttribute("aria-label", host.getAttribute("aria-label"));
    const controls = doc.createElement("span");
    controls.className = "markdown-video-controls";
    const status = doc.createElement("span");
    status.className = "markdown-video-status";
    status.setAttribute("role", "status");
    status.textContent = "Loading video…";
    host.append(video, controls, status);
    slot.replaceWith(host);
    let disposed = false, timer = null, url = null, visible = false, manualPause = false, completed = false, requested = false, manualPlayback = false;
    const listeners = [];
    function on(node, type, handler) { node.addEventListener(type, handler); listeners.push(() => node.removeEventListener(type, handler)); }
    function button(label, symbol, action) {
      const element = doc.createElement("button");
      element.type = "button";
      if (icons[symbol]) element.innerHTML = icon(symbol); else element.textContent = symbol;
      element.setAttribute("aria-label", label);
      element.title = label;
      on(element, "click", action);
      controls.append(element);
      return element;
    }
    function play(manual = false) {
      if (disposed || !video.src || requested) return;
      if (manual) { manualPause = false; completed = false; manualPlayback = true; }
      requested = true;
      Promise.resolve(video.play()).catch(() => {
        if (!disposed) { status.textContent = "Select Play to start the video."; manualPause = true; }
      }).finally(() => { requested = false; if (!disposed && !active()) video.pause(); });
    }
    function toggle() {
      if (video.paused) play(true);
      else { manualPause = true; video.pause(); }
    }
    function seek(seconds) { if (Number.isFinite(video.duration)) video.currentTime = Math.max(0, Math.min(video.duration, seconds)); }
    const playButton = button("Play", "play", toggle);
    button("Back 10 seconds", "−10", () => seek(video.currentTime - 10));
    button("Forward 10 seconds", "+10", () => seek(video.currentTime + 10));
    const seekbar = doc.createElement("input");
    seekbar.type = "range"; seekbar.min = "0"; seekbar.max = "0"; seekbar.step = "0.1"; seekbar.value = "0";
    seekbar.setAttribute("aria-label", "Seek video"); seekbar.className = "markdown-video-seek";
    on(seekbar, "input", () => seek(Number(seekbar.value)));
    const clock = doc.createElement("span"); clock.className = "markdown-video-time";
    controls.append(seekbar, clock);
    const mute = button("Unmute", "mute", () => { video.muted = !video.muted; update(); });
    const volume = doc.createElement("input");
    volume.type = "range"; volume.min = "0"; volume.max = "1"; volume.step = "0.05"; volume.value = "1";
    volume.setAttribute("aria-label", "Volume"); volume.className = "markdown-video-volume";
    on(volume, "input", () => { video.volume = Number(volume.value); video.muted = video.volume === 0; });
    controls.append(volume);
    const fullscreen = button("Enter fullscreen", "fullscreen", () => {
      const operation = doc.fullscreenElement === host ? doc.exitFullscreen?.() : host.requestFullscreen?.();
      operation?.catch(() => { status.textContent = "Fullscreen is unavailable."; });
    });
    function update() {
      playButton.innerHTML = icon(video.paused ? "play" : "pause");
      playButton.setAttribute("aria-label", video.paused ? "Play" : "Pause");
      playButton.title = video.paused ? "Play" : "Pause";
      mute.setAttribute("aria-label", video.muted ? "Unmute" : "Mute");
      mute.setAttribute("aria-pressed", String(!video.muted));
      mute.title = video.muted ? "Unmute" : "Mute";
      mute.innerHTML = icon(video.muted ? "mute" : "volume");
      volume.value = String(video.muted ? 0 : video.volume);
      seekbar.max = String(Number.isFinite(video.duration) ? video.duration : 0);
      seekbar.value = String(video.currentTime || 0);
      seekbar.setAttribute("aria-valuetext", `${time(video.currentTime)} of ${time(video.duration)}`);
      clock.textContent = `${time(video.currentTime)} / ${time(video.duration)}`;
    }
    function active() { return visible && host.isConnected && !doc.hidden && host.getClientRects().length > 0 && !host.closest("[hidden], [inert], [aria-hidden='true']"); }
    function sync() {
      if (!active()) video.pause();
      else if (autoplay && !manualPause && !completed && video.paused) play();
    }
    function reveal() {
      clearTimeout(timer); host.classList.add("controls-visible");
      timer = setTimeout(() => { if (!host.contains(doc.activeElement)) host.classList.remove("controls-visible"); }, 2000);
    }
    on(host, "pointermove", reveal);
    on(host, "pointerdown", reveal);
    on(host, "pointerleave", () => { clearTimeout(timer); if (!host.contains(doc.activeElement)) host.classList.remove("controls-visible"); });
    on(host, "focusin", reveal);
    on(host, "focusout", reveal);
    on(host, "keydown", event => {
      if (event.target !== host && event.target !== video) return;
      if (event.key === " " || event.key === "k") { event.preventDefault(); toggle(); }
      else if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); seek(video.currentTime + (event.key === "ArrowLeft" ? -10 : 10)); }
      else if (event.key === "m") { video.muted = !video.muted; update(); }
      reveal();
    });
    on(video, "click", () => { toggle(); reveal(); });
    for (const type of ["play", "pause", "timeupdate", "durationchange", "volumechange"]) on(video, type, update);
    on(video, "loadedmetadata", () => { status.textContent = ""; update(); sync(); });
    on(video, "play", () => { status.textContent = ""; });
    on(video, "ended", () => { completed = true; update(); });
    on(video, "error", () => { status.textContent = "This video could not be played."; });
    on(doc, "visibilitychange", sync);
    on(doc, "fullscreenchange", () => { fullscreen.setAttribute("aria-label", doc.fullscreenElement === host ? "Exit fullscreen" : "Enter fullscreen"); });
    const observer = new IntersectionObserver(entries => { visible = entries[0].intersectionRatio >= 0.5; sync(); }, { threshold: [0, 0.5, 1] });
    observer.observe(host);
    const controller = {
      host, sync,
      stopAutoplay() { if (!manualPlayback && !video.paused) video.pause(); },
      destroy() {
        if (disposed) return;
        disposed = true; clearTimeout(timer); observer.disconnect(); listeners.forEach(remove => remove());
        video.pause(); video.removeAttribute("src"); video.load();
        if (url) URL.revokeObjectURL(url);
        players.delete(host);
      }
    };
    players.set(host, controller);
    (async () => {
      let blob = media.blob;
      if (!blob) {
        const match = /^data:video\/mp4;base64,([A-Za-z0-9+/]+={0,2})$/.exec(String(media.src || ""));
        if (!match || match[1].length > Math.ceil(maxBytes / 3) * 4) throw new Error("Unsupported video");
        const binary = root.atob(match[1]);
        blob = new Blob([Uint8Array.from(binary, character => character.charCodeAt(0))], { type: "video/mp4" });
      }
      if (blob.type !== "video/mp4" || !blob.size || blob.size > maxBytes) throw new Error("Unsupported video");
      const header = new Uint8Array(await blob.slice(0, 12).arrayBuffer());
      if (header.length < 12 || String.fromCharCode(...header.slice(4, 8)) !== "ftyp") throw new Error("Invalid MP4");
      if (disposed) return;
      url = URL.createObjectURL(blob); video.src = url;
    })().catch(() => { if (!disposed) status.textContent = "This video is missing or has an unsupported format or size."; });
    update();
    return controller;
  }
  // Visibility changes and replaced markdown must release even manually started players.
  const lifecycle = new MutationObserver(() => {
    for (const [host, player] of players) {
      if (!host.isConnected) player.destroy(); else player.sync();
    }
  });
  lifecycle.observe(root.document.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ["hidden", "inert", "aria-hidden", "class", "style"] });
  BlobUI.VideoPlayer = Object.freeze({
    attach,
    setAutoplay(value) { autoplay = value !== false; for (const player of players.values()) { if (!autoplay) player.stopAutoplay(); player.sync(); } },
    destroyTree(container) { for (const [host, player] of players) if (container.contains(host)) player.destroy(); },
  });
})(window);
