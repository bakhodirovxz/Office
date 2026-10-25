/*
 * Embed script of the document server: DocsAPI.DocEditor(id, config), shaped like the public ONLYOFFICE Docs API
 * (editor config, events, methods) so a host page written for that API works unchanged.
 * Clean-room: designed from the public API documentation and from a host page's use of it only; no ONLYOFFICE code was read,
 * copied or vendored, and this file is written from scratch.
 *
 * Load it as a classic <script src> (never type=module and never fetch): it is served without CORS headers.
 * Everything after loading is postMessage between this page and one iframe of the document server, both ends checking the
 * other's origin and window (PLAN phase 5, S14). The `events` of the config never leave this page.
 */
(function () {
  'use strict';
  var script = document.currentScript;
  if (!script || !script.src) return;
  var dsOrigin = new URL(script.src).origin;
  // events the wrapper may relay; onSaved and onClosed are non-standard additions of ours
  var EVENTS = ['onAppReady', 'onDocumentReady', 'onDocumentStateChange', 'onError', 'onWarning', 'onRequestClose', 'onSaved', 'onClosed'];
  // methods of the ONLYOFFICE API we do not provide: calling one is an error, not a silent no-op
  var UNSUPPORTED = ['downloadAs', 'insertImage', 'refreshHistory', 'setHistoryData', 'requestClose', 'setActionLink', 'setFavorite',
    'setReferenceData', 'setRequestedDocument', 'setRequestedSpreadsheet', 'setRevisedFile', 'setSharingSettings', 'setUsers',
    'showMessage', 'denyEditingRights', 'startFilling'];

  function DocEditor(id, config) {
    var self = this;
    var ph = document.getElementById(id);
    config = config || {};
    if (!ph) { console.error('DocsAPI.DocEditor: no element with id "' + id + '"'); return; }
    var events = config.events || {};
    // the config goes to the server as the provider signed it: functions and the unsigned `events` stay here
    var plain = JSON.parse(JSON.stringify(config, function (k, v) { return typeof v === 'function' || (k === 'events' && this === config) ? undefined : v; }));
    var frame = document.createElement('iframe');
    frame.id = id;
    frame.name = 'frameEditor';
    frame.src = dsOrigin + '/ds/editor#o=' + encodeURIComponent(location.origin);
    frame.title = (config.document && config.document.title) || 'editor';
    frame.allow = 'clipboard-read; clipboard-write; fullscreen';
    frame.setAttribute('allowfullscreen', '');
    frame.style.cssText = 'border:0;display:block;width:' + dim(config.width) + ';height:' + dim(config.height);
    ph.parentNode.replaceChild(frame, ph);
    var gone = false;

    function post(msg) { if (frame.contentWindow) frame.contentWindow.postMessage(msg, dsOrigin); }
    function onMessage(ev) {
      if (ev.origin !== dsOrigin || ev.source !== frame.contentWindow || !ev.data || typeof ev.data.type !== 'string') return;
      if (ev.data.type === 'docs:ready') post({ type: 'docs:config', config: plain });
      else if (ev.data.type === 'docs:event' && EVENTS.indexOf(ev.data.event) >= 0) {
        var cb = events[ev.data.event];
        if (typeof cb === 'function') { try { cb.call(self, { target: self, data: ev.data.data }); } catch (e) { console.error(e); } }
      }
    }
    window.addEventListener('message', onMessage);

    // non-standard: ask the editor to save now (a status 6 callback), and close the session (status 2, or 4 when nothing was saved)
    this.requestSave = function () { post({ type: 'docs:requestSave' }); };
    this.close = function () { post({ type: 'docs:close' }); };
    this.destroyEditor = function () {
      if (gone) return;
      gone = true;
      // the server session ends first; the frame lives until it says so, but at most 3 s, and is hidden meanwhile
      post({ type: 'docs:close' });
      frame.style.display = 'none';
      function finish() {
        window.removeEventListener('message', onMessage);
        window.removeEventListener('message', waitClosed);
        clearTimeout(timer);
        if (frame.parentNode) frame.parentNode.replaceChild(ph, frame);
      }
      function waitClosed(ev) {
        if (ev.origin === dsOrigin && ev.source === frame.contentWindow && ev.data && ev.data.type === 'docs:event' && ev.data.event === 'onClosed') finish();
      }
      var timer = setTimeout(finish, 3000);
      window.addEventListener('message', waitClosed);
    };
    UNSUPPORTED.forEach(function (m) { self[m] = function () { throw new Error('DocsAPI.DocEditor.' + m + ' is not supported'); }; });
  }
  function dim(v) { return v == null ? '100%' : typeof v === 'number' ? v + 'px' : /^[0-9.]+(px|%|em|rem|vh|vw)?$/.test(v) ? (/[a-z%]$/.test(v) ? v : v + 'px') : '100%'; }
  window.DocsAPI = { DocEditor: DocEditor };
})();
