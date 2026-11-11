// node cdp-eval.mjs '<js expression>'   evaluates in the emulator Chrome tab at http://localhost:* (adb forward tcp:9222 set by rig `forward`)
const t = (await (await fetch('http://127.0.0.1:9222/json/list')).json()).find((x) => x.type === 'page' && x.url.startsWith('http://localhost'));
const ws = new WebSocket(t.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) { console.log(JSON.stringify(d.result?.result?.value ?? d.error ?? d.result)); ws.close(); } };
ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: process.argv[2], returnByValue: true, awaitPromise: true } }));
