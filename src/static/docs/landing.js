// Fills the "CALL VIA TERMINAL" example on the landing page with the
// deployment's API base URL. The value comes from landing-config.js
// (RUSTOPUS_API_BASE); when unset or empty, the page's own origin is used.
// External file (not inline) because the server's CSP is script-src 'self'.
(function () {
    var base = (window.RUSTOPUS_API_BASE || "").replace(/\/+$/, "")
        || window.location.origin;

    var cmd = document.getElementById("terminal-cmd");
    if (cmd && base) {
        cmd.textContent = 'curl "' + base + '/get-product?url=<octopus-wsdl>&authcode=<code>"';
    }
})();