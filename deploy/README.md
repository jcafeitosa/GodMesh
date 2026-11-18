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

Verificação dos invariantes no banco — comando único, contra banco criado do zero:

```bash
PGHOST=127.0.0.1 PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
  migrations/tests/run_all.sh
```

O script cria um banco descartável (`godmesh_inv_*`), aplica
`migrations/0001_core.sql` + `migrations/0002_incident_tenancy.sql` +
`migrations/0003_outbox_publisher.sql` + `migrations/0004_grant_revocation.sql` +
`migrations/0005_sync_inbox.sql`, roda `migrations/tests/0001_invariants.sql`,
`0002_invariants.sql`, `0003_invariants.sql`, `0004_invariants.sql` e
`0005_invariants.sql` e falha com exit != 0 se qualquer
bloco `NOTICE: OK` esperado faltar. Esperado:
`RESULTADO: OK — 30 invariantes verificados em banco criado do zero.`

O banco de desenvolvimento compartilhado não é tocado — nada de `DROP`/`TRUNCATE`
nele. Só o banco descartável do próprio script é removido ao final (`--keep`
preserva para inspeção). `--sabotage <arquivo.sql>` aplica SQL extra antes dos
testes, apenas para provar que a falha é detectada.

## Produção — contêiner

`deploy/prod/Dockerfile` é multistage:

- **builder**: `rust:1.100-slim-bookworm` (1.100.0, publicado em 12/11/2026) compila
  `cargo build --release --locked --bin godmesh-server --package godmesh-server`
  com `CARGO_PROFILE_RELEASE_STRIP=symbols` (binário sem `.symtab`/`.strtab`);
- **runtime**: `debian:trixie-slim` (Debian 13, publicado em 05/11/2026), só com
  `ca-certificates`, usuário `godmesh` (uid/gid 10001) sem home e sem shell,
  migrações em `/migrations` com `chmod a-w`, `EXPOSE 8080`, `HEALTHCHECK` em
  `/healthz` + alcance do Postgres, `ENTRYPOINT` do binário.

As bases são fixadas por **tag explícita + digest do índice multi-arch** nos `ARG`
`RUST_IMAGE` e `RUNTIME_IMAGE`. Sem `latest`. Build não anda sozinho quando a tag for
repontada.

### Variáveis lidas pelo binário

| Variável | Lida pelo binário? | Regra |
|---|---|---|
| `DATABASE_URL` | **sim** | obrigatória; ausente/vazia → exit 78 |
| `BIND_ADDR` | **sim** | obrigatória fora de `GODMESH_ENV=dev|development` (exit 78, mensagem cita a variável); em dev o default é `127.0.0.1:8080` |
| `GODMESH_ENV` | **sim** | ausente = `production` (falha fechada: exige `BIND_ADDR`) |
| `GODMESH_NODE_KEY` | **sim** | obrigatória para servir: caminho da identidade Ed25519 do nó. Ausente/vazia → exit 78; arquivo mais aberto que `0600` → exit 78; arquivo ausente é criado `0600`. Persista em volume: identidade nova a cada deploy é outro nó |
| `GODMESH_FEDERATION_BIND` | **sim** | opcional; **desligada por default**. Ligada, abre o listener QUIC/mTLS em `IP:porta` (UDP) e exige `GODMESH_FEDERATION_CLUSTER` e `GODMESH_FEDERATION_REGION` |
| `GODMESH_FEDERATION_CLUSTER` / `_REGION` | **sim** | obrigatórias com o listener ligado (sem default silencioso) |
| `GODMESH_FEDERATION_PEERS_REFRESH_MS` | **sim** | opcional; default `5000`. Intervalo de recarga da allowlist de pares a partir de `federation_peer` |
| `GODMESH_MIGRATIONS_DIR` | **não** | definida no Dockerfile como reserva; o binário atual não aplica migração. **Não** conte com ela |
| `RUST_BACKTRACE` | indireto | só afeta mensagem de panic; `0` na imagem |

`EXPOSE 8080` é **metadado**: não publica porta, não abre firewall e não substitui
`-p 8080:8080`. Quem publica porta é o operador, na execução.

### Build e execução final (18/11/2026)

