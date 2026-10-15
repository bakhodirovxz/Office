// DocsAPI.DocEditor shim (spike 4). Creates the editor iframe, posts the signed config to it,
// and relays only onDocumentReady / onDocumentStateChange / onError / onSaved to config.events.
(function () {
  var script = document.currentScript;
  var origin = new URL(script.src).origin;
  var RELAYED = ['onDocumentReady', 'onDocumentStateChange', 'onError', 'onSaved', 'onClosed', 'onAutosave', 'onRecovered'];

  function DocEditor(placeholderId, config) {
    var ph = document.getElementById(placeholderId);
    var iframe = document.createElement('iframe');
    iframe.src = origin + '/editor' + (config && config.adapter ? '?adapter=' + encodeURIComponent(config.adapter) : '');
    iframe.style.cssText = 'width:100%;height:100%;border:0';
    iframe.setAttribute('title', 'document editor');
    ph.innerHTML = '';
    ph.appendChild(iframe);
    var events = (config && config.events) || {};
    function onMessage(ev) {
      if (ev.origin !== origin || ev.source !== iframe.contentWindow || !ev.data) return;
      if (ev.data.type === 'docs:ready') {
        var plain = JSON.parse(JSON.stringify(config, function (k, v) { return typeof v === 'function' ? undefined : v; }));
        iframe.contentWindow.postMessage({ type: 'docs:config', config: plain }, origin);
      } else if (ev.data.type === 'docs:event' && RELAYED.indexOf(ev.data.event) >= 0) {
        var cb = events[ev.data.event];
        if (typeof cb === 'function') cb({ data: ev.data.data });
      }
    }
    window.addEventListener('message', onMessage);
    // Non-standard additions: ask the editor to save now (bridge adapter only), and close it
    // (the docserver then reports status 2, or 4 when nothing was saved).
    this.requestSave = function () { iframe.contentWindow.postMessage({ type: 'docs:requestSave' }, origin); };
    this.close = function () { iframe.contentWindow.postMessage({ type: 'docs:close' }, origin); };
    this.destroyEditor = function () {
      window.removeEventListener('message', onMessage);
      ph.innerHTML = '';
    };
  }
  window.DocsAPI = { DocEditor: DocEditor };
})();
