# LibreOffice reference renderer (TEST ONLY: WP9 fidelity harness). Nothing in the product calls it; no .ppt (owner O2).
# Debian packages come over plain http from deb.debian.org; apt verifies the GPG signatures, so the TLS-intercepting
# proxy on the dev PC does not matter. The versions are pinned; if the mirror drops one (security updates replace
# it), rebuild against snapshot.debian.org (see SOURCES.md).
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8

ENV DEBIAN_FRONTEND=noninteractive PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 HOME=/tmp TMPDIR=/tmp LANG=C.UTF-8
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        libreoffice-writer=4:7.4.7-1+deb12u14 libreoffice-calc=4:7.4.7-1+deb12u14 \
        libreoffice-impress=4:7.4.7-1+deb12u14 \
        fonts-crosextra-carlito=20220224-1 fonts-crosextra-caladea=20200211-1 \
        fonts-liberation2=2.1.5-1 fonts-noto-core=20201225-1 python3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin app

WORKDIR /app
COPY server.py worker.py relay.py /app/
COPY tests /app/tests
USER 10001
EXPOSE 8080
CMD ["python3", "-I", "/app/server.py"]