```bash
# build (versão + digests fixados no topo do Dockerfile; nunca `latest`)
docker build -f deploy/prod/Dockerfile -t godmesh-server:1.0.0 .

docker run -d --name godmesh-server \
  --read-only \
  --cap-drop=ALL \
  --security-opt no-new-privileges \
  --user 10001:10001 \
  --network host \
  -e DATABASE_URL='postgres://USUARIO:SENHA@HOST:5432/godmesh' \
  -e GODMESH_GRANTS="$(cat /caminho/grants.json)" \
  -e BIND_ADDR=127.0.0.1:8080 \
  -v godmesh-node:/var/lib/godmesh \
  godmesh-server:1.0.0
```

Flags de execução e o que cada uma compra:

| Flag | Papel |
|---|---|
| `--read-only` | rootfs imutável; é o **único** controle que resiste a `--user 0:0` (o `chmod a-w /migrations` é só DAC) |
| `--cap-drop=ALL` | nenhuma capability Linux; o servidor só precisa de socket TCP de saída |
| `--security-opt no-new-privileges` | setuid/file-capabilities não elevam privilégio |
| `--user 10001:10001` | uid/gid sem home e sem shell definidos na imagem |
| `--network host` | bind direto; em produção troque por `-p 8080:8080` e `BIND_ADDR=0.0.0.0:8080` |
| `-e DATABASE_URL` | obrigatória: ausente/vazia → mensagem clara, exit 78 |
| `-e GODMESH_GRANTS` | obrigatória para servir: JSON de grants verificados; ausente/inválida → exit 78 |
| `-e BIND_ADDR` | obrigatória fora de `GODMESH_ENV=dev`/`development` (exit 78 citando a variável) |
| `-v godmesh-node:/var/lib/godmesh` | preserva a identidade do nó (`GODMESH_NODE_KEY`) entre execuções; sem volume, cada contêiner é um nó novo |

O binário aplica as migrações na subida (`/migrations`, versionadas na imagem,
idempotentes) antes de escutar. `--migrate-only` aplica e sai 0, para pipeline.

Build de desenvolvimento (mesmo Dockerfile):

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
```

Teste da imagem:

```bash
# 1. sem configuração: falha clara, exit 78 (EX_CONFIG)
docker run --rm godmesh-server:dev

# 2. produção sem BIND_ADDR: exit 78 citando a variável
docker run --rm -e BIND_ADDR= \
  -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  godmesh-server:dev

# 3. servidor real contra o Postgres de dev
docker run --rm --network host \
  -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  -e BIND_ADDR=0.0.0.0:18081 godmesh-server:dev
curl -s http://127.0.0.1:18081/healthz   # {"ack_class":"transport_accepted","status":"ok"}
```

O binário **não** implementa `--help` nem subcomandos: qualquer argumento é ignorado.

### Execução recomendada (flags de runtime)

O `chmod -R a-w /migrations` é **só DAC** e não resiste a `--user 0:0`. Somente
`--read-only` fecha o rootfs:

```bash
docker run --read-only --cap-drop=ALL --security-opt no-new-privileges \
  --user 10001:10001 -p 8080:8080 \
  -e DATABASE_URL=postgres://USUARIO:SENHA@HOST:5432/godmesh \
  godmesh-server:TAG
```

Teste literal (imagem `godmesh-server:gate-d`, 18/11/2026):

```
$ docker run --rm --user 0:0 --entrypoint /bin/bash godmesh-server:gate-d \
    -c 'touch /migrations/x && ls -l /migrations/x; rm -f /migrations/x'
-rw-r--r-- 1 root root 0 Nov 18 13:52 /migrations/x      # root escreveu: chmod não basta

$ docker run --rm --read-only --user 0:0 --cap-drop=ALL \
    --security-opt no-new-privileges --entrypoint /bin/bash godmesh-server:gate-d \
    -c 'touch /migrations/x'
