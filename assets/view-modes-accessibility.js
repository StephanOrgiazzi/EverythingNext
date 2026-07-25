(() => {
  "use strict";

  let panelObserver = null;
  let mountObserver = null;

  function syncInactiveGrid(panel, detailsGrid) {
    const iconMode = panel.classList.contains("icon-view");
    detailsGrid.tabIndex = iconMode ? -1 : 0;

    if (iconMode) {
      detailsGrid.setAttribute("aria-hidden", "true");
    } else {
      detailsGrid.removeAttribute("aria-hidden");
    }
  }

  function tryMount() {
    const panel = document.querySelector(".results-panel");
    const detailsGrid = panel?.querySelector(".results-scroll");
    if (!panel || !detailsGrid) return false;

    syncInactiveGrid(panel, detailsGrid);
    panelObserver = new MutationObserver(() => syncInactiveGrid(panel, detailsGrid));
    panelObserver.observe(panel, { attributes: true, attributeFilter: ["class"] });
    return true;
  }

  function beginMounting() {
    if (tryMount()) return;

    mountObserver = new MutationObserver(() => {
      if (!tryMount()) return;
      mountObserver?.disconnect();
      mountObserver = null;
    });
    mountObserver.observe(document.documentElement, { childList: true, subtree: true });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", beginMounting, { once: true });
  } else {
    beginMounting();
  }
})();
