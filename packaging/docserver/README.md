# Document server (ONLYOFFICE-API compatible)

A self-hosted document server for your own application. Your app embeds it exactly as it would embed ONLYOFFICE Document Server:
load `api.js`, hand `DocsAPI.DocEditor` a config signed with a shared JWT secret, and receive save callbacks. It opens, edits and
saves **docx, xlsx, pptx and pdf** (and opens them view-only). It runs as one container; accounts are not part of it (your app
decides who may open what and signs the config).

This page is for a **new install**. Replacing an ONLYOFFICE container that already runs is the installer path
(`install.sh`, [docs/phase5/dropin.md](../../docs/phase5/dropin.md)).

## Quick start

The server speaks TLS (your editors are embedded in an https or localhost page; browsers refuse the rest): either give it a
certificate and key as files, as below, or put a TLS proxy in front (see `DOCSERVER_TRUSTED_PROXIES`).

```sh
docker run -d --name docserver --network appnet -p 8443:8787 \
  -v docserver-data:/data -v ./certs:/certs:ro \
  -e JWT_ENABLED=true \
  -e JWT_SECRET="$(openssl rand -hex 24)" \
  -e DOCSERVER_PUBLIC_URL=https://docs.example.com:8443 \
  -e DOCSERVER_TLS_CERT=/certs/fullchain.pem -e DOCSERVER_TLS_KEY=/certs/privkey.pem \
  -e DOCSERVER_FETCH_ALLOW=http://app:3000 \
  -e DOCSERVER_FRAME_ORIGINS=https://app.example.com \
  -e DOCSERVER_INTERNAL_PORT=8780 -e DOCSERVER_INTERNAL_URL=http://docserver:8780 -e DOCSERVER_INTERNAL_ALLOW=app \
  <image>
```

Use the same `JWT_SECRET` in your app (keep it somewhere better than a shell line; `JWT_SECRET_FILE=/run/secrets/ds` works too).
At least 16 bytes are required, 32 or more recommended. Without `JWT_SECRET` or `JWT_SECRET_FILE` the container refuses to start;
there is no built-in secret.

The same as compose:

```yaml
services:
  docserver:
    image: <image>
    read_only: true
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    tmpfs: ["/tmp:size=256m,uid=10002"]
    ports: ["8443:8787"]
    volumes: ["docserver-data:/data", "./certs:/certs:ro"]   # key and certificate readable by uid 10002
    environment:
      JWT_ENABLED: "true"
      JWT_SECRET_FILE: /run/secrets/ds_jwt
      DOCSERVER_PUBLIC_URL: https://docs.example.com:8443
      DOCSERVER_TLS_CERT: /certs/fullchain.pem
      DOCSERVER_TLS_KEY: /certs/privkey.pem
      DOCSERVER_FETCH_ALLOW: http://app:3000
      DOCSERVER_FRAME_ORIGINS: https://app.example.com
      DOCSERVER_INTERNAL_PORT: "8780"
      DOCSERVER_INTERNAL_URL: http://docserver:8780
      DOCSERVER_INTERNAL_ALLOW: app
    secrets: [ds_jwt]
  app:                         # your application, on the same compose network
    ...
volumes: { docserver-data: {} }
secrets: { ds_jwt: { file: ./ds_jwt.txt } }
```

A missing required setting stops the container with a one-line message naming it (`docker logs docserver`).

## Settings

ONLYOFFICE's names first (same meaning), then ours (`DOCSERVER_*`). The container sets `JWT_*` from the environment only when
`JWT_SECRET` or `JWT_SECRET_FILE` is present.

| Setting | Default | Meaning |
|---|---|---|
| `JWT_ENABLED` | `true` | `false` is refused: the server never runs without a token. |
| `JWT_SECRET` / `JWT_SECRET_FILE` | none, **required** | The HS256 secret your app signs the editor config with and the server signs callbacks with. |
| `JWT_HEADER`, `JWT_IN_BODY` | `Authorization`, `true` | Accepted for compatibility, ignored (a warning at start): the callback token is always in the body and in `Authorization`. |
| `DOCSERVER_PUBLIC_URL` | **required** | `https://host[:port]` your users' browsers use to reach this server (no path). |
| `DOCSERVER_TLS_CERT`, `DOCSERVER_TLS_KEY` | none | Certificate chain and key (PEM files) for the server's own TLS. One of these two or `DOCSERVER_TRUSTED_PROXIES` is required. |
| `DOCSERVER_TRUSTED_PROXIES` | none | Comma list of the proxy addresses, CIDR ranges or container names that terminate TLS in front of the server (then no certificate is given). |
| `DOCSERVER_FETCH_ALLOW` | **required** | Comma list of `scheme://host[:port]` origins the server may download documents from (your `document.url` hosts). Private addresses are allowed for these entries only. |
| `DOCSERVER_CALLBACK_ALLOW` | = `DOCSERVER_FETCH_ALLOW` | Origins it may POST save callbacks to (your `callbackUrl` hosts). |
| `DOCSERVER_FRAME_ORIGINS` | the public URL only | Comma list of origins of **your pages** that embed the editor. A page not listed gets "the session was refused". |
| `DOCSERVER_INTERNAL_URL` | **required** | `http://host:port` your app uses to download the saved file named in the callback's `url` (the address of this container on the network you share with the app). |
| `DOCSERVER_INTERNAL_PORT` | `80` | The port of that plain-http listener (never publish it with `-p`; a port below 1024 needs `--sysctl net.ipv4.ip_unprivileged_port_start=80`). |
| `DOCSERVER_INTERNAL_ALLOW` | **required** | Who may call it: your app's container names, IP addresses or CIDR ranges. Nobody else is answered. |
| `DOCSERVER_PUBLIC_PORT` | `8787` | The port the public side listens on inside the container. |
| `DOCSERVER_UI_LANG` | the config's `lang` | Force the editors' UI language (for example `ru`, `uz-Latn`). |
| `DOCSERVER_EDIT_TYPES` | `docx,xlsx,pptx,pdf` | File types that may edit; the others open view-only. |
| `DOCSERVER_PROVIDER` | `docsapi` | The name in the audit log. |
| `DOCSERVER_MAX_DOCUMENT_MB` | `100` | Largest document it downloads. |
| `DOCSERVER_KEEP_CLOSED_HOURS`, `DOCSERVER_KEEP_ABANDONED_DAYS` | `168`, `7` | Retention of closed and abandoned sessions in `/data`. |
| `DOCSERVER_CLOSE_IDLE_MINUTES` | `5` | A page that went away is closed (and saved) after this many idle minutes. |
| `DOCSERVER_ALLOW_OPAQUE_ORIGIN` | `0` | Only for a mobile WebView with an opaque (`null`) page origin; see dropin.md D3b. |
| `DOCSERVER_SECRETS_DIR` | `/run/secrets` | Mount `server-secret`, `converter-secret` and `audit-key` here for stable internal keys; absent ones are generated once into `/data/secrets` (fine for a trial; the audit key belongs outside the data volume). |

