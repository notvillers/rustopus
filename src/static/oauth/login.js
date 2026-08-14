/* Sign-in page behaviour.
 *
 * External file, not an inline <script>: the CSP these handlers set is
 * `script-src 'self'`, exactly like the admin dashboard's.
 *
 * It does one thing. The form's submit takes a round trip to Octopus to prove
 * the authcode, which is fast but not instant, and a second click would submit
 * the same one-time request id twice — so the button is disabled and says what
 * is happening. Nothing here validates the credentials: that is the server's
 * business, and a client-side check would only teach a guesser which half of
 * their guess was wrong. */

(function () {
    'use strict';

    var form = document.querySelector('form');
    var button = document.getElementById('submit');
    if (!form || !button) { return; }

    form.addEventListener('submit', function () {
        button.disabled = true;
        button.textContent = 'Checking with Octopus…';
    });
})();
