# CI uses ubuntu-24.04 too: keep the runtime glibc compatible with its binary.
# Build context contains prebuilt files from galroon-nas-x86_64, never Rust sources.
FROM ubuntu:24.04
RUN apt-get update \
    && apt-get install -y --no-install-recommends 7zip ca-certificates \
    && test -x /usr/bin/7z \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/galroon
COPY galroon-core ./galroon-core
COPY web/ ./web/
# Artifact downloads do not preserve Unix executable permissions.
RUN chmod 0755 /opt/galroon/galroon-core \
    && chmod -R a+rX /opt/galroon/web
# Ubuntu 24.04's 7zip package (7-Zip 23.01) installs the full 7-Zip as /usr/bin/7z, not 7zz.
ENV GALROON_7ZIP=/usr/bin/7z
# Runtime uid/gid MUST be supplied by compose's user:, not baked into the image.
ENTRYPOINT ["/opt/galroon/galroon-core"]
# The control socket lives under the tmpfs compose mounts at /run/galroon: Synology ACL
# shares cannot hold the 0700 directory the socket requires. Core creates the control/
# subdirectory itself, because Docker 24 does not reapply the tmpfs mode=0700 on restart.
CMD ["serve", "--data-dir", "/data", "--control-dir", "/run/galroon/control"]
