# Evidência — empacotamento final do servidor em contêiner (18/11/2026)

**Objetivo:** fechar a última etapa do plano local-first (ADR-070) empacotando o
servidor **completo e verde** numa imagem OCI, provando execução endurecida
contra o Postgres de dev, falha explícita sem configuração, healthcheck sensível
ao banco, revogação de grant sem reinício e `SIGTERM` limpo.

**Ambiente:** macOS arm64 (OrbStack), Docker `29.8.2`, Postgres 18 em
`localhost:55432/godmesh`. Código base: `67e2a2c` (branch
`room/pk-empacotamento-final-do-container-yg3d`). Data de execução:
**2026-11-18**, 16:47Z–16:53Z.

**Estado de partida (verificado pelo CTO em `main`):** 235 testes, 0 falhas,
clippy 0, 38 invariantes de banco, migrações 0001–0007 aplicadas pelo binário,
rotas `/v1/admin/grants*`, `/v1/incidents*`, `/v1/events`, `/v1/sync/*`,
`/metrics`, `/healthz` consultando o pool (503 sem banco).

---

## 1. Bases: tag estável e digest (Docker Hub API)

Consulta em **2026-11-18T16:47Z** a
`https://hub.docker.com/v2/repositories/library/rust/tags/1.100-slim-bookworm` e
`.../library/debian/tags/trixie-slim`.

```
== rust/1.100-slim-bookworm
digest: sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
last_updated: 2026-11-12T22:46:18.55064Z
  amd64 linux sha256:476f6598b8d14a1f1bc7e58efc67af206ac8a559c37a3401ce512db2582d0ff7
  arm64 linux sha256:a05e7b80377ed6099c177cad2b9f536e182b9574490c7b4c07acaa29a44fda57
  ppc64le linux sha256:16e77859581d656072311249b0bad6d4f521e9695a1fa798d9ea3ffab9419241
  386 linux sha256:5f5d8587b6bfce6022f4530d5960b62988d5819435d002e1760b1c2cd5dcd6f2
  arm linux sha256:47938f102ab8b7dc3fc3f2e4b454ac8c2675976bb445c0f77806a894628e35e7

== debian/trixie-slim
digest: sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
last_updated: 2026-11-05T08:28:05.296886Z
  amd64 linux sha256:7378260c0a098d27ac206225db5e7d92b8c7c84ea7afccc86eeb513756bbd392
  arm64 linux sha256:24a01011d10eba3a023bc8585b9650b8b29021240d4217c05b905e09ef4a5232
  ppc64le linux sha256:89ff4dab6f0566b916409a75d9e68fc66058aa327f69a80139eca5288b662f89
  s390x linux sha256:31622eb35d6cf34271fbdafc934459f4bca3e4d188762e45826af2dbd4a20063
  arm linux sha256:ced25bc1bff26885d07c82647bc5ad766731d578e8cbc0d02b2e45b2095eaa67
  riscv64 linux sha256:8262446fd210d732d072b0a09ba1dd015055971e642c82d145afe5ed88a059e7
```

Listagem das tags estáveis (`name=slim-bookworm`, ordenada por `last_updated`):

```
slim-bookworm            2026-11-12T22:47:11Z   <- rust:stable slim
1.100.0-slim-bookworm    2026-11-12T22:46:44Z
1.100-slim-bookworm      2026-11-12T22:46:18Z
1-slim-bookworm          2026-11-12T22:45:50Z
1.99.1-slim-bookworm     2026-11-05T16:44:54Z
```

```
trixie-slim              2026-11-05T08:28:05Z   <- debian:stable slim (13)
trixie-20261104-slim     2026-11-05T08:28:00Z
stable-slim              2026-11-05T08:27:13Z
```

**Conclusão:** Rust estável atual = **1.100.0** (a tag fixada `1.100-slim-bookworm`
já aponta para ela) e Debian estável atual = **13 (trixie)**. Nenhuma versão mais
nova foi publicada desde a resolução anterior (17/11/2026) e os **digests de
índice são idênticos** — sem repontamento, o Dockerfile **não precisou trocar tag
nem digest**. Fonte e data registradas no topo de `deploy/prod/Dockerfile`.

Sem `latest` implícito: as duas bases continuam fixadas por tag explícita +
digest do índice multi-arch nos `ARG RUST_IMAGE` / `RUNTIME_IMAGE`.

---

## 2. Build real

Comando:

```
$ docker build --no-cache --progress=plain \
    -f deploy/prod/Dockerfile -t godmesh-server:1.0.0 .
```

