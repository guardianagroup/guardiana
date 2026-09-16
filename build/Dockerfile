# Reproducible build container (brief §10). Everything is pinned:
# - the base image by digest (rust 1.98-bookworm, amd64);
# - zig by version and SHA-256 (cross-links the Windows binary);
# - cargo-zigbuild by version.
# The source enters at /src; build/repro.sh runs cargo inside with --locked,
# SOURCE_DATE_EPOCH from the commit, and strips the binaries.
FROM rust@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922

ARG ZIG_VERSION=0.16.0
ARG ZIG_SHA256=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00
ARG CARGO_ZIGBUILD_VERSION=0.23.4

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends xz-utils ca-certificates curl; \
    rm -rf /var/lib/apt/lists/*; \
    curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" -o /tmp/zig.tar.xz; \
    echo "${ZIG_SHA256}  /tmp/zig.tar.xz" | sha256sum -c -; \
    mkdir -p /opt/zig; tar -xJf /tmp/zig.tar.xz -C /opt/zig --strip-components=1; rm /tmp/zig.tar.xz; \
    ln -s /opt/zig/zig /usr/local/bin/zig; \
    rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu; \
    cargo install --locked "cargo-zigbuild@${CARGO_ZIGBUILD_VERSION}"

ENV CARGO_HOME=/cargo-home \
    RUSTFLAGS="--remap-path-prefix=/src=guardiana --remap-path-prefix=/cargo-home=cargo" \
    CARGO_INCREMENTAL=0
WORKDIR /src
