// Fills the "CALL VIA TERMINAL" example on the landing page with the
// deployment's API base URL. The value comes from landing-config.js
// (RUSTOPUS_API_BASE); when unset or empty, the page's own origin is used.
// External file (not inline) because the server's CSP is script-src 'self'.
(function () {
    var base = (window.RUSTOPUS_API_BASE || "").replace(/\/+$/, "")
        || window.location.origin;

    var cmd = document.getElementById("terminal-cmd");
    if (cmd && base) {
        cmd.textContent = 'curl "' + base + '/get-bulk?url=<wsdl>&authcode=<code>&pid=<partner_id>"';
    }

    var heroXml = document.getElementById("hero-xml-sample");
    if (heroXml) {
        fetch("/docs/example.xml")
            .then(function (res) { return res.text(); })
            .then(function (text) { heroXml.textContent = text.trim(); })
            .catch(function () {});
    }
})();