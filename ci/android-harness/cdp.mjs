// Minimal CDP client for the WebView's devtools socket (Playwright's connectOverCDP does not work on a WebView: no browser contexts).
// `adb forward tcp:<port> localabstract:webview_devtools_remote_<pid>` first. Node's global WebSocket; our code, MIT OR Apache-2.0.
export async function connect(port) {
  const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const ws = new WebSocket(list.find((t) => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
  let id = 0; const pend = new Map(); const ctx = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pend.has(d.id)) { const [ok, bad] = pend.get(d.id); pend.delete(d.id); d.error ? bad(new Error(d.error.message)) : ok(d.result); }
    else if (d.method === 'Runtime.executionContextCreated') ctx.set(d.params.context.id, d.params.context);
    else if (d.method === 'Runtime.executionContextDestroyed') ctx.delete(d.params.executionContextId);
    else if (d.method === 'Runtime.executionContextsCleared') ctx.clear();
  };
  const send = (method, params = {}) => new Promise((ok, bad) => { const i = ++id; pend.set(i, [ok, bad]); ws.send(JSON.stringify({ id: i, method, params })); });
  await send('Runtime.enable'); await send('Page.enable');
  return {
    send, close: () => ws.close(),
    /** Evaluate `expr` in every default execution context (one per same-process frame); returns [{url, value|error}]. */
    async evalAll(expr) {
      const out = [];
      for (const c of [...ctx.values()].filter((c) => c.auxData?.isDefault)) {
        const r = await send('Runtime.evaluate', { expression: expr, contextId: c.id, returnByValue: true, awaitPromise: true }).catch((e) => ({ error: String(e) }));
        out.push({ origin: c.origin, frame: c.auxData?.frameId, value: r.result?.value, error: r.exceptionDetails?.text ?? r.error });
      }
      return out;
    },
  };
}
