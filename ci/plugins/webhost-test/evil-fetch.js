// Test-only (M11): a plugin Worker that tries the network itself. Served from /shell/plugins/ with the Worker CSP it must fail.
onmessage = async () => {
  const r = {};
  for (const [k, u] of [['same', '/shell-config.json'], ['other', 'https://example.org/']]) {
    try { r[k] = 'reached ' + (await fetch(u)).status; } catch (e) { r[k] = 'blocked ' + e.name; }
  }
  try { importScripts('/shell/plugins/host.js'); r.imp = 'imported'; } catch (e) { r.imp = 'blocked ' + e.name; }
  postMessage(r);
};