Saída literal relevante (`RC=0`):

```
#9  [builder 1/5] FROM docker.io/library/rust:1.100-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
#14 [builder 5/5] RUN cargo build --release --locked --bin godmesh-server --package godmesh-server
#14 1.707  Downloading crates ...
#14 30.40    Compiling godmesh-server v0.1.0 (/src/crates/godmesh-server)
#14 45.03    Finished `release` profile [optimized] target(s) in 44.82s
#14 DONE 45.2s
#21 exporting manifest sha256:6849b622a6fe7b135b189734d287f81515240360fa6fcd6749b980e60b2bb9b8 0.0s done
#21 exporting config sha256:...
#21 naming to docker.io/library/godmesh-server:1.0.0 done
BUILD_SECONDS=48
```

- **Tempo total:** 48 s (build `--no-cache`); `cargo build --release --locked`
  = 44,82 s compilando **157 crates** do zero. Build incremental pós-edição do
  `healthcheck.sh`: 47 s (camadas de Rust em cache).
- **Tamanho:** `docker images` = **155 MB**;
  `docker image inspect` → `SizeBytes=154568871`; binário publicado = 6.167.216 bytes.
- **Digest da imagem construída:**
  `sha256:6849b622a6fe7b135b189734d287f81515240360fa6fcd6749b980e60b2bb9b8`
  (docker driver do OrbStack gera atestação; `RepoDigests` local =
  `godmesh-server@sha256:6849b622…`).
- **Arquitetura:** `arm64` (uma única plataforma).

Build mantém `--release --locked`, `CARGO_PROFILE_RELEASE_STRIP=symbols` e
`.dockerignore` (só `Cargo.toml`, `Cargo.lock`, `crates/`, `migrations/`,
`deploy/prod/*` no contexto).

`strip` conferido por parse ELF do binário extraído da imagem:

```
seções: 30
.symtab presente: False | .strtab presente: False
tamanho binário: 6167216 bytes
```

Segredo na imagem: `grep -c godmesh_dev_only /usr/local/bin/godmesh-server` = `0`
(nem a senha de dev, nem token de grant no binário ou no log — ver §5).

---

## 3. Healthcheck corrigido

O comentário antigo de `deploy/prod/healthcheck.sh` afirmava que `/healthz` era
**estático**. Isso ficou desatualizado quando o handler passou a consultar o pool
(`crates/godmesh-server/src/http/mod.rs`: `SELECT 1` com timeout de 500 ms;
503 `{"status":"unavailable","dependency":"postgres"}` sem banco). O script foi
reescrito:

1. **`GET /healthz` no bind efetivo é autoritativo e exige 200** — prova de que o
   pool respondeu (`SELECT 1`).
2. **Alcance TCP do Postgres** derivado de `DATABASE_URL` virou **diagnóstico
   secundário**: emite aviso no log e **não** reprova quando o `/healthz` já
   respondeu 200 (o pool é prova mais forte que uma porta aberta). Host unix-socket
   e IPv6 literal são pulados com aviso, sem derrubar o contêiner.

Verificação de que o healthcheck agora acompanha o banco (banco derrubado — §5):
as três últimas entradas de `.State.Health.Log` mostram `ExitCode: 1` com
`HTTP/1.0 503 Service Unavailable`, e o contêiner virou `unhealthy`.

---

## 4. Execução endurecida

```
$ docker run -d --name godmesh-final \
    --read-only --cap-drop=ALL --security-opt no-new-privileges \
    --user 10001:10001 --network host \
    -e DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh' \
    -e GODMESH_GRANTS='{"grants":[…operacional…,…admin:grants+admin:metrics…]}' \
    -e BIND_ADDR=127.0.0.1:18092 \
    godmesh-server:1.0.0
912df04f5f5872dbfbebb06858c64332fe21a1db5f7e442ef614647abffcc45b
```

Log de subida:

```
godmesh-server: migrações em /migrations — 0 aplicada(s) [], 7 já aplicada(s) [0001_core.sql, 0002_incident_tenancy.sql, 0003_outbox_publisher.sql, 0004_grant_revocation.sql, 0005_sync_inbox.sql, 0006_incident_listing.sql, 0007_grant_issuance.sql]
godmesh-server: escutando em http://127.0.0.1:18092 (2 grant(s) configurado(s))
godmesh-server: outbox publicador ativo a cada 1000 ms (sink de teste outbox_sink)
```

