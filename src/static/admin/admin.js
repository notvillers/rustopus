/* Rustopus MCP admin dashboard.
 *
 * External file, not an inline <script>: the CSP in main.rs::security_headers()
 * is `script-src 'self'`, so an inline script would be blocked. `connect-src
 * 'self'` already permits the same-origin fetch() calls below.
 *
 * Authentication rides on the browser's own HTTP Basic credentials — the same
 * ones that fetched this page — so nothing here handles or stores a token. */

(function () {
    'use strict';

    var statusEl = document.getElementById('status');
    var bodyEl = document.getElementById('entries-body');
    var fillEl = document.getElementById('usage-fill');
    var usageTextEl = document.getElementById('usage-text');
    var diskFillEl = document.getElementById('disk-fill');
    var diskTextEl = document.getElementById('disk-text');
    var formEl = document.getElementById('add-form');
    var reloadEl = document.getElementById('reload');

    function setStatus(message, kind) {
        statusEl.textContent = message || '';
        statusEl.className = 'status' + (kind ? ' ' + kind : '');
    }

    function formatBytes(bytes) {
        var mb = (bytes || 0) / 1048576;
        if (mb >= 1024) { return (mb / 1024).toFixed(2) + ' GB'; }
        return mb.toFixed(1) + ' MB';
    }

    function formatDuration(ms) {
        if (!ms) { return '—'; }
        if (ms < 1000) { return ms + ' ms'; }
        return (ms / 1000).toFixed(1) + ' s';
    }

    function formatTime(iso) {
        if (!iso) { return 'never'; }
        var date = new Date(iso);
        if (isNaN(date.getTime())) { return iso; }
        var age = Math.round((Date.now() - date.getTime()) / 1000);
        if (age < 60) { return age + 's ago'; }
        if (age < 3600) { return Math.round(age / 60) + 'm ago'; }
        if (age < 86400) { return Math.round(age / 3600) + 'h ago'; }
        return Math.round(age / 86400) + 'd ago';
    }

    /* Build cells with textContent, never innerHTML: entry labels are operator
     * input and must not be able to inject markup into this page. */
    function cell(row, text, className) {
        var td = document.createElement('td');
        td.textContent = text;
        if (className) { td.className = className; }
        row.appendChild(td);
        return td;
    }

    function statusOf(entry) {
        if (entry.running) { return { text: 'refreshing…', className: 'state-warn' }; }
        if (!entry.enabled) { return { text: 'disabled', className: 'state-idle' }; }
        if (!entry.last_outcome) { return { text: 'not yet run', className: 'state-idle' }; }
        if (entry.last_outcome === 'ok') { return { text: 'ok', className: 'state-ok' }; }
        return { text: entry.last_outcome, className: 'state-bad' };
    }

    function actionButton(label, className, handler) {
        var button = document.createElement('button');
        button.type = 'button';
        button.textContent = label;
        if (className) { button.className = className; }
        button.addEventListener('click', handler);
        return button;
    }

    function request(method, url, body) {
        var options = { method: method, headers: {} };
        if (body !== undefined) {
            options.headers['Content-Type'] = 'application/json';
            options.body = JSON.stringify(body);
        }
        return fetch(url, options).then(function (response) {
            if (response.status === 401) {
                throw new Error('Not authorised — reload the page and re-enter the admin token.');
            }
            return response.json().catch(function () { return {}; }).then(function (payload) {
                if (!response.ok) {
                    throw new Error(payload.error || ('request failed with ' + response.status));
                }
                return payload;
            });
        });
    }

    function renderBar(fill, ratio) {
        var percent = Math.min((ratio || 0) * 100, 100);
        fill.style.width = percent.toFixed(1) + '%';
        fill.className = 'fill' + (ratio >= 0.9 ? ' bad' : ratio >= 0.75 ? ' warn' : '');
        return percent;
    }

    function renderUsage(cache, disk) {
        var percent = renderBar(fillEl, cache.usage_ratio);
        usageTextEl.textContent =
            formatBytes(cache.used_bytes) + ' of ' + formatBytes(cache.budget_bytes) +
            ' (' + percent.toFixed(1) + '%), ' + cache.entries_held + ' snapshot(s) held';

        if (!disk) { return; }
        var diskPercent = renderBar(diskFillEl, disk.usage_ratio);
        diskTextEl.textContent =
            formatBytes(disk.used_bytes) + ' of ' + formatBytes(disk.budget_bytes) +
            ' (' + diskPercent.toFixed(1) + '%), ' + disk.snapshots_stored + ' snapshot(s) in ' + disk.path;
    }

    function renderEntries(entries) {
        bodyEl.textContent = '';

        if (!entries.length) {
            var empty = document.createElement('tr');
            var td = document.createElement('td');
            td.colSpan = 11;
            td.className = 'empty';
            td.textContent = 'No entries configured. Add one below to keep a catalog warm.';
            empty.appendChild(td);
            bodyEl.appendChild(empty);
            return;
        }

        entries.forEach(function (entry) {
            var row = document.createElement('tr');
            cell(row, entry.label);

            /* Already masked server-side (FFD3…0E37); the full code never
             * reaches this page. */
            var codeCell = document.createElement('td');
            var code = document.createElement('code');
            code.textContent = entry.authcode;
            codeCell.appendChild(code);
            row.appendChild(codeCell);

            cell(row, String(entry.pid));
            cell(row, formatTime(entry.last_run));
            cell(row, formatDuration(entry.last_duration_ms));
            cell(row, entry.bytes ? formatBytes(entry.bytes) : '—');
            cell(row, entry.products ? String(entry.products) : '—');
            cell(row, entry.hits + entry.misses ? (entry.hit_rate * 100).toFixed(0) + '%' : '—');
            cell(row, entry.on_disk ? 'yes' : 'no', entry.on_disk ? 'state-ok' : 'state-idle');

            var state = statusOf(entry);
            cell(row, state.text, state.className);

            var actions = document.createElement('td');
            var wrapper = document.createElement('div');
            wrapper.className = 'actions';

            wrapper.appendChild(actionButton('Refresh', null, function (event) {
                event.target.disabled = true;
                setStatus('Refreshing "' + entry.label + '" — a full catalog pull takes about half a minute…');
                request('POST', '/admin/api/entries/' + encodeURIComponent(entry.id) + '/refresh')
                    .then(function () { setStatus('Refreshed "' + entry.label + '".', 'success'); load(); })
                    .catch(function (error) { setStatus(error.message, 'error'); event.target.disabled = false; });
            }));

            wrapper.appendChild(actionButton(entry.enabled ? 'Disable' : 'Enable', null, function () {
                request('PATCH', '/admin/api/entries/' + encodeURIComponent(entry.id), { enabled: !entry.enabled })
                    .then(function () { load(); })
                    .catch(function (error) { setStatus(error.message, 'error'); });
            }));

            wrapper.appendChild(actionButton('Evict', null, function () {
                request('POST', '/admin/api/entries/' + encodeURIComponent(entry.id) + '/evict')
                    .then(function () { setStatus('Dropped the cached snapshot for "' + entry.label + '".', 'success'); load(); })
                    .catch(function (error) { setStatus(error.message, 'error'); });
            }));

            wrapper.appendChild(actionButton('Remove', 'danger', function () {
                if (!window.confirm('Remove "' + entry.label + '"? Its stored authcode is deleted too.')) { return; }
                request('DELETE', '/admin/api/entries/' + encodeURIComponent(entry.id))
                    .then(function () { setStatus('Removed "' + entry.label + '".', 'success'); load(); })
                    .catch(function (error) { setStatus(error.message, 'error'); });
            }));

            actions.appendChild(wrapper);
            row.appendChild(actions);
            bodyEl.appendChild(row);
        });
    }

    function load() {
        return request('GET', '/admin/api/state')
            .then(function (payload) {
                renderUsage(payload.cache, payload.disk);
                renderEntries(payload.entries);
            })
            .catch(function (error) {
                setStatus(error.message, 'error');
            });
    }

    formEl.addEventListener('submit', function (event) {
        event.preventDefault();
        var data = new FormData(formEl);
        var body = {
            label: (data.get('label') || '').trim(),
            authcode: (data.get('authcode') || '').trim(),
            pid: parseInt(data.get('pid'), 10),
            url: (data.get('url') || '').trim() || null
        };
        if (isNaN(body.pid)) {
            setStatus('Partner ID must be a number.', 'error');
            return;
        }

        request('POST', '/admin/api/entries', body)
            .then(function () {
                formEl.reset();
                setStatus('Entry added. It will be warmed on the next sweep, or use Refresh to do it now.', 'success');
                load();
            })
            .catch(function (error) { setStatus(error.message, 'error'); });
    });

    reloadEl.addEventListener('click', function () {
        setStatus('');
        load();
    });

    load();
    /* A refresh in progress finishes without the page being touched; poll so its
     * outcome and the new cache usage appear on their own. */
    window.setInterval(load, 15000);
})();
