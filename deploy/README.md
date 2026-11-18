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

`deploy/prod/Dockerfile` é a imagem alvo. **Build não verificado**: o binário `godmesh-server` ainda não existe.

Regras que valem em produção:
- porta 8080 exposta, usuário sem privilégio (uid 10001);
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com versão resolvida e digest registrados.

## Pendências registradas

1. **PostGIS**: as imagens `postgis/postgis` 17 e 18 são **amd64-only** (verificado em 17/11/2026). Este Mac é arm64, então o dev roda `postgres:18-alpine` sem PostGIS. O trabalho geoespacial (SPEC-011) fica bloqueado até haver imagem multi-arch homologada ou build próprio.
2. **Postgres 18 e volume**: a imagem exige mount em `/var/lib/postgresql` (não `.../data`). Já corrigido no compose.
3. **Binário de servidor**: depende de `tokio`/`axum`/`sqlx`, não aprovados até aqui.
