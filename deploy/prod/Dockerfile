# Imagem de produção — servidor GodMesh.
# Estado: esqueleto. O binário `godmesh-server` ainda não existe (depende das
# dependências de servidor, não aprovadas até 17/11/2026). Build NÃO verificado.
#
# Regra: dev usa dependências no OrbStack e backend local; produção roda contêiner.

FROM rust:1.99-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
# Quando o binário existir:
# RUN cargo build --release --locked -p godmesh-server
RUN echo "aguardando godmesh-server" && mkdir -p target/release && touch target/release/godmesh-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 godmesh
COPY --from=build /src/target/release/godmesh-server /usr/local/bin/godmesh-server
COPY migrations /migrations
USER godmesh
EXPOSE 8080
ENV GODMESH_ENV=production
ENTRYPOINT ["/usr/local/bin/godmesh-server"]
