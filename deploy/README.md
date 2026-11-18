# Desenvolvimento e produção

## Desenvolvimento — OrbStack

Só **dependências de infraestrutura** rodam em contêiner. Backend, frontend e apps rodam locais, como manda a diretriz do projeto.

```bash
docker compose -f deploy/dev/compose.yaml up -d
# Postgres em localhost:55432, banco godmesh, usuário godmesh
```

Migrações são aplicadas na inicialização do volume (`/docker-entrypoint-initdb.d`). Para reaplicar do zero:

```bash
docker compose -f deploy/dev/compose.yaml down -v
docker compose -f deploy/dev/compose.yaml up -d
```

Verificação dos invariantes no banco:

```bash
docker exec -i godmesh-pg psql -U godmesh -d godmesh -f - < migrations/tests/0001_invariants.sql
```

Esperado: cinco `NOTICE: OK`.

## Produção — contêiner

`deploy/prod/Dockerfile` é multistage:

- **builder**: `rust:1.100-slim-bookworm` (1.100.0, publicado em 12/11/2026) compila
  `cargo build --release --locked --bin godmesh-server --package godmesh-server`;
- **runtime**: `debian:trixie-slim` (Debian 13, publicado em 05/11/2026), só com
  `ca-certificates`, usuário `godmesh` (uid/gid 10001) sem home e sem shell,
  migrações em `/migrations` somente-leitura, `EXPOSE 8080`, `HEALTHCHECK` em
  `/healthz`, `ENTRYPOINT` do binário.

As bases são fixadas por **tag explícita + digest do índice multi-arch** nos `ARG`
`RUST_IMAGE` e `RUNTIME_IMAGE`. Sem `latest`. Build não anda sozinho quando a tag for
repontada.

Build:

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
```

Teste da imagem:

```bash
# 1. sem configuração: falha clara, exit 78 (EX_CONFIG)
docker run --rm godmesh-server:dev

# 2. servidor real contra o Postgres de dev
docker run --rm --network host \
  -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  -e BIND_ADDR=127.0.0.1:18081 godmesh-server:dev
curl -s http://127.0.0.1:18081/healthz   # {"ack_class":"transport_accepted","status":"ok"}
```

O binário **não** implementa `--help` nem subcomandos: qualquer argumento é ignorado.

## Estado verificado em 18/11/2026 (build real)

Execução em 2026-11-18T13:14Z–13:20Z, code base `70f0a57`. Evidência literal:
`docs/09-equipe/evidencias/2026-11-17-container-prod.md`, seção 6.

| Item | Estado |
|---|---|
| `docker build` da imagem de produção | **verificado**: build completo em 135,59 s (125,7 s de `cargo build --release --locked`) |
| Tamanho da imagem real | **155 MB** de disco / **34,1 MB** de conteúdo (154.860.288 bytes) |
| Bases `rust:1.100-slim-bookworm` e `debian:trixie-slim` | **verificado** na Docker Hub API em 18/11/2026: digests `arm64` e datas idênticos aos já registrados, sem repontamento |
| `docker run` sem `DATABASE_URL` | **verificado**: mensagem clara, sem panic, `exit=78` |
| `POST /v1/incidents` com grant válido | **verificado**: `201` com `ack_class: persisted_local` e linha real em `sos_incident` |
| `POST /v1/incidents` com `freshness: none` | **verificado**: `422` `DenyDeniedNoTrustedTime` |
| `POST /v1/incidents` com `share_precise_location` | **verificado**: `422` `DenyNeverOffline` |
| Runtime: uid 10001, sem cargo/rustc/gcc/cc/make, `/migrations` read-only | **verificado** na imagem real |
| `GET /healthz` | **verificado**: `200 {"ack_class":"transport_accepted","status":"ok"}` |
| `HEALTHCHECK` da imagem | **verificado**: `Health=healthy`, `Failing=0` |
| `SIGTERM` / graceful shutdown | **verificado**: log "encerrado", exit 0 |
| Build multi-arch (`amd64`) | **não feito** — só `linux/arm64` construído |
| Migração aplicada de dentro do contêiner | **não feito** — a imagem carrega `/migrations`, o binário atual não lê `GODMESH_MIGRATIONS_DIR` nem migra |
| Registry, assinatura/atestação, SBOM | **não feito** |
| Limites de memória/CPU, política de restart | **não configurados** |

O estado anterior (18/11/2026, antes de `crates/godmesh-server` existir) usou um build
com **stub**: o `docker build` falhava em `package ID specification 'godmesh-server'
did not match any packages`. Essa seção continua na evidência, marcada como histórica.

Regras que valem em produção:
- porta 8080 exposta, usuário sem privilégio (uid 10001);
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com tag explícita, versão resolvida e
  digest do índice registrados (no topo do Dockerfile e na evidência).

## Pendências registradas

1. **PostGIS**: as imagens `postgis/postgis` 17 e 18 são **amd64-only** (verificado em 17/11/2026). Este Mac é arm64, então o dev roda `postgres:18-alpine` sem PostGIS. O trabalho geoespacial (SPEC-011) fica bloqueado até haver imagem multi-arch homologada ou build próprio.
2. **Postgres 18 e volume**: a imagem exige mount em `/var/lib/postgresql` (não `.../data`). Já corrigido no compose.
3. **Build `amd64`**: a imagem de produção só foi construída e testada em `linux/arm64`.
   Nenhuma evidência de `amd64` com o binário real.
4. **Migração**: o binário não aplica migração e não lê `GODMESH_MIGRATIONS_DIR`. A
   imagem só carrega os `.sql`; a etapa controlada de migração ainda não existe.
5. **Publicação**: registry, assinatura/atestação e SBOM continuam fora de escopo até
   haver autorização para publicar.
