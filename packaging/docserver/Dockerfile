# The document server image (PLAN phase 5, WP10): hoshiya-serve (Linux build, see build.sh) + the editor dists + the shell.
# The entrypoint builds the config from ONLYOFFICE-style env when JWT_SECRET(_FILE) is set (docs/phase5/dropin.md D6).
# Nothing is fetched here: the binary and the dists are built on the host and copied in (the dev PC's proxy breaks TLS in containers).
# Run it with a read-only root: only /data (sessions, audit log) and /tmp (conversion scratch) are written.
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
RUN useradd --system --uid 10002 --no-create-home --shell /usr/sbin/nologin hoshiya \
 && mkdir /data /app && chown hoshiya /data
COPY --chmod=0755 hoshiya-serve /app/hoshiya-serve
COPY --chmod=0755 entrypoint.sh /app/entrypoint.sh
COPY --chmod=0755 diff /app/diff
COPY dist /app/dist
COPY brand.toml /app/brand/brand.toml
USER 10002
WORKDIR /app
# the converter routes need the domain CLIs, which this image does not carry (the document server does not use them)
ENV CONVERT_DOCS_CLI=unused CONVERT_SHEETS_CLI=unused CONVERT_SLIDES_CLI=unused CONVERT_PDF_CLI=unused
EXPOSE 8787
# bash is in the base image, curl is not: GET /healthz over the container's own loopback (the one request tls = "upstream" answers
# without the proxy)
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
  CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8787 && printf 'GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3 && head -n1 <&3 | grep -q ' 200 '"]
ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["--config", "/etc/hoshiya/api.toml"]