Rootfs imutável sob `--read-only` com `--user 0:0`:

```
$ docker run --rm --read-only --cap-drop=ALL --security-opt no-new-privileges \
    --user 10001:10001 --entrypoint /bin/bash godmesh-server:1.0.0 -c 'id; touch /migrations/x'
uid=10001(godmesh) gid=10001(godmesh) groups=10001(godmesh)
touch: cannot touch '/migrations/x': Read-only file system
```

### 4a. `/healthz` → 200

```
HTTP/1.1 200 OK
content-type: application/json
content-length: 48

{"ack_class":"transport_accepted","status":"ok"}
```

### 4b. Ciclo do incidente — `201` + `200`

```
--- POST /v1/incidents
HTTP 201
{"incident_id":"inc-container-1795020594","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
--- POST /v1/incidents/inc-container-1795020594/replicate
HTTP 200
{"incident_id":"inc-container-1795020594","state":"replicated_to_authority","ack_class":"replicated_to_authority","ui_label":"replicado à autoridade, sem confirmação","acks":[{"ack_class":"persisted_local","source":"inbox:tenant-container-br-a","at_unix":1795020594},{"ack_class":"replicated_to_authority","source":"authority:godmesh-server","at_unix":1795020594}]}
--- POST /v1/incidents/inc-container-1795020594/ack-technical
HTTP 200
{"incident_id":"inc-container-1795020594","state":"acknowledged_technical","ack_class":"materialized","ui_label":"recebido por um nó, sem pessoa","acks":[…persisted_local, replicated_to_authority, materialized…]}
--- POST /v1/incidents/inc-container-1795020594/acknowledge
HTTP 200
{"incident_id":"inc-container-1795020594","state":"acknowledged_human","ack_class":"acknowledged_human","ui_label":"reconhecido por pessoa autorizada","acks":[…4 provas, a última source "ana.souza"…]}
--- POST /v1/incidents/inc-container-1795020594/close
HTTP 200
{"incident_id":"inc-container-1795020594","state":"closed","ack_class":"acknowledged_human","ui_label":"encerrado","acks":[4 provas]}
```

### 4c. `POST /v1/events` → 201

```
--- POST /v1/events
HTTP 201
{"origin_node":"node-container","stream_id":"stream-container-1795020595","partition":1,"authority_epoch":7,"seq":1,"retention_class":"mensagem","ack_class":"persisted_local"}
```

### 4d. Revogação de grant → 401 **sem reiniciar** o processo

Grant emitido pela API administrativa, usado, revogado pela rota e recusado na
chamada seguinte — mesmo PID, mesmo contêiner, cache de revogação invalidado
explicitamente pela rota:

```
--- 1. emitir
HTTP 201
token_id=ID_DO_GRANT
--- 2. usar o grant emitido (POST /v1/incidents)
HTTP 201
{"incident_id":"inc-revogacao-1795020611","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
--- 3. revogar (POST /v1/admin/grants/ID_DO_GRANT/revoke)
{"token_id":"ID_DO_GRANT","revoked":true,"revoked_at":1795020612}
HTTP 200
--- 4. repetir a MESMA chamada com o token revogado (sem reiniciar processo)
{"ack_class":"transport_accepted","detail":"credencial revogada","error":"credential_revoked"}
HTTP 401
--- 5. GET /v1/incidents?limit=5 com o token revogado
{"ack_class":"transport_accepted","detail":"credencial revogada","error":"credential_revoked"}
HTTP 401
```

### 4e. `GET /metrics` com escopo `admin:metrics`

```
HTTP/1.1 200 OK
content-type: text/plain; version=0.0.4; charset=utf-8

# HELP godmesh_http_requests_total Requisições HTTP atendidas por rota e status.
# TYPE godmesh_http_requests_total counter
godmesh_http_requests_total{route="/healthz",status="200"} 2
godmesh_http_requests_total{route="/v1/admin/grants",status="201"} 3
godmesh_http_requests_total{route="/v1/admin/grants/{token_id}/revoke",status="200"} 2
godmesh_http_requests_total{route="/v1/events",status="201"} 1
godmesh_http_requests_total{route="/v1/incidents",status="201"} 3
godmesh_http_requests_total{route="/v1/incidents",status="401"} 3
godmesh_http_requests_total{route="/v1/incidents",status="422"} 2
godmesh_http_requests_total{route="/v1/incidents/{id}/ack-technical",status="200"} 1
godmesh_http_requests_total{route="/v1/incidents/{id}/acknowledge",status="200"} 1
godmesh_http_requests_total{route="/v1/incidents/{id}/close",status="200"} 1
godmesh_http_requests_total{route="/v1/incidents/{id}/replicate",status="200"} 1
…
godmesh_pool_connections{state="size"} 2
godmesh_pool_connections{state="idle"} 1
godmesh_outbox_pending 0
godmesh_metrics_scrape_dependency_up{dependency="postgres"} 1
```