The container is built to run with a read-only root: the `/data` volume (sessions, version history, audit log) and a `/tmp`
tmpfs are the only writable places. Back up `/data`.

## Connecting your application

An existing ONLYOFFICE connector needs three changes: the server URL, the secret, and the network rules below.

1. **api.js.** Load it from `https://<public url>/web-apps/apps/api/documents/api.js`, then `new DocsAPI.DocEditor(id, config)` as you do now.
2. **JWT.** Sign the whole config (everything but `events`) with HS256 and the shared secret, put the result in `config.token`.
   A token without `exp` is accepted (as ONLYOFFICE does); one session lasts at most 12 h. The save callback body carries `token`
   (a JWT of the body) and an `Authorization: Bearer` header (a JWT of `{"payload": body}`); verify either with the same secret.
3. **Document URL.** `document.url` must be on an origin in `DOCSERVER_FETCH_ALLOW` and reachable from the container; it must
   answer the document bytes (a short-lived token in the URL is the usual protection), without redirects.
   `document.fileType` must match the file (docx, xlsx, pptx, pdf), `document.key` is yours (new per open is fine; history is
   kept per session) and `documentType` is `word`, `cell`, `slide` or `pdf`.
4. **Callback URL.** `editorConfig.callbackUrl` must be on an origin in `DOCSERVER_CALLBACK_ALLOW`. On status 2 (document closed
   with changes) and 6 (force save) the body has a `url`: download it from your app, from a network the server lists in
   `DOCSERVER_INTERNAL_ALLOW`. Answer `{"error":0}`. Status 4 means closed without changes; a view-only session sends nothing.
5. **Your page** must be in `DOCSERVER_FRAME_ORIGINS`.
6. **View-only:** `document.permissions.edit: false` and `editorConfig.mode: "view"` (leave out `callbackUrl`). The server then
   refuses saves for that session, whatever the page does.

A working host of about 100 lines, with no dependencies, is `ci/web-smoke/generic-host/host.mjs`.

## Differences from ONLYOFFICE that matter

Full tables: [docs/phase5/onlyoffice-compat.md](../../docs/phase5/onlyoffice-compat.md). In short:

- Allow lists: ONLYOFFICE downloads from any address you hand it; this server downloads from `DOCSERVER_FETCH_ALLOW` only, and your
  app downloads saved files only through the internal listener, from `DOCSERVER_INTERNAL_ALLOW`.
- Callback status 1 (a user connected) is sent once on the first change, not on every connection; status 2 is sent when the
  document is closed (or the idle close), status 6 on `requestSave` or forcesave.
- Only HS256 tokens, only the four file types, no `doc`, `odt`, `rtf` or `txt`; no comment-only or review-only mode (opened as view);
  co-editing is not offered. Fields not listed in the compat tables are ignored.
- The editors are ours (not ONLYOFFICE's), so layout, fonts and the toolbar differ.

## Upgrading

Pull or load the new image, remove the container and run it again with the same settings and the same `/data` volume.
Sessions open at that moment end with the old container (not tested here: close them first). Keep the same
`DOCSERVER_SECRETS_DIR` (or volume) so that the internal keys stay stable.

## Logs and health

`docker logs docserver`: one line per start, refusal and audit event; tokens, URLs with tokens and document contents are never
logged. The image's `HEALTHCHECK` asks `/healthz` on the public port. The audit log is in `/data/docserver`.

## Proof

`rehearse-generic.sh` starts this image fresh with the settings above only, drives the generic host through open, edit, save,
reopen and download for docx, xlsx, pptx and pdf and a view-only open for each, in Chromium. Results: `docs/STATUS.md`
(Phase 5, quick start) and screenshots in `docs/phase5/evidence/generic/`.
