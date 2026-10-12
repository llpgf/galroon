# CI uses ubuntu-24.04 too: keep the runtime glibc compatible with its binary.
# Build context contains prebuilt files from galroon-nas-x86_64, never Rust sources.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends 7zip ca-certificates \
    && test -x /usr/bin/7zz \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/galroon
COPY galroon-core ./galroon-core
COPY web/ ./web/
# Artifact downloads do not preserve Unix executable permissions.
RUN chmod 0755 /opt/galroon/galroon-core \
    && chmod -R a+rX /opt/galroon/web
ENV GALROON_7ZIP=/usr/bin/7zz
# Runtime uid/gid MUST be supplied by compose's user:, not baked into the image.
ENTRYPOINT ["/opt/galroon/galroon-core"]
CMD ["serve", "--data-dir", "/data"]