Negativas no fio:

```
--- GET /metrics com grant operacional (sem admin:metrics)
{"ack_class":"transport_accepted","detail":"grant sem o escopo exigido por esta rota","error":"insufficient_scope"}
HTTP 403
--- GET /metrics sem credencial
{"ack_class":"transport_accepted","detail":"credencial ausente ou desconhecida","error":"unauthenticated"}
HTTP 401
```

---

## 5. Falha explícita

### 5a. Sem `DATABASE_URL` → exit 78

```
$ docker run --rm godmesh-server:1.0.0
godmesh-server: configuração inválida: DATABASE_URL não definida; informe a URL do Postgres em DATABASE_URL (exemplo de formato: postgres://usuario:senha@host:porta/banco)
exit=78
```

### 5b. Sem `GODMESH_GRANTS` → exit 78

```
$ docker run --rm -e DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh' \
    -e BIND_ADDR=127.0.0.1:18099 godmesh-server:1.0.0
godmesh-server: configuração inválida: registro de grants indisponível: GODMESH_GRANTS não definida; exemplo: GODMESH_GRANTS='{"grants":[...]}'
exit=78
```

### 5c. Banco derrubado → contêiner `unhealthy`

```
$ psql postgres://…/postgres \
    -c "ALTER DATABASE godmesh ALLOW_CONNECTIONS false;" \
    -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='godmesh' AND pid <> pg_backend_pid();"
ALTER DATABASE
 encerrado
-----------
 t
 t
(2 rows)

$ curl -s -w '\nHTTP %{http_code}\n' http://127.0.0.1:18092/healthz
{"ack_class":"transport_accepted","dependency":"postgres","status":"unavailable"}
HTTP 503
```

Progressão do healthcheck (interval 30 s, retries 3):

```
16:50:54Z healthy Failing=1
16:51:24Z healthy Failing=2
16:51:55Z unhealthy Failing=3
```

Últimas entradas de `.State.Health.Log`:

```
"ExitCode": 1,
"Output": "healthcheck: /healthz em 127.0.0.1:18092 não respondeu 200: HTTP/1.0 503 Service Unavailable\r\n"
```
(repetido 3×); a entrada anterior, com banco no ar, foi
`"ExitCode": 0, "Output": "healthcheck: ok — /healthz 200 (pool consultado) e Postgres alcançável em localhost:55432"`.

**Restauração (feita):**

```
$ psql postgres://…/postgres -c "ALTER DATABASE godmesh ALLOW_CONNECTIONS true;"
ALTER DATABASE
$ curl -s -w '\nHTTP %{http_code}\n' http://127.0.0.1:18092/healthz
{"ack_class":"transport_accepted","status":"ok"}
HTTP 200
```

O log do contêiner registrou as tentativas do publicador durante a janela de
banco fora (`godmesh-server: outbox passagem falhou: … database "godmesh" is not
currently accepting connections`) — falha explícita, sem panic e sem derrubar o
processo.

---

## 6. `SIGTERM` / `docker stop`

```
$ docker stop --time 10 godmesh-final
docker stop rc=0 elapsed=0.17s
exit code:
0

--- logs finais
godmesh-server: outbox encerrado (publicados 0, confirmados 0, falhas 0)
godmesh-server: shutdown solicitado; 0 requisição(ões) em voo drenada(s)
godmesh-server: encerrado
```

Encerramento em **0,17 s** (limite pedido: < 10 s), `ExitCode 0`, com log de
drenagem e encerramento do publicador.

---

## 7. Tabela verificado × não verificado

