// cargo styleguide serve --watch: reloads the page after each successful
// rebuild. The URL hash keeps the route, theme and isolated knobs.
(function () {
  var version = null;
  var banner = document.createElement("div");
  banner.style.cssText =
    "position:fixed;right:12px;bottom:12px;padding:6px 10px;border-radius:4px;" +
    "font:12px system-ui,sans-serif;color:#fff;pointer-events:none;display:none;z-index:1";
  document.body.appendChild(banner);
  function show(text, color) {
    banner.textContent = text;
    banner.style.background = color;
    banner.style.display = text ? "block" : "none";
  }
  function poll() {
    fetch("/__styleguide/reload", { cache: "no-store" })
      .then(function (response) { return response.json(); })
      .then(function (state) {
        if (version === null) version = state.version;
        if (state.version !== version) return location.reload();
        if (state.building) show("Rebuilding…", "#1978c8");
        else if (state.failed) show("Build failed; see the terminal", "#c62828");
        else show("", "");
      })
      .catch(function () { show("Disconnected from cargo styleguide", "#767676"); })
      .then(function () { setTimeout(poll, 1000); });
  }
  poll();
})();
