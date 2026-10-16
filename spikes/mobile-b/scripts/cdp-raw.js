// raw CDP evaluate (works with Android WebView, which Playwright cannot attach to). usage: node cdp-raw.js PORT "<expr>"
(async () => {
  const list = await (await fetch(`http://localhost:${process.argv[2]}/json`)).json();
  const t = list.find((x) => x.type === 'page') || list[0];
  const ws = new WebSocket(t.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) { console.log(d.result.result.value ?? JSON.stringify(d.result)); ws.close(); } };
  ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: process.argv[3], returnByValue: true } }));
})().catch((e) => { console.error('ERR', e.message); process.exit(1); });
