# PDF -> DOCX sidecar. The image as a whole is AGPL-3.0 (PyMuPDF); see LICENSES.md and SOURCES.md.
# Wheels come from ./wheelhouse (fetch-wheels.sh checks every hash), so the build needs no index access.
FROM python:3.12-slim-bookworm@sha256:5d9e6adc9abd558bc0ce05f9ab16cef71328916fc9728f6b81e337af86a737e2

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 TMPDIR=/tmp
COPY requirements.lock /build/requirements.lock
COPY wheelhouse /build/wheelhouse
RUN pip install --no-cache-dir --no-index --find-links /build/wheelhouse --no-deps --require-hashes \
        -r /build/requirements.lock \
 && rm -rf /build

WORKDIR /app
COPY merge_lines.py server.py worker.py relay.py /app/
COPY tests /app/tests
# merge_lines.py is the owner's code and must stay byte-identical.
RUN echo "9563dd3ae3ff705d571bb49772e43c5f4feb2515b0f178bda62b7ceac666e8a1  /app/merge_lines.py" | sha256sum -c - \
 && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin app
USER 10001
EXPOSE 8080
CMD ["python", "-I", "/app/server.py"]
