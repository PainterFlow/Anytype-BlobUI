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
    fullscreen: '<path d="M3 8V3h5M12 3h5v5M17 12v5h-5M8 17H3v-5"/>',
    fullscreenExit: '<path d="M8 3v5H3M12 8h5V3M17 12h-5v5M8 17v-5H3"/>',
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
    video.controls = false;
    video.setAttribute("controlslist", "nofullscreen");
    video.setAttribute("webkit-playsinline", "");
    video.setAttribute("aria-label", host.getAttribute("aria-label"));
    const playButton = doc.createElement("button");
    playButton.type = "button";
    playButton.className = "markdown-video-overlay";
    playButton.innerHTML = icon("play");
    playButton.setAttribute("aria-label", "Play");
    playButton.title = "Play";
    const controls = doc.createElement("span");
    controls.className = "markdown-video-controls";
    const status = doc.createElement("span");
    status.className = "markdown-video-status";
    status.setAttribute("role", "status");
    status.textContent = "Loading video…";
    host.append(video, playButton, controls, status);
    slot.replaceWith(host);
    let disposed = false, timer = null, statusTimer = null, url = null, visible = false, manualPause = false, completed = false, requested = false, manualPlayback = false;
    let progressFrame = null, seekScrubbing = false, volumeScrubbing = false, seekPending = false;
    let playButtonState = null, muteButtonState = null;
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
    const fullscreenButton = button("Enter fullscreen", "fullscreen", toggleFullscreen);
    fullscreenButton.hidden = typeof host.requestFullscreen !== "function" && typeof host.webkitRequestFullscreen !== "function";
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
    on(playButton, "click", () => { toggle(); reveal(); });
    function seek(seconds) { if (Number.isFinite(video.duration)) video.currentTime = Math.max(0, Math.min(video.duration, seconds)); }
    const seekbar = doc.createElement("input");
    seekbar.type = "range"; seekbar.min = "0"; seekbar.max = "0"; seekbar.step = "0.01"; seekbar.value = "0";
    seekbar.setAttribute("aria-label", "Seek video"); seekbar.className = "markdown-video-seek";
    on(seekbar, "input", () => {
      const target = Number(seekbar.value);
      const previous = video.currentTime;
      seek(target);
      seekPending = target !== previous;
      updateRangeProgress(seekbar);
    });
    const clock = doc.createElement("span"); clock.className = "markdown-video-time";
    controls.append(seekbar, clock);
    const mute = button("Unmute", "mute", () => { video.muted = !video.muted; update(); });
    const volume = doc.createElement("input");
    volume.type = "range"; volume.min = "0"; volume.max = "1"; volume.step = "0.05"; volume.value = "1";
    volume.setAttribute("aria-label", "Volume"); volume.className = "markdown-video-volume";
    on(volume, "input", () => {
      video.volume = Number(volume.value);
      video.muted = video.volume === 0;
      updateRangeProgress(volume);
    });
    controls.append(volume);
    function updateRangeProgress(input) {
      const min = Number(input.min) || 0;
      const max = Number(input.max) || 0;
      const value = Number(input.value) || 0;
      const ratio = max > min ? Math.max(0, Math.min(1, (value - min) / (max - min))) : 0;
      const width = input.getBoundingClientRect().width;
      const thumbRadius = Math.min(6, width / 2);
      const fillEnd = thumbRadius + Math.max(0, width - thumbRadius * 2) * ratio;
      input.style.setProperty("--range-fill-end", `${fillEnd}px`);
    }
    const scrubKeys = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);
    function bindScrubbing(input, setScrubbing) {
      const start = () => setScrubbing(true);
      const finish = () => { setScrubbing(false); update(); };
      on(input, "pointerdown", start);
      on(input, "keydown", event => { if (scrubKeys.has(event.key)) start(); });
      on(doc, "pointerup", finish);
      on(doc, "pointercancel", finish);
      on(doc, "keyup", event => { if (scrubKeys.has(event.key) && doc.activeElement === input) finish(); });
      on(input, "blur", finish);
      on(input, "lostpointercapture", finish);
    }
    bindScrubbing(seekbar, value => { seekScrubbing = value; });
    bindScrubbing(volume, value => { volumeScrubbing = value; });
    const rangeObserver = typeof root.ResizeObserver === "function"
      ? new root.ResizeObserver(() => { updateRangeProgress(seekbar); updateRangeProgress(volume); })
      : null;
    rangeObserver?.observe(seekbar);
    rangeObserver?.observe(volume);
    on(root, "resize", () => { updateRangeProgress(seekbar); updateRangeProgress(volume); });
    function updateTimeline() {
      const duration = String(Number.isFinite(video.duration) ? video.duration : 0);
      if (seekbar.max !== duration) seekbar.max = duration;
      const currentTime = String(video.currentTime || 0);
      if (!seekScrubbing && !seekPending && !video.seeking && seekbar.value !== currentTime) seekbar.value = currentTime;
      updateRangeProgress(seekbar);
      const displayedTime = seekScrubbing || seekPending ? Number(seekbar.value) : video.currentTime;
      const valueText = `${time(displayedTime)} of ${time(video.duration)}`;
      if (seekbar.getAttribute("aria-valuetext") !== valueText) seekbar.setAttribute("aria-valuetext", valueText);
      const clockText = `${time(video.currentTime)} / ${time(video.duration)}`;
      if (clock.textContent !== clockText) clock.textContent = clockText;
    }
    function update() {
      const nextPlayButtonState = video.paused ? "play" : "pause";
      if (playButtonState !== nextPlayButtonState) {
        playButton.innerHTML = icon(nextPlayButtonState);
        playButton.setAttribute("aria-label", video.paused ? "Play" : "Pause");
        playButton.title = video.paused ? "Play" : "Pause";
        playButtonState = nextPlayButtonState;
      }
      const nextMuteButtonState = video.muted ? "mute" : "volume";
      if (muteButtonState !== nextMuteButtonState) {
        mute.setAttribute("aria-label", video.muted ? "Unmute" : "Mute");
        mute.setAttribute("aria-pressed", String(!video.muted));
        mute.title = video.muted ? "Unmute" : "Mute";
        mute.innerHTML = icon(nextMuteButtonState);
        muteButtonState = nextMuteButtonState;
      }
      const volumeValue = String(video.muted ? 0 : video.volume);
      if (!volumeScrubbing && volume.value !== volumeValue) volume.value = volumeValue;
      updateRangeProgress(volume);
      updateTimeline();
      updateFullscreen();
    }
    function fullscreenElement() { return doc.fullscreenElement || doc.webkitFullscreenElement || doc.webkitCurrentFullScreenElement || null; }
    function isFullscreen() { return fullscreenElement() === host || fullscreenElement() === video; }
    function updateFullscreen() {
      const active = isFullscreen();
      const nextIcon = active ? "fullscreenExit" : "fullscreen";
      fullscreenButton.setAttribute("aria-label", active ? "Exit fullscreen" : "Enter fullscreen");
      fullscreenButton.setAttribute("aria-pressed", String(active));
      fullscreenButton.title = active ? "Exit fullscreen" : "Enter fullscreen";
      if (fullscreenButton.dataset.icon !== nextIcon) {
        fullscreenButton.innerHTML = icon(nextIcon);
        fullscreenButton.dataset.icon = nextIcon;
      }
    }
    function temporaryStatus(message) {
      clearTimeout(statusTimer);
      status.textContent = message;
      statusTimer = setTimeout(() => {
        statusTimer = null;
        if (!disposed && status.textContent === message) status.textContent = "";
      }, 2800);
    }
    function reportFullscreenFailure() {
      if (!disposed) temporaryStatus("Fullscreen is unavailable in this window.");
    }
    function toggleFullscreen() {
      const current = isFullscreen();
      const action = current
        ? doc.exitFullscreen || doc.webkitExitFullscreen
        : host.requestFullscreen || host.webkitRequestFullscreen;
      if (typeof action !== "function") {
        fullscreenButton.hidden = true;
        reportFullscreenFailure();
        return;
      }
      try {
        const result = action.call(current ? doc : host);
        if (result && typeof result.catch === "function") result.catch(reportFullscreenFailure);
      } catch { reportFullscreenFailure(); }
      reveal();
    }
    function active() { return visible && host.isConnected && !doc.hidden && host.getClientRects().length > 0 && !host.closest("[hidden], [inert], [aria-hidden='true']"); }
    function stopProgressUpdates() {
      if (progressFrame === null) return;
      root.cancelAnimationFrame(progressFrame);
      progressFrame = null;
    }
    function queueProgressUpdate() {
      if (disposed || video.paused || video.ended || !active()) { stopProgressUpdates(); return; }
      if (progressFrame !== null) return;
      progressFrame = root.requestAnimationFrame(() => {
        progressFrame = null;
        if (disposed || video.paused || video.ended || !active()) return;
        updateTimeline();
        queueProgressUpdate();
      });
    }
    function sync() {
      if (!active()) { video.pause(); stopProgressUpdates(); }
      else if (autoplay && !manualPause && !completed && !video.ended && video.paused) play();
      if (!video.paused) queueProgressUpdate();
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
    on(host, "dblclick", event => { event.preventDefault(); event.stopPropagation(); });
    on(video, "contextmenu", event => event.preventDefault());
    on(host, "keydown", event => {
      if (event.target !== host && event.target !== video) return;
      if (event.key === " " || event.key === "k") { event.preventDefault(); toggle(); }
      else if (event.key === "m") { video.muted = !video.muted; update(); }
      reveal();
    });
    on(video, "click", () => { toggle(); reveal(); });
    for (const type of ["timeupdate", "durationchange", "volumechange"]) on(video, type, update);
    on(video, "seeking", () => { seekPending = true; update(); });
    on(video, "seeked", () => { seekPending = false; update(); });
    on(video, "play", () => { update(); queueProgressUpdate(); });
    on(video, "pause", () => { stopProgressUpdates(); update(); });
    on(video, "playing", queueProgressUpdate);
    on(video, "loadedmetadata", () => { status.textContent = ""; update(); sync(); });
    for (const type of ["fullscreenchange", "webkitfullscreenchange"]) on(doc, type, updateFullscreen);
    for (const type of ["fullscreenerror", "webkitfullscreenerror"]) on(doc, type, reportFullscreenFailure);
    on(video, "play", () => { status.textContent = ""; });
    on(video, "ended", () => { completed = true; stopProgressUpdates(); update(); });
    on(video, "error", () => { status.textContent = "This video could not be played."; });
    on(doc, "visibilitychange", sync);
    const observer = new IntersectionObserver(entries => { visible = entries[0].intersectionRatio >= 0.5; sync(); }, { threshold: [0, 0.5, 1] });
    observer.observe(host);
    const controller = {
      host, sync,
      stopAutoplay() { if (!manualPlayback && !video.paused) video.pause(); },
      destroy() {
        if (disposed) return;
        if (isFullscreen()) {
          const exit = doc.exitFullscreen || doc.webkitExitFullscreen;
          try { const result = exit?.call(doc); if (result && typeof result.catch === "function") result.catch(() => {}); } catch {}
        }
        disposed = true; clearTimeout(timer); clearTimeout(statusTimer); stopProgressUpdates(); observer.disconnect(); rangeObserver?.disconnect(); listeners.forEach(remove => remove());
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
  const lifecycle = new MutationObserver(records => {
    if (!records.some(record => !(record.type === "attributes" && record.attributeName === "style" && record.target.matches(".markdown-video-controls input[type='range']")))) return;
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
