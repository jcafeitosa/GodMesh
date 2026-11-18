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
  migrações em `/migrations` somente-leitura, `EXPOSE 8080`, `ENTRYPOINT` do binário.

Build:

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
```

Teste da imagem (exige o binário já presente na imagem):

```bash
docker run --rm godmesh-server:dev --help
docker run --rm -e DATABASE_URL=postgres://godmesh:senha@host:5432/godmesh godmesh-server:dev
```

### Estado verificado em 18/11/2026

| Item | Estado |
|---|---|
| `docker build` da imagem de produção | **FALHA esperada**: `error: package ID specification 'godmesh-server' did not match any packages` — o pacote ainda não existe |
| Toolchain + `--release --locked --workspace` numa imagem `rust:1.100-slim-bookworm` | **verificado** (sonda descartável, 15,7 s) |
| Runtime: usuário 10001, sem cargo/gcc, `/migrations` read-only, `EXPOSE 8080`, `ENTRYPOINT` | **verificado** (sonda) |
| Tamanho da imagem de runtime | **145 MB** de disco / 31,4 MB de conteúdo — com binário **stub**, não o real |
| `docker run ... --help` do binário real | **não verificado** |
| Falha clara (não panic) por falta de `DATABASE_URL` | **não verificado** — comportamento do binário, que não existe |
| Build multi-arch | **não feito** — só `linux/arm64` construído |

Evidência literal: `docs/09-equipe/evidencias/2026-11-17-container-prod.md`.

Regras que valem em produção:
- porta 8080 exposta, usuário sem privilégio (uid 10001);
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com versão resolvida e digest registrados
  (digests do índice multi-arch estão no topo do Dockerfile e na evidência).

## Pendências registradas

1. **PostGIS**: as imagens `postgis/postgis` 17 e 18 são **amd64-only** (verificado em 17/11/2026). Este Mac é arm64, então o dev roda `postgres:18-alpine` sem PostGIS. O trabalho geoespacial (SPEC-011) fica bloqueado até haver imagem multi-arch homologada ou build próprio.
2. **Postgres 18 e volume**: a imagem exige mount em `/var/lib/postgresql` (não `.../data`). Já corrigido no compose.
3. **Binário de servidor**: depende de `tokio`/`axum`/`sqlx`, não aprovados até aqui.
   Enquanto ele não existir, `docker build -f deploy/prod/Dockerfile` falha no estágio
   builder com `package ID specification 'godmesh-server' did not match any packages`.
   Isso é o estado real, não uma simulação — ver evidência de 18/11/2026.