touch: cannot touch '/migrations/x': Read-only file system
```

### O que o `HEALTHCHECK` verifica — e o que não verifica

`deploy/prod/healthcheck.sh` faz duas coisas, com pesos diferentes:

1. `GET /healthz` no **bind efetivo** (`BIND_ADDR`, espelhando a regra do binário),
   exigindo `200` e `"status":"ok"`. **Autoritativo.** Desde 18/11/2026 o handler
   consulta o pool (`SELECT 1`, timeout de 500 ms) e devolve `503
   {"status":"unavailable","dependency":"postgres"}` sem banco — então `200`
   prova consulta bem-sucedida, não apenas porta aberta.
2. **Alcance TCP** do Postgres derivado de `DATABASE_URL`. **Diagnóstico
   secundário**: registra no log se a porta está fechada e **não** reprova o
   contêiner quando o `/healthz` já respondeu `200`.

Limites declarados: `/healthz` prova uma consulta ao pool, não a saúde de tabelas,
réplica primária, disco ou outbox drenado; a checagem TCP não autentica nem
consulta. Host unix-socket e IPv6 literal em `DATABASE_URL` pulam a checagem TCP
com aviso. Evidência: `docs/09-equipe/evidencias/2026-11-18-container-final.md`.

## Estado final do contêiner (18/11/2026)

Imagem `godmesh-server:1.0.0` construída em `linux/arm64` (Docker 29.8.2,
OrbStack), 18/11/2026 16:47Z–16:53Z, código `67e2a2c`. Evidência literal:
`docs/09-equipe/evidencias/2026-11-18-container-final.md`.

| Item | Estado |
|---|---|
| `docker build --no-cache` | **verificado**: 48 s, 157 crates, `--release --locked` |
| Tamanho / digest | **155 MB** (`SizeBytes=154568871`), digest `sha256:6849b622a6fe7b135b189734d287f81515240360fa6fcd6749b980e60b2bb9b8` |
| Bases | **verificado** na Docker Hub API em 18/11/2026: `rust:1.100-slim-bookworm` (1.100.0, 12/11) e `debian:trixie-slim` (13, 05/11), digests idênticos aos anteriores |
| Runtime endurecido (`--read-only --cap-drop=ALL --no-new-privileges --user 10001:10001`) | **verificado**: sobe, serve, healthcheck `0` |
| `/healthz` | **verificado**: 200 com banco; **503** com banco derrubado (consulta o pool) |
| `HEALTHCHECK` | **verificado**: `healthy` com banco; `unhealthy` após 3 falhas com banco fora; volta a `healthy` ao restaurar |
| Ciclo do incidente + `/v1/events` | **verificado** no contêiner: 201/200 |
| Emitir → usar → revogar grant | **verificado**: 201 → 201 → 200 → **401** sem reiniciar |
| `/metrics` | **verificado**: 200 com `admin:metrics`; 403 sem o escopo; 401 sem credencial |
| Falta de `DATABASE_URL` / `GODMESH_GRANTS` | **verificado**: exit 78 com mensagem clara |
| `SIGTERM` | **verificado**: 0,17 s, exit 0, log de drenagem |
| `amd64`, registry, SBOM, assinatura, TLS, multi-réplica, limites de recurso | **não feito** |

## Estado verificado em 18/11/2026 (build real anterior)

Execução em 2026-11-18T13:14Z–13:20Z, code base `70f0a57`. Evidência literal:
`docs/09-equipe/evidencias/2026-11-17-container-prod.md`, seção 6.

> Os números abaixo são da imagem de `70f0a57`. O build do gate D (18/11/2026,
> branch `room/d-container-producao-ahtf`) está em
> [Build do gate D](#build-do-gate-d-18112026) com tamanho próprio.

| Item | Estado |
|---|---|
| `docker build` da imagem de produção | **verificado**: build completo em 135,59 s (125,7 s de `cargo build --release --locked`) |
| Tamanho da imagem real | **155 MB** de disco / **34,1 MB** de conteúdo (154.860.288 bytes) |
| Bases `rust:1.100-slim-bookworm` e `debian:trixie-slim` | **verificado** na Docker Hub API em 18/11/2026: digests `arm64` e datas idênticos aos já registrados, sem repontamento |
| `docker run` sem `DATABASE_URL` | **verificado**: mensagem clara, sem panic, `exit=78` |
| `POST /v1/incidents` com grant válido | **verificado**: `201` com `ack_class: persisted_local` e linha real em `sos_incident` |
| `POST /v1/incidents` com `freshness: none` | **verificado**: `422` `DenyDeniedNoTrustedTime` |
| `POST /v1/incidents` com `share_precise_location` | **verificado**: `422` `DenyNeverOffline` |
| Runtime: uid 10001, sem cargo/rustc/gcc/cc/make | **verificado** na imagem real |
| `/migrations` resistente a root | **verificado** só com `--read-only`; `chmod a-w` sozinho não resiste a `--user 0:0` |
| `BIND_ADDR` obrigatório em produção | **verificado**: exit 78 com mensagem citando a variável |
| Senha de dev fora do binário | **verificado**: `strings target/release/godmesh-server | grep -c godmesh_dev_only` = `0` |
| `strip` no binário publicado | **verificado**: sem `.symtab`/`.strtab` (parse ELF) |
| `GET /healthz` | **verificado**: `200 {"ack_class":"transport_accepted","status":"ok"}`. **Não** consulta o pool |
| `HEALTHCHECK` da imagem | **verificado**: `healthy` com banco no ar; `unhealthy` após derrubar o Postgres (alcance TCP) |
| `SIGTERM` / graceful shutdown | **verificado**: log "encerrado", exit 0 |
| Build multi-arch (`amd64`) | **não feito** — só `linux/arm64` construído |
| Migração aplicada de dentro do contêiner | **não feito** — a imagem carrega `/migrations`, o binário atual não lê `GODMESH_MIGRATIONS_DIR` nem migra |
| Registry, assinatura/atestação, SBOM | **não feito** |
| Limites de memória/CPU, política de restart | **não configurados** |

O estado anterior (18/11/2026, antes de `crates/godmesh-server` existir) usou um build
com **stub**: o `docker build` falhava em `package ID specification 'godmesh-server'
did not match any packages`. Essa seção continua na evidência, marcada como histórica.

Regras que valem em produção:
- usuário sem privilégio (uid 10001), `--read-only`, `--cap-drop=ALL`, `--no-new-privileges`;
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com tag explícita, versão resolvida e
  digest do índice registrados (no topo do Dockerfile e na evidência).

## Build do gate D (18/11/2026)

Imagem `godmesh-server:gate-d` construída em `linux/arm64` (Docker 29.8.2, OrbStack,
macOS arm64), 18/11/2026 13:47Z. Evidência literal completa:
`docs/09-equipe/evidencias/2026-11-18-gate-d-container.md`.

| Item | Estado |
|---|---|
| `docker build` | **verificado**: 108,67 s na primeira execução; reprocesso 3,53 s (cache); 97,20 s após ajuste de formatação do `Dockerfile` |
| Tamanho | **152 MB** de disco / **33,7 MB** de conteúdo (`{{.Size}}` = 152.422.619 bytes, imagem `0b49a111e1cd`); antes: 155 MB / 34,1 MB |
| Arquitetura construída | **só `linux/arm64`** |
| `BIND_ADDR` obrigatório fora de dev | **verificado** local e no contêiner: exit 78 citando a variável |
| `GODMESH_ENV=dev` sem `BIND_ADDR` | **verificado** no contêiner: sobe em `127.0.0.1:8080` e o healthcheck resolve o mesmo endereço |
| Senha de dev no binário | **verificado** ausente: `strings` = `0`; binário da imagem também `0` |
| `strip` | **verificado**: ELF sem `.symtab` e sem `.strtab` (parse dos section headers) |
| Healthcheck com banco no ar | **verificado**: `healthy`, `Failing=0` |
| Healthcheck com Postgres derrubado | **verificado**: `Failing` crescente → `unhealthy` após 3 falhas (retries=3, intervalo 30 s) |
| `/healthz` com banco caído | **responde `200`** nesta imagem histórica (`gate-d`); corrigido depois — ver estado final acima |
| `touch /migrations/x` | **verificado**: falha com `Read-only file system` sob `--read-only`, mesmo com `--user 0:0`; **sucede** só com `chmod a-w` (root) |
| Servidor sob `--read-only --cap-drop=ALL --no-new-privileges --user 10001:10001` | **verificado**: healthcheck `0`, `/healthz` `200` |
| Build `amd64` | **não feito** |

Regras que valem em produção:
- `BIND_ADDR` explícito e publicação de porta feita pelo operador (`EXPOSE` não publica porta);
- usuário sem privilégio (uid 10001), `--read-only`, `--cap-drop=ALL`, `--no-new-privileges`;
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
6. ~~**`/healthz` não consulta o pool**~~ — **resolvido** (18/11/2026): o handler
   faz `SELECT 1` com timeout de 500 ms e devolve `503` sem banco; o
   `healthcheck.sh` exige `200` e usa a checagem TCP só como diagnóstico.
   Evidência: `docs/09-equipe/evidencias/2026-11-18-container-final.md`.
7. **Distribuição**: sem registry, SBOM, assinatura ou atestação; só `linux/arm64`
   construído, sem `amd64`.
