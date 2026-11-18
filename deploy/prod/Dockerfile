# syntax=docker/dockerfile:1
#
# Imagem de produção — servidor GodMesh (multi-stage).
#
# Bases resolvidas em 18/11/2026 (Docker Hub API, ver
# docs/09-equipe/evidencias/2026-11-17-container-prod.md):
#   rust:1.100-slim-bookworm  -> 1.100.0, publicado em 2026-11-12
#     índice multi-arch: sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
#     linux/arm64:       sha256:a05e7b80377ed6099c177cad2b9f536e182b9574490c7b4c07acaa29a44fda57
#   debian:trixie-slim (debian 13, stable atual) -> publicado em 2026-11-05
#     índice multi-arch: sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
#     linux/arm64:       sha256:24a01011d10eba3a023bc8585b9650b8b29021240d4217c05b905e09ef4a5232
#
# Estado em 18/11/2026: o binário `godmesh-server` ainda NÃO existe no workspace
# (crates/godmesh-server não foi criado). O build falha de propósito em
# `cargo build -p godmesh-server` com "package ID specification ... did not match
# any packages". Nada aqui é simulado para esconder isso.

ARG RUST_IMAGE=rust:1.100-slim-bookworm
ARG RUNTIME_IMAGE=debian:trixie-slim

# ---------- builder ----------
FROM ${RUST_IMAGE} AS builder

WORKDIR /src

ENV CARGO_TERM_COLOR=never \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_RETRY=3

# Manifestos primeiro: mantém a resolução de dependências em camada própria.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release --locked --bin godmesh-server --package godmesh-server

# ---------- runtime ----------
FROM ${RUNTIME_IMAGE} AS runtime

# ca-certificates: TLS de saída (push/webhooks, se o servidor usar).
# Sem compilador, sem toolchain Rust, sem cache de build nesta camada.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

RUN groupadd --system --gid 10001 godmesh \
    && useradd --system --uid 10001 --gid godmesh \
        --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin godmesh

COPY --from=builder /src/target/release/godmesh-server /usr/local/bin/godmesh-server

# Migrações versionadas na imagem, aplicadas por etapa controlada
# (nunca por docker-entrypoint-initdb.d). Somente leitura para o usuário final.
COPY --chown=root:root migrations /migrations
RUN chmod -R a-w /migrations

ENV GODMESH_ENV=production \
    GODMESH_MIGRATIONS_DIR=/migrations \
    RUST_BACKTRACE=0

USER 10001:10001

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/godmesh-server"]