# syntax=docker/dockerfile:1
#
# Imagem de produção — servidor GodMesh (multi-stage).
#
# Bases confirmadas e revalidadas em 2026-11-18T16:47Z pela Docker Hub API
# (`https://hub.docker.com/v2/repositories/library/rust/tags/1.100-slim-bookworm`
# e `.../library/debian/tags/trixie-slim`). As tags estáveis "latest" continuam
# sendo estas; nenhuma versão mais nova foi publicada desde o build anterior, e
# os digests abaixo são idênticos aos registrados em 17/11/2026 (sem repontamento):
#   rust:1.100-slim-bookworm  -> Rust 1.100.0, publicado em 2026-11-12T22:46:18Z
#     índice multi-arch: sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
#     linux/arm64:       sha256:a05e7b80377ed6099c177cad2b9f536e182b9574490c7b4c07acaa29a44fda57
#     linux/amd64:       sha256:476f6598b8d14a1f1bc7e58efc67af206ac8a559c37a3401ce512db2582d0ff7
#   debian:trixie-slim (Debian 13, stable) -> publicado em 2026-11-05T08:28:05Z
#     índice multi-arch: sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
#     linux/arm64:       sha256:24a01011d10eba3a023bc8585b9650b8b29021240d4217c05b905e09ef4a5232
#     linux/amd64:       sha256:7378260c0a098d27ac206225db5e7d92b8c7c84ea7afccc86eeb513756bbd392
#
# Evidência desta rodada: docs/09-equipe/evidencias/2026-11-18-container-final.md
#
# Estado em 18/11/2026 (após o merge de crates/godmesh-server em `main`, 70f0a57):
# o build real completa e a imagem roda. A verificação de 17/11/2026 falhou aqui
# porque o pacote ainda não existia ("package ID specification ... did not match
# any packages"); essa seção da evidência foi mantida e marcada como build com stub.

# Base fixada por tag explícita + digest do índice multi-arch: o build não pode
# andar sozinho quando a tag for repontada. Para atualizar, ver a consulta em
# docs/09-equipe/evidencias/2026-11-17-container-prod.md e trocar tag e digest juntos.
ARG RUST_IMAGE=rust:1.100-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
ARG RUNTIME_IMAGE=debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

# ---------- builder ----------
FROM ${RUST_IMAGE} AS builder

WORKDIR /src

ENV CARGO_TERM_COLOR=never \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_RETRY=3 \
    CARGO_PROFILE_RELEASE_STRIP=symbols

# `strip = symbols` via cargo (rustc -Cstrip) em vez de `binutils`: o runtime não
# leva `strip`, e o builder não precisa depender de ferramenta externa.

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

# Healthcheck em script próprio (inspecionável e testável fora da imagem).
COPY deploy/prod/healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod 0555 /usr/local/bin/healthcheck.sh

# BIND_ADDR: o binário exige a variável em produção (GODMESH_ENV=production).
# `0.0.0.0` é o que faz o serviço alcançável de fora do contêiner; o default de
# loopback do binário só vale em GODMESH_ENV=dev.
#
# GODMESH_MIGRATIONS_DIR: reservado; o binário atual NÃO lê esta variável (não
# aplica migração). A imagem só carrega os .sql.
ENV GODMESH_ENV=production \
    BIND_ADDR=0.0.0.0:8080 \
    GODMESH_MIGRATIONS_DIR=/migrations \
    RUST_BACKTRACE=0

USER 10001:10001

# EXPOSE é METADADO: não publica porta, não altera o firewall e não substitui
# `-p 8080:8080`. Quem publica é o operador, na execução.
EXPOSE 8080

# Healthcheck: GET /healthz no bind efetivo (autoritativo — desde 18/11/2026 o
# handler consulta o pool com `SELECT 1`, devolvendo 503 sem banco) + alcance TCP
# do Postgres como diagnóstico secundário. Ver deploy/prod/healthcheck.sh.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/healthcheck.sh"]

# Runtime recomendado (documentado, não aplicado pela imagem):
#   docker run --read-only --cap-drop=ALL --security-opt no-new-privileges \
#              --user 10001:10001 -p 8080:8080 -e DATABASE_URL=... godmesh-server:TAG
# `--read-only` torna o rootfs imutável, inclusive /migrations (o `chmod a-w` só
# vale por DAC e é contornável por root); `--user 0:0` + `--read-only` é o teste
# registrado em docs/09-equipe/evidencias/2026-11-18-gate-d-container.md.
ENTRYPOINT ["/usr/local/bin/godmesh-server"]
