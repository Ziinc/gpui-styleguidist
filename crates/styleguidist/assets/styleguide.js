// gpui-styleguidist: turns each pre-rendered preview into a small playground.
// Knobs with pre-rendered values swap snapshots and rewrite "View code" with
// the chosen values; "Open live playground" embeds the hosted wasm
// styleguide, where every knob and interaction is live.
(function () {
  "use strict";
  function text(value) {
    return typeof value === "boolean" || typeof value === "number" ? String(value) : value;
  }
  // A knob value as the Rust expression "View code" shows for it.
  function literal(part, value) {
    if (part.code && typeof value === "string" && part.code[value] !== undefined) {
      return part.code[value];
    }
    if (typeof value === "boolean") return String(value);
    if (typeof value === "number") return Number.isInteger(value) ? value + ".0" : String(value);
    return JSON.stringify(value);
  }
  function copyButton(code) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "gsg-copy";
    button.textContent = "Copy";
    button.addEventListener("click", function () {
      var done = function () {
        button.textContent = "Copied";
        setTimeout(function () { button.textContent = "Copy"; }, 1500);
      };
      if (navigator.clipboard) {
        navigator.clipboard.writeText(code.textContent).then(done, function () {
          getSelection().selectAllChildren(code);
        });
      } else {
        getSelection().selectAllChildren(code);
      }
    });
    return button;
  }
  function enhance(root) {
    var data;
    try {
      data = JSON.parse(root.querySelector("script.gsg-data").textContent);
    } catch (e) {
      return;
    }
    var main = root.querySelector(".gsg-preview img");
    var code = root.querySelector(".gsg-code code");
    if (code) code.parentNode.parentNode.insertBefore(copyButton(code), code.parentNode);
    var images = {};
    root.querySelectorAll("img[data-key]").forEach(function (img) {
      images[img.dataset.theme + "\u0000" + img.dataset.key] = img;
    });
    var state = { theme: data.theme, values: {} };
    data.knobs.forEach(function (knob) {
      state.values[knob.name] = knob.default;
    });
    var bar = document.createElement("div");
    bar.className = "gsg-knobs";
    var status = document.createElement("span");
    status.className = "gsg-live";

    function key() {
      return data.knobs
        .map(function (knob) {
          return knob.name + "=" + text(state.values[knob.name]);
        })
        .join(",");
    }
    function updateCode() {
      if (!code || !data.code || !data.code.length) return;
      code.textContent = data.code
        .map(function (part) {
          return part.text !== undefined ? part.text : literal(part, state.values[part.knob]);
        })
        .join("");
    }
    function update() {
      updateCode();
      var img = images[state.theme + "\u0000" + key()];
      if (img) {
        main.src = img.src;
        main.width = img.width;
        main.height = img.height;
        main.alt = img.alt;
        status.textContent = "";
      } else {
        status.textContent = data.playground
          ? "not pre-rendered: open the live playground"
          : "not pre-rendered";
      }
      var frame = root.querySelector("iframe");
      if (frame) frame.src = playgroundUrl();
    }
    function playgroundUrl() {
      var query = new URLSearchParams({ theme: state.theme });
      data.knobs.forEach(function (knob) {
        query.set(knob.name, text(state.values[knob.name]));
      });
      return data.playground + "#!/" + data.id + "?" + query.toString();
    }
    function field(label, input) {
      var wrap = document.createElement("label");
      wrap.appendChild(document.createTextNode(label));
      wrap.appendChild(input);
      bar.appendChild(wrap);
    }
    function select(options, current, onChange) {
      var input = document.createElement("select");
      options.forEach(function (option) {
        var item = document.createElement("option");
        item.value = option;
        item.textContent = option === "" ? "(empty)" : option;
        item.selected = option === current;
        input.appendChild(item);
      });
      input.addEventListener("change", function () {
        onChange(input.value);
        update();
      });
      return input;
    }

    if (data.themes.length > 1) {
      field("theme", select(data.themes, state.theme, function (v) { state.theme = v; }));
    }
    data.knobs.forEach(function (knob) {
      var input;
      if (knob.type === "bool") {
        input = document.createElement("input");
        input.type = "checkbox";
        input.checked = knob.default;
        input.addEventListener("change", function () {
          state.values[knob.name] = input.checked;
          update();
        });
      } else if (knob.type === "select") {
        input = select(knob.options, knob.default, function (v) { state.values[knob.name] = v; });
      } else if (knob.type === "text" && knob.samples && knob.samples.length > 1) {
        input = select(knob.samples, knob.default, function (v) { state.values[knob.name] = v; });
      } else {
        input = document.createElement("span");
        input.className = "gsg-live";
        input.textContent = text(knob.default) + (data.playground ? " (edit live)" : "");
      }
      field(knob.name, input);
    });
    bar.appendChild(status);

    if (data.playground) {
      var open = document.createElement("button");
      open.type = "button";
      open.textContent = "Open live playground";
      open.addEventListener("click", function () {
        var frame = root.querySelector("iframe");
        if (frame) {
          frame.remove();
          open.textContent = "Open live playground";
          return;
        }
        frame = document.createElement("iframe");
        frame.title = data.title + " playground";
        frame.src = playgroundUrl();
        root.insertBefore(frame, bar.nextSibling);
        open.textContent = "Close live playground";
      });
      bar.appendChild(open);
    }

    if (bar.childNodes.length > 1 || data.playground) {
      root.insertBefore(bar, root.querySelector("details"));
      root.classList.add("gsg-enhanced");
    }
  }
  function run() {
    document.querySelectorAll(".gpui-styleguide").forEach(enhance);
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", run);
  } else {
    run();
  }
})();
