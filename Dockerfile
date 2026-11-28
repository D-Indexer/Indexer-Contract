# syntax=docker/dockerfile:1
# Reproducible build and dev environment (#239).
#
#   make docker-build   builds the optimized wasm in the `build` stage and
#                       writes it to target/docker/, then prints its sha256.
#   .devcontainer/      opens the `dev` stage (toolchain only).
#
# Always linux/amd64 (Apple Silicon runs it under emulation), so every host
# uses the same Rust and Stellar CLI binaries and produces the same wasm.
# Bump each pin as one named change: digest, rustup version + sha256, Stellar
# CLI version + sha256 (keep the CLI pin in sync with contract-ci.yml).
FROM --platform=linux/amd64 debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc AS dev

ARG RUSTUP_VERSION=1.29.3
ARG RUSTUP_SHA256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55
ARG STELLAR_CLI_VERSION=28.1.0
ARG STELLAR_CLI_SHA256=c1680deee94301d33ada7a17f98411e642a4248c727afbd2e43050d345746462

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev make jq python3 git libdbus-1-3 libudev1 \
 && rm -rf /var/lib/apt/lists/*

COPY scripts/install-stellar-cli.sh /tmp/install-stellar-cli.sh
RUN STELLAR_CLI_VERSION="$STELLAR_CLI_VERSION" STELLAR_CLI_SHA256="$STELLAR_CLI_SHA256" \
    /tmp/install-stellar-cli.sh /usr/local/bin && rm /tmp/install-stellar-cli.sh

RUN useradd --create-home --uid 1000 builder
USER builder
ENV CARGO_HOME=/home/builder/.cargo \
    RUSTUP_HOME=/home/builder/.rustup \
    PATH=/home/builder/.cargo/bin:$PATH
RUN curl -sSfL -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
 && echo "${RUSTUP_SHA256}  /tmp/rustup-init" | sha256sum --check --strict \
 && chmod +x /tmp/rustup-init \
 && /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none \
 && rm /tmp/rustup-init

# The toolchain, components and wasm target come from rust-toolchain.toml.
WORKDIR /workspace
COPY --chown=builder rust-toolchain.toml ./
RUN rustup toolchain install && rustc --version

FROM dev AS build
COPY --chown=builder . .
# Fixed source paths so any path that reaches the wasm is host-independent.
ENV RUSTFLAGS="--remap-path-prefix=/home/builder/.cargo=/cargo --remap-path-prefix=/workspace=/build"
RUN stellar contract build --locked --optimize --out-dir target/stellar \
 && sha256sum target/stellar/indexer_contract.wasm

FROM scratch AS artifact
COPY --from=build /workspace/target/stellar/indexer_contract.wasm /