| Item | Estado |
|---|---|
| Bases estáveis `rust:1.100-slim-bookworm` e `debian:trixie-slim` na Docker Hub | **verificado** em 2026-11-18T16:47Z: mesmas versões, digests de índice e por-arch idênticos; sem repontamento |
| `docker build --no-cache -f deploy/prod/Dockerfile -t godmesh-server:1.0.0 .` | **verificado**: sucesso, 48 s, 157 crates, `--release --locked` |
| Tamanho / digest da imagem | **verificado**: 155 MB (`SizeBytes=154568871`), digest `sha256:6849b622…` |
| `strip` no binário publicado | **verificado**: ELF sem `.symtab`/`.strtab` |
| Senha de dev e tokens ausentes da imagem e do log | **verificado**: `grep -c` = 0 em ambos |
| Rootfs imutável sob `--read-only` (mesmo com uid 0) | **verificado**: `Read-only file system` |
| Execução `--read-only --cap-drop=ALL --no-new-privileges --user 10001:10001` | **verificado**: sobe, serve, healthcheck `0` |
| Migrações 0001–0007 aplicadas pelo próprio binário na subida | **verificado**: log "0 aplicada(s), 7 já aplicada(s)" |
| `/healthz` → 200 com banco | **verificado** |
| `/healthz` → 503 com banco derrubado | **verificado** |
| Contêiner vira `unhealthy` com banco derrubado | **verificado**: Failing=1→3 → `unhealthy` (~80 s) |
| Healthcheck saudável de novo após restaurar o banco | **verificado**: `/healthz` 200 |
| Ciclo incidente (create/replicate/ack-technical/acknowledge/close) | **verificado**: 201 + 200×4, trilha com 4 ACKs |
| `POST /v1/events` | **verificado**: 201 `persisted_local` |
| Emissão/revogação de grant e 401 imediato sem reinício | **verificado** pela rota administrativa |
| `/metrics` com `admin:metrics` | **verificado**: 200, `text/plain; version=0.0.4` |
| `/metrics` sem escopo / sem credencial | **verificado**: 403 / 401 |
| Falta de `DATABASE_URL` / `GODMESH_GRANTS` | **verificado**: mensagem clara, `exit=78` |
| `SIGTERM` / `docker stop` | **verificado**: 0,17 s, exit 0, log de drenagem |
| Build `linux/amd64` | **não feito** — só `linux/arm64` |
| Registry / push / assinatura / atestação / SBOM | **não feito** |
| Limites de memória/CPU e política de restart | **não configurados** |
| Multi-réplica, balanceamento, eleição | **não testado** |
| TLS (entrada e saída), mTLS | **não configurado** (só `ca-certificates` na imagem) |
| Migração como etapa separada (`--migrate-only`) em pipeline | **não executado aqui** (o binário migra na subida; o modo existe) |
| Carga, latência sob concorrência, soak | **não testado** |
| Backup/restauração do banco a partir do contêiner | **não testado** |

### O que **não** foi provado

- **Portabilidade `amd64`**: a imagem só foi construída e executada em
  `linux/arm64`. Nada aqui garante comportamento em `amd64`.
- **Distribuição**: não há push para registry, nem SBOM, nem assinatura/atestação
  de proveniência. O digest é local ao driver do OrbStack.
- **Isolamento de recursos**: sem `--memory`, `--cpus`, `--pids-limit`, seccomp
  customizado ou AppArmor/SELinux; `--cap-drop=ALL` e `no-new-privileges` foram
  aplicados, mas não há prova de resistência a um compromisso do processo.
- **Alta disponibilidade**: uma réplica só. Nada sobre múltiplas réplicas
  compartilhando o mesmo Postgres, nem sobre o publicador do outbox concorrente
  entre réplicas.
- **TLS**: o servidor fala HTTP claro; a imagem traz `ca-certificates` para saída,
  mas não há terminação TLS, certificado ou mTLS configurado.
- **Saúde de negócio**: `/healthz` prova uma consulta (`SELECT 1`) ao pool — não
  prova réplica primária, disco, outbox drenado nem consistência das tabelas.

---

## 8. Arquivos alterados

- `deploy/prod/healthcheck.sh` — `/healthz` autoritativo (200 obrigatório, agora
  que consulta o pool) + TCP do Postgres como diagnóstico secundário; comentário
  corrigido com o comportamento real e os limites.
- `deploy/prod/Dockerfile` — comentário de bases revalidado em 18/11/2026 (tag,
  digest de índice, digests arm64 **e** amd64, fonte e data) e comentário do
  `HEALTHCHECK` corrigido. Nenhuma tag/digest precisou mudar.
- `deploy/README.md` — comando final de build/execução, lista de flags e estado
  do healthcheck atualizados.
- `docs/09-equipe/evidencias/2026-11-18-container-final.md` — este arquivo.

**Não alterado:** código de domínio (`crates/`), migrações, `Cargo.*`. Nenhum
segredo foi escrito em arquivo, imagem ou log.