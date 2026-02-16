// Miniquad JS plugin for loading progress overlay
// Rust calls these to update the HTML loading bar (avoids GL buffer flashing)

"use strict";

miniquad_add_plugin({
    name: "loading",
    version: "0.1.0",
    register_plugin: function (importObject) {
        importObject.env.loading_set_progress = function (pct_times_100) {
            // pct_times_100 is 0..10000 (percent * 100 for precision)
            var pct = pct_times_100 / 100.0;
            var fill = document.getElementById("loading-bar-fill");
            var pctEl = document.getElementById("loading-pct");
            if (fill) fill.style.width = pct + "%";
            if (pctEl) pctEl.textContent = Math.floor(pct) + "%";
        };

        importObject.env.loading_set_label = function (label_js) {
            var label = consume_js_object(label_js);
            var el = document.getElementById("loading-label");
            if (el) el.textContent = label;
        };

        importObject.env.console_log = function (msg_js) {
            var msg = consume_js_object(msg_js);
            console.log(msg);
        };

        importObject.env.loading_hide = function () {
            var overlay = document.getElementById("loading-overlay");
            if (!overlay) return;

            // Replace loading content with a "Click to Play" prompt.
            // Browsers require a real user gesture before delivering keyboard
            // events to a canvas, so we need the user to click once.
            var title = document.getElementById("loading-title");
            var barBg = document.getElementById("loading-bar-bg");
            var pct = document.getElementById("loading-pct");
            var label = document.getElementById("loading-label");
            if (title) title.textContent = "Click to Play";
            if (barBg) barBg.style.display = "none";
            if (pct) pct.style.display = "none";
            if (label) label.style.display = "none";
            overlay.style.cursor = "pointer";

            function startGame() {
                overlay.removeEventListener("click", startGame);
                overlay.removeEventListener("touchstart", startGame);
                document.removeEventListener("keydown", startGame);
                overlay.classList.add("hidden");
                setTimeout(function () {
                    overlay.style.display = "none";
                }, 300);
                var canvas = document.getElementById("glcanvas");
                if (canvas) {
                    canvas.setAttribute("tabindex", "1");
                    canvas.focus();
                }
            }

            overlay.addEventListener("click", startGame);
            overlay.addEventListener("touchstart", startGame);
            // Also allow any keypress to dismiss (for users who instinctively type)
            document.addEventListener("keydown", startGame);
        };
    },
});
