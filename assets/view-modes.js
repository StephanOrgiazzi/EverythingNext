(() => {
  "use strict";

  const STORAGE_KEY = "everything-modern-view-mode";
  const LOGICAL_ROW_HEIGHT = 34;
  const LOGICAL_OVERSCAN = 8;
  const GRID_PADDING = 10;
  const GRID_GAP = 8;
  const MODES = {
    details: {
      label: "Détails",
      itemHeight: LOGICAL_ROW_HEIGHT,
      minWidth: Number.POSITIVE_INFINITY,
      maxColumns: 1,
      icon: "M4 5h3v3H4V5Zm5 0h11v3H9V5ZM4 10.5h3v3H4v-3Zm5 0h11v3H9v-3ZM4 16h3v3H4v-3Zm5 0h11v3H9v-3Z",
    },
    small: {
      label: "Petites icônes",
      itemHeight: 46,
      minWidth: 360,
      maxColumns: 2,
      icon: "M3 4h7v7H3V4Zm2 2v3h3V6H5Zm9-2h7v7h-7V4Zm2 2v3h3V6h-3ZM3 14h7v7H3v-7Zm2 2v3h3v-3H5Zm9-2h7v7h-7v-7Zm2 2v3h3v-3h-3Z",
    },
    medium: {
      label: "Icônes moyennes",
      itemHeight: 132,
      minWidth: 210,
      maxColumns: 5,
      icon: "M3 3h8v8H3V3Zm2 2v4h4V5H5Zm8-2h8v8h-8V3Zm2 2v4h4V5h-4ZM3 13h8v8H3v-8Zm2 2v4h4v-4H5Zm8-2h8v8h-8v-8Zm2 2v4h4v-4h-4Z",
    },
    large: {
      label: "Grandes icônes",
      itemHeight: 184,
      minWidth: 250,
      maxColumns: 4,
      icon: "M2 2h9v9H2V2Zm2 2v5h5V4H4Zm9-2h9v9h-9V2Zm2 2v5h5V4h-5ZM2 13h9v9H2v-9Zm2 2v5h5v-5H4Zm9-2h9v9h-9v-9Zm2 2v5h5v-5h-5Z",
    },
  };

  let state = null;
  let mountObserver = null;

  const svg = (path, className = "") =>
    `<svg class="${className}" viewBox="0 0 24 24" aria-hidden="true"><path d="${path}"></path></svg>`;

  function readStoredMode() {
    try {
      const mode = window.localStorage.getItem(STORAGE_KEY);
      return Object.hasOwn(MODES, mode) ? mode : "details";
    } catch (_) {
      return "details";
    }
  }

  function storeMode(mode) {
    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch (_) {
      // Storage can be disabled without affecting the view switcher.
    }
  }

  function beginMounting() {
    if (tryMount()) return;
    mountObserver = new MutationObserver(() => {
      if (tryMount() && mountObserver) {
        mountObserver.disconnect();
        mountObserver = null;
      }
    });
    mountObserver.observe(document.documentElement, { childList: true, subtree: true });
  }

  function tryMount() {
    if (state || document.querySelector("[data-view-switcher]")) return Boolean(state);

    const commandBar = document.querySelector(".command-bar");
    const panel = document.querySelector(".results-panel");
    const originalScroll = panel?.querySelector(".results-scroll");
    const originalCanvas = originalScroll?.querySelector(".virtual-canvas");
    const statusbar = panel?.querySelector(".statusbar");
    if (!commandBar || !panel || !originalScroll || !originalCanvas || !statusbar) return false;

    const spacer = document.createElement("span");
    spacer.className = "command-bar-spacer";
    spacer.setAttribute("aria-hidden", "true");

    const switcher = document.createElement("div");
    switcher.className = "view-switcher";
    switcher.dataset.viewSwitcher = "";

    const trigger = document.createElement("button");
    trigger.type = "button";
    trigger.className = "command-button view-button";
    trigger.setAttribute("aria-haspopup", "menu");
    trigger.setAttribute("aria-expanded", "false");

    const menu = document.createElement("div");
    menu.className = "view-menu";
    menu.setAttribute("role", "menu");
    menu.hidden = true;

    for (const [mode, config] of Object.entries(MODES)) {
      const option = document.createElement("button");
      option.type = "button";
      option.className = "view-option";
      option.dataset.mode = mode;
      option.setAttribute("role", "menuitemradio");
      option.innerHTML = `${svg(config.icon, "view-option-icon")}<span>${config.label}</span><span class="view-option-check" aria-hidden="true">✓</span>`;
      option.addEventListener("click", (event) => {
        event.stopPropagation();
        applyMode(mode);
        setMenuOpen(false);
        trigger.focus();
      });
      menu.append(option);
    }

    trigger.addEventListener("click", (event) => {
      event.stopPropagation();
      setMenuOpen(menu.hidden);
    });
    trigger.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      setMenuOpen(true);
      focusMenuOption(event.key === "ArrowDown" ? "active" : "last");
    });
    menu.addEventListener("keydown", onMenuKeyDown);

    switcher.append(trigger, menu);
    commandBar.append(spacer, switcher);

    const overlay = document.createElement("div");
    overlay.className = "icon-results-scroll";
    overlay.tabIndex = 0;
    overlay.setAttribute("role", "grid");
    overlay.setAttribute("aria-label", "Résultats de recherche en mode icônes");
    overlay.hidden = true;

    const overlayCanvas = document.createElement("div");
    overlayCanvas.className = "icon-virtual-canvas";
    overlay.append(overlayCanvas);
    panel.insertBefore(overlay, statusbar);

    state = {
      commandBar,
      panel,
      originalScroll,
      originalCanvas,
      overlay,
      overlayCanvas,
      trigger,
      menu,
      mode: "details",
      columns: 1,
      renderQueued: false,
      syncingFromOverlay: false,
      syncingFromOriginal: false,
      rowMap: new Map(),
      tileMap: new Map(),
    };

    const resultsObserver = new MutationObserver(scheduleRender);
    resultsObserver.observe(originalCanvas, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["class", "style", "src"],
    });

    overlay.addEventListener("scroll", onOverlayScroll, { passive: true });
    originalScroll.addEventListener("scroll", onOriginalScroll, { passive: true });
    overlay.addEventListener("click", onOverlayBackgroundClick);
    overlay.addEventListener("keydown", onOverlayKeyDown);

    document.addEventListener("click", (event) => {
      if (!switcher.contains(event.target)) setMenuOpen(false);
    });
    document.addEventListener("keydown", (event) => {
      if (event.key !== "Escape" || !state || state.menu.hidden) return;
      event.preventDefault();
      event.stopPropagation();
      setMenuOpen(false);
      state.trigger.focus();
    });

    if (window.ResizeObserver) {
      const resizeObserver = new ResizeObserver(() => recalculateLayout(true));
      resizeObserver.observe(panel);
      resizeObserver.observe(overlay);
    } else {
      window.addEventListener("resize", () => recalculateLayout(true));
    }

    applyMode(readStoredMode(), false);
    return true;
  }

  function menuOptions() {
    return state ? Array.from(state.menu.querySelectorAll(".view-option")) : [];
  }

  function focusMenuOption(target) {
    const options = menuOptions();
    if (options.length === 0) return;
    if (target === "first") options[0].focus();
    else if (target === "last") options.at(-1).focus();
    else (options.find((option) => option.classList.contains("active")) || options[0]).focus();
  }

  function onMenuKeyDown(event) {
    const options = menuOptions();
    if (options.length === 0) return;
    const current = Math.max(0, options.indexOf(document.activeElement));

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const delta = event.key === "ArrowDown" ? 1 : -1;
      options[(current + delta + options.length) % options.length].focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      focusMenuOption("first");
    } else if (event.key === "End") {
      event.preventDefault();
      focusMenuOption("last");
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setMenuOpen(false);
      state?.trigger.focus();
    } else if (event.key === "Tab") {
      setMenuOpen(false);
    }
  }

  function setMenuOpen(open) {
    if (!state) return;
    state.menu.hidden = !open;
    state.trigger.setAttribute("aria-expanded", String(open));
  }

  function updateModeUi() {
    if (!state) return;
    const config = MODES[state.mode];
    state.trigger.innerHTML = `${svg(config.icon, "view-button-icon")}<span class="view-button-label">${config.label}</span>${svg("m7 9 5 5 5-5 1.4 1.4-6.4 6.4-6.4-6.4L7 9Z", "view-chevron")}`;
    state.trigger.title = `Affichage : ${config.label}`;
    state.trigger.setAttribute("aria-label", `Choisir le mode d’affichage. Mode actuel : ${config.label}`);
    for (const option of state.menu.querySelectorAll(".view-option")) {
      const active = option.dataset.mode === state.mode;
      option.classList.toggle("active", active);
      option.setAttribute("aria-checked", String(active));
    }
  }

  function firstVisibleIndex() {
    if (!state || state.mode === "details") {
      return state ? Math.max(0, Math.floor(state.originalScroll.scrollTop / LOGICAL_ROW_HEIGHT)) : 0;
    }
    const config = MODES[state.mode];
    return Math.max(0, Math.floor(state.overlay.scrollTop / config.itemHeight) * state.columns);
  }

  function applyMode(mode, persist = true) {
    if (!state || !Object.hasOwn(MODES, mode)) return;
    const anchorIndex = firstVisibleIndex();

    state.mode = mode;
    state.panel.dataset.viewMode = mode;
    state.panel.classList.toggle("icon-view", mode !== "details");
    state.overlay.hidden = mode === "details";
    updateModeUi();

    if (mode === "details") {
      state.originalScroll.scrollTop = anchorIndex * LOGICAL_ROW_HEIGHT;
    } else {
      recalculateLayout(false);
      const config = MODES[mode];
      state.overlay.scrollTop = Math.floor(anchorIndex / state.columns) * config.itemHeight;
      syncOriginalToOverlay();
      scheduleRender();
    }

    if (persist) storeMode(mode);
  }

  function calculateColumns() {
    if (!state || state.mode === "details") return 1;
    const config = MODES[state.mode];
    const width = state.overlay.clientWidth || state.panel.clientWidth || config.minWidth;
    const available = Math.max(config.minWidth, width - GRID_PADDING * 2);
    return Math.max(
      1,
      Math.min(config.maxColumns, Math.floor((available + GRID_GAP) / (config.minWidth + GRID_GAP))),
    );
  }

  function recalculateLayout(preserveAnchor) {
    if (!state || state.mode === "details") return;
    const anchorIndex = preserveAnchor ? firstVisibleIndex() : 0;
    const nextColumns = calculateColumns();
    const changed = nextColumns !== state.columns;
    state.columns = nextColumns;
    state.overlay.setAttribute("aria-colcount", String(nextColumns));

    if (changed && preserveAnchor) {
      state.overlay.scrollTop = Math.floor(anchorIndex / nextColumns) * MODES[state.mode].itemHeight;
      syncOriginalToOverlay();
    }
    scheduleRender();
  }

  function onOverlayScroll() {
    if (!state || state.mode === "details") return;
    if (!state.syncingFromOriginal) syncOriginalToOverlay();
    scheduleRender();
  }

  function syncOriginalToOverlay() {
    if (!state || state.mode === "details") return;
    const config = MODES[state.mode];
    const firstIndex = Math.floor(state.overlay.scrollTop / config.itemHeight) * state.columns;
    // The Leptos list subtracts eight rows of overscan. Offset the hidden
    // logical viewport so all visible icon cells fall inside its rendered range.
    const target = (firstIndex + LOGICAL_OVERSCAN) * LOGICAL_ROW_HEIGHT;
    if (Math.abs(state.originalScroll.scrollTop - target) < 1) return;

    state.syncingFromOverlay = true;
    state.originalScroll.scrollTop = target;
    requestAnimationFrame(() => {
      if (state) state.syncingFromOverlay = false;
    });
  }

  function onOriginalScroll() {
    if (!state || state.mode === "details" || state.syncingFromOverlay) return;
    const config = MODES[state.mode];
    const logicalStart = Math.max(
      0,
      Math.floor(state.originalScroll.scrollTop / LOGICAL_ROW_HEIGHT) - LOGICAL_OVERSCAN,
    );
    const target = Math.floor(logicalStart / state.columns) * config.itemHeight;
    if (Math.abs(state.overlay.scrollTop - target) < 1) return;

    state.syncingFromOriginal = true;
    state.overlay.scrollTop = target;
    requestAnimationFrame(() => {
      if (state) state.syncingFromOriginal = false;
    });
  }

  function scheduleRender() {
    if (!state || state.mode === "details" || state.renderQueued) return;
    state.renderQueued = true;
    requestAnimationFrame(() => {
      if (!state) return;
      state.renderQueued = false;
      renderTiles();
    });
  }

  function rowIndex(row) {
    const transform = row.style.transform || "";
    const match = transform.match(/translateY\(([-\d.]+)px\)/);
    if (!match) return null;
    return Math.max(0, Math.round(Number.parseFloat(match[1]) / LOGICAL_ROW_HEIGHT));
  }

  function rowSignature(row) {
    if (row.classList.contains("skeleton-row")) return `${state.mode}|skeleton`;
    const nameCell = row.querySelector(".cell.col-name");
    const icon = nameCell?.querySelector(".file-icon")?.outerHTML || "";
    const name = nameCell?.querySelector(".file-name")?.textContent?.trim() || "";
    const path = row.querySelector(".cell.col-path")?.textContent?.trim() || "";
    const size = row.querySelector(".cell.col-size")?.textContent?.trim() || "";
    const date = row.querySelector(".cell.col-date")?.textContent?.trim() || "";
    return [state.mode, name, path, size, date, icon].join("\u001f");
  }

  function createTile(index) {
    const tile = document.createElement("div");
    tile.dataset.index = String(index);
    tile.setAttribute("role", "gridcell");
    tile.addEventListener("click", (event) => forwardMouseEvent(index, "click", event));
    tile.addEventListener("dblclick", (event) => forwardMouseEvent(index, "dblclick", event));
    tile.addEventListener("contextmenu", (event) => forwardMouseEvent(index, "contextmenu", event));
    return tile;
  }

  function renderTiles() {
    if (!state || state.mode === "details") return;

    const config = MODES[state.mode];
    const width = state.overlay.clientWidth || state.panel.clientWidth;
    const cellWidth = Math.max(
      120,
      (width - GRID_PADDING * 2 - GRID_GAP * (state.columns - 1)) / state.columns,
    );

    const declaredHeight = Number.parseFloat(state.originalCanvas.style.height || "0");
    let total = Number.isFinite(declaredHeight)
      ? Math.max(0, Math.round(declaredHeight / LOGICAL_ROW_HEIGHT))
      : 0;

    const orderedTiles = [];
    const visibleIndexes = new Set();
    state.rowMap.clear();

    for (const row of state.originalCanvas.querySelectorAll(".result-row")) {
      const index = rowIndex(row);
      if (index === null) continue;
      total = Math.max(total, index + 1);
      visibleIndexes.add(index);
      state.rowMap.set(index, row);

      let tile = state.tileMap.get(index);
      if (!tile) {
        tile = createTile(index);
        state.tileMap.set(index, tile);
      }

      tile.className = `icon-result icon-result-${state.mode}`;
      tile.setAttribute("aria-rowindex", String(Math.floor(index / state.columns) + 1));
      tile.setAttribute("aria-colindex", String((index % state.columns) + 1));
      tile.setAttribute("aria-selected", String(row.classList.contains("selected")));
      tile.classList.toggle("selected", row.classList.contains("selected"));
      tile.classList.toggle("focused", row.classList.contains("focused"));
      tile.classList.toggle("skeleton-tile", row.classList.contains("skeleton-row"));

      const column = index % state.columns;
      const visualRow = Math.floor(index / state.columns);
      const x = GRID_PADDING + column * (cellWidth + GRID_GAP);
      const y = GRID_PADDING + visualRow * config.itemHeight;
      tile.style.width = `${cellWidth}px`;
      tile.style.height = `${config.itemHeight - GRID_GAP}px`;
      tile.style.transform = `translate3d(${x}px, ${y}px, 0)`;

      const signature = rowSignature(row);
      if (tile.viewSignature !== signature) {
        if (row.classList.contains("skeleton-row")) populateSkeletonTile(tile);
        else populateTile(tile, row);
        tile.viewSignature = signature;
      }
      orderedTiles.push(tile);
    }

    for (const [index, tile] of state.tileMap) {
      if (visibleIndexes.has(index)) continue;
      tile.remove();
      state.tileMap.delete(index);
    }

    const children = state.overlayCanvas.children;
    const orderChanged =
      children.length !== orderedTiles.length ||
      orderedTiles.some((tile, index) => children[index] !== tile);
    if (orderChanged) state.overlayCanvas.replaceChildren(...orderedTiles);

    const rowCount = total === 0 ? 0 : Math.ceil(total / state.columns);
    state.overlayCanvas.style.height = `${GRID_PADDING * 2 + rowCount * config.itemHeight}px`;
    state.overlay.setAttribute("aria-rowcount", String(rowCount));
  }

  function populateSkeletonTile(tile) {
    const icon = document.createElement("span");
    icon.className = "icon-tile-skeleton icon-tile-skeleton-icon";
    const label = document.createElement("span");
    label.className = "icon-tile-skeleton icon-tile-skeleton-label";
    tile.removeAttribute("aria-label");
    tile.replaceChildren(icon, label);
  }

  function populateTile(tile, row) {
    const nameCell = row.querySelector(".cell.col-name");
    const icon = nameCell?.querySelector(".file-icon")?.cloneNode(true);
    const name = nameCell?.querySelector(".file-name")?.textContent?.trim() || "";
    const path = row.querySelector(".cell.col-path")?.textContent?.trim() || "";
    const size = row.querySelector(".cell.col-size")?.textContent?.trim() || "";
    const date = row.querySelector(".cell.col-date")?.textContent?.trim() || "";

    const visual = document.createElement("div");
    visual.className = "icon-result-visual";
    if (icon) visual.append(icon);

    const text = document.createElement("div");
    text.className = "icon-result-text";
    const nameElement = document.createElement("span");
    nameElement.className = "icon-result-name";
    nameElement.textContent = name;
    const metadata = document.createElement("span");
    metadata.className = "icon-result-metadata";
    metadata.textContent = state.mode === "small" ? path : [size, date].filter(Boolean).join(" · ");
    text.append(nameElement, metadata);

    tile.title = path ? `${name}\n${path}` : name;
    tile.setAttribute("aria-label", path ? `${name}, ${path}` : name);
    tile.replaceChildren(visual, text);
  }

  function forwardMouseEvent(index, type, event) {
    if (!state) return;
    event.preventDefault();
    event.stopPropagation();
    const row = state.rowMap.get(index);
    if (!row) return;

    row.dispatchEvent(
      new MouseEvent(type, {
        bubbles: true,
        cancelable: true,
        view: window,
        button: event.button,
        buttons: event.buttons,
        clientX: event.clientX,
        clientY: event.clientY,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        metaKey: event.metaKey,
      }),
    );
    requestAnimationFrame(() => {
      state?.overlay.focus({ preventScroll: true });
      scheduleRender();
    });
  }

  function onOverlayBackgroundClick(event) {
    if (!state || event.target !== state.overlayCanvas) return;
    state.originalScroll.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    state.overlay.focus({ preventScroll: true });
  }

  function onOverlayKeyDown(event) {
    if (!state || state.mode === "details") return;

    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      event.stopPropagation();
      openFocusedTileContextMenu();
      return;
    }

    if (event.key === "Home" || event.key === "End") {
      requestAnimationFrame(ensureFocusedTileVisible);
      return;
    }

    let legacyKey = null;
    let repetitions = 1;
    if (event.key === "ArrowRight") legacyKey = "ArrowDown";
    if (event.key === "ArrowLeft") legacyKey = "ArrowUp";
    if (event.key === "ArrowDown") {
      legacyKey = "ArrowDown";
      repetitions = state.columns;
    }
    if (event.key === "ArrowUp") {
      legacyKey = "ArrowUp";
      repetitions = state.columns;
    }
    if (event.key === "PageDown" || event.key === "PageUp") {
      legacyKey = event.key === "PageDown" ? "ArrowDown" : "ArrowUp";
      const rows = Math.max(1, Math.floor(state.overlay.clientHeight / MODES[state.mode].itemHeight));
      repetitions = rows * state.columns;
    }
    if (!legacyKey) return;

    event.preventDefault();
    event.stopPropagation();
    for (let index = 0; index < repetitions; index += 1) {
      state.originalScroll.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: legacyKey,
          bubbles: true,
          cancelable: true,
          ctrlKey: event.ctrlKey,
          shiftKey: event.shiftKey,
          altKey: event.altKey,
          metaKey: event.metaKey,
        }),
      );
    }
    requestAnimationFrame(ensureFocusedTileVisible);
  }

  function openFocusedTileContextMenu() {
    if (!state) return;
    const tile = state.overlayCanvas.querySelector(".icon-result.focused");
    if (!tile) return;
    const rect = tile.getBoundingClientRect();
    tile.dispatchEvent(
      new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        view: window,
        button: 2,
        clientX: Math.round(rect.left + Math.min(180, rect.width / 2)),
        clientY: Math.round(rect.top + Math.min(34, rect.height)),
      }),
    );
  }

  function ensureFocusedTileVisible() {
    if (!state || state.mode === "details") return;
    const focused = state.originalCanvas.querySelector(".result-row.focused");
    const index = focused ? rowIndex(focused) : null;
    if (index === null) return;

    const config = MODES[state.mode];
    const row = Math.floor(index / state.columns);
    const top = GRID_PADDING + row * config.itemHeight;
    const bottom = top + config.itemHeight - GRID_GAP;
    const viewportTop = state.overlay.scrollTop;
    const viewportBottom = viewportTop + state.overlay.clientHeight;
    if (top < viewportTop) state.overlay.scrollTop = Math.max(0, top - GRID_PADDING);
    else if (bottom > viewportBottom) {
      state.overlay.scrollTop = bottom - state.overlay.clientHeight + GRID_PADDING;
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", beginMounting, { once: true });
  } else {
    beginMounting();
  }
})();
