# Evidência — Listagem paginada de incidentes e observabilidade mínima (18/11/2026)

**Branch:** `room/t2-listagem-e-observabilidade-u534`
**Ambiente:** macOS arm64; `cargo 1.99.1 (5f94df478 2026-08-27)`; `PostgreSQL 18.7 on
aarch64-unknown-linux-musl (Alpine, gcc 15.2.0)` em
`postgres://godmesh:godmesh_dev_only@localhost:55432` — desenvolvimento local-first,
**sem contêiner nesta fase**. Banco próprio desta suíte: **`godmesh_t2`**, criado do zero e
migrado pelo runner oficial (`godmesh_server::migrate::migrate`) — `0001`…`0006`. Nenhum
`DROP`/`TRUNCATE` no banco compartilhado `godmesh`.

**Escopo entregue:** `GET /v1/incidents` (listagem paginada por cursor, escopo
`incident:read`), `GET /metrics` (texto Prometheus, escopo `admin:metrics`), log estruturado
de uma linha por requisição e migração `0006` (chave de paginação).

**Estado do trabalho:** implementação + testes executados. **Não** é declaração de SLO, de
capacidade, de alerta ativo ou de validação em carga — nada disso foi medido nem aprovado.

## 1. Dependências

**Nenhuma dependência nova.** `Cargo.toml` do workspace não foi alterado nesta fatia.
Versões já resolvidas, conferidas em 18/11/2026 (fonte: docs.rs):

```
https://docs.rs/axum/0.8.9/axum/ -> 200
https://docs.rs/sqlx/0.9.1/sqlx/ -> 200
```

O texto Prometheus é formatado à mão (`format!`/`writeln!`) justamente para não trazer
`metrics`/`prometheus` como dependência por ~150 linhas de formatação. O cursor é o de
`godmesh_protocol::cursor` (ADR-057), já no workspace.

## 2. Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `migrations/0006_incident_listing.sql` | **novo**: `sos_incident.list_seq bigint GENERATED ALWAYS AS IDENTITY` (ordem de inserção atribuída pelo banco) + índice `sos_incident_listagem (tenant_id, list_seq DESC)` |
| `migrations/tests/0006_invariants.sql` | **novo**: 3 invariantes (list_seq cresce na inserção; não aceita escrita explícita; índice na ordem publicada) |
| `migrations/tests/run_all.sh` | aplica `0006` e exige os 3 `OK` do arquivo novo (**33 invariantes** no total) |
| `crates/godmesh-server/src/incidents.rs` | **novo**: caso de uso `list_incidents`, filtro `state`, teto `MAX_INCIDENTS_LIMIT = 100`, cursor vinculado a `(tenant, coleção, partição, epoch, principal, filtros)` |
| `crates/godmesh-server/src/metrics.rs` | **novo**: contadores por rota+status, histograma de latência (faixas fixas), gauges de pool/outbox, render no exposition format 0.0.4 |
| `crates/godmesh-server/src/observability.rs` | **novo**: `RequestLog` (`StderrRequestLog`/`CapturedLog`), `RequestContext` (identidade do log), middleware `observe`, sanitização de valores |
| `crates/godmesh-server/src/http.rs` | rota `GET /v1/incidents` no grupo `incident:read`; grupo `admin:metrics` com `GET /metrics`; `AppState` ganha `metrics` e `log`; `router_with_log` para teste; `require_grant` preenche `token_id`/tenant no `RequestContext`; middleware `observe` como camada mais externa |
| `crates/godmesh-server/src/auth.rs` | escopo `Scope::AdminMetrics` (`admin:metrics`), `Scope::ALL` passa a 7 |
| `crates/godmesh-server/src/events.rs` | `cursor_error`/`cursor_token_error` viram `pub(crate)` — reuso, sem mudança de comportamento |
| `crates/godmesh-server/src/lib.rs` | `pub mod incidents`, `metrics`, `observability` |
| `crates/godmesh-server/tests/incidents_list.rs` | **novo**: 7 testes de integração HTTP no fio |
| `crates/godmesh-server/tests/observability.rs` | **novo**: 3 testes de integração HTTP no fio |

Nenhum teste existente foi alterado ou enfraquecido. Nenhum documento normativo
(`docs/07-contratos`, `docs/05-operacao`) foi alterado: esta evidência registra o que foi
**implementado e executado**, não uma decisão nova de contrato.

## 3. Contrato entregue — `GET /v1/incidents`

Escopo exigido: `incident:read`. Tenancy vem do `VerifiedGrant`, nunca da URL
(`deny_unknown_fields` recusa `tenant_id`/`principal` na query).

| Parâmetro | Regra |
|---|---|
| `limit` | **obrigatório**; `1..=100` (`MAX_INCIDENTS_LIMIT`). Ausente, `0` ou `>100` → `422 invalid_request`. O teto é **recusado**, nunca truncado em silêncio. |
| `state` | opcional; validado contra o conjunto fechado do contrato. Desconhecido ou vazio → `422 invalid_request`. |
| `cursor` | opcional; token opaco da ADR-057. Ausente = começa do início. |

**Ordenação estável publicada:** `list_seq DESC` — ordem de inserção, com `list_seq`
atribuída pelo banco (`GENERATED ALWAYS AS IDENTITY`, migração `0006`). A página seguinte é
`list_seq < posição`, então inserção concorrente **não repete nem pula** linha (paginação por
`OFFSET` repetiria). `created_at_unix` acompanha cada item como dado, não como chave.

| Situação | Status | Código |
|---|---|---|
| página do tenant do grant | `200` | `{incidents[], next_cursor}` |
| `next_cursor` nulo | `200` | fim do intervalo (não se emite cursor vazio) |
| cursor de outro tenant | `400` | `cursor_incomparable` |
| cursor de outro filtro (`state`) | `400` | `cursor_incomparable` |
| cursor de outro principal / epoch | `400` | `cursor_invalid` / `cursor_incomparable` |
| cursor ilegível ou de versão desconhecida | `400` | `cursor_invalid` |
| `limit` ausente/`0`/`>100` | `422` | `invalid_request` |
| `state` desconhecido/vazio | `422` | `invalid_request` |
| sem credencial / sem escopo | `401` / `403` | `unauthenticated` / `insufficient_scope` |

Exemplo real (servidor local, banco `godmesh_t2`, `limit=1`):

```http
GET /v1/incidents?limit=1
Authorization: Bearer demo-read-token

200 {"incidents":[{"incident_id":"inc-demo-2","state":"persisted_local",
     "ack_class":"persisted_local","ui_label":"registrado localmente, não enviado",
     "created_at_unix":1795019435}],
     "next_cursor":"v1.MToxfDE0OnRlbmFudC10Mi1kZW1vfDk6aW5jaWRlbnRzfDEyOnByaW5jaXBhbC10Mnw3..."
```

O token carrega `(tenant, "incidents", partição 0, epoch do grant, principal, filtros,
perfil "gmp.incidents.v1", retenção "incidente_sos", último list_seq)`. Não é assinado — como
na leitura de eventos, a query **sempre** filtra por `tenant_id` do grant, logo token
adulterado não amplia escopo.

## 4. Formato da métrica — `GET /metrics`

**Escolha de proteção: escopo `admin:metrics`, não "só loopback".** Justificativa: o processo
escuta em `BIND_ADDR`, que em produção é `0.0.0.0` atrás de proxy — ali todo cliente parece
loopback, e um `/metrics` aberto publicaria volume de tráfego e backlog de outbox para quem
alcançasse a porta. Escopo é verificável, testado no fio e **falha fechado** (`require_grant`
+ `require_scope`), e independe da topologia de rede. `401` sem credencial, `403
insufficient_scope` com credencial sem o escopo.

Séries publicadas (nomes com prefixo `godmesh_`; `route` é sempre o **padrão** registrado, ou
`unmatched`):

```
# TYPE godmesh_http_requests_total counter
godmesh_http_requests_total{route="/v1/incidents",status="200"} 1
# TYPE godmesh_http_request_duration_seconds histogram
godmesh_http_request_duration_seconds_bucket{route="/v1/incidents",le="0.005"} 0
... (0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.500, 5.000, +Inf)
godmesh_http_request_duration_seconds_sum{route="/v1/incidents"} 0.074032
godmesh_http_request_duration_seconds_count{route="/v1/incidents"} 1
# TYPE godmesh_pool_connections gauge
godmesh_pool_connections{state="size"} 2
godmesh_pool_connections{state="idle"} 1
# TYPE godmesh_outbox_pending gauge
godmesh_outbox_pending 0
# TYPE godmesh_metrics_scrape_dependency_up gauge
godmesh_metrics_scrape_dependency_up{dependency="postgres"} 1
```

Decisões explícitas:

* **sem label de tenant, principal, `token_id` ou caminho cru** — métrica agregada não é canal
  de dado pessoal; rota com id por série seria cardinalidade sem teto;
* `+Inf` é implícito e igual ao `_count`; faixas são fixas (10 faixas de 5 ms a 5 s), **não** é
  p95/p99 — percentil exigiria janela e população aprovadas;
* outbox e pool são globais do processo, não por tenant: o gauge não responde "quanto do
  tenant X está pendente";
* leitura de dependência falhou → a série de outbox **não sai** e
  `godmesh_metrics_scrape_dependency_up` marca `0`. Ausência de leitura não vira zero.

## 5. Log estruturado

Uma linha por requisição, emitida pelo middleware externo (portanto cobre 401/403/404 e rota
não casada), formato `chave=valor` separado por espaço:

```
event=http_request request_id=req-4865-18e930b062962248-0 method=GET route=/v1/incidents status=200 duration_ms=74 token_id=demo-inc-read tenant=tenant-t2-demo
event=http_request request_id=req-4865-18e930b0697ad400-2 method=GET route=/metrics status=401 duration_ms=0 token_id=- tenant=-
```

* `token_id` e tenant vêm do grant verificado (`require_grant` preenche o `RequestContext`);
  sem credencial saem como `-` (ausência registrada como ausência);
* **nunca** o token, nunca o corpo, nunca a query, nunca o caminho cru com id;
* todo valor é sanitizado para `[A-Za-z0-9._:@-]` mais `/{}`: nada de espaço, quebra de linha
  ou caractere de controle vindo de configuração forja campo ou linha nova.

## 6. Verificações executadas

### 6.1 `cargo test --workspace` (com `DATABASE_URL` para `godmesh_t2`)

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_t2 \
    cargo test --workspace
...
     Running tests/incidents_list.rs (target/debug/deps/incidents_list-e605bea2f589db73)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s
     Running tests/observability.rs (target/debug/deps/observability-3d8a80c22051f22a)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
...
```

Agregado da execução completa (38 binários de teste): **218 passed, 0 failed**. Exit code `0`.
Nenhuma linha `test result: FAILED` e nenhum bloco `failures:` no log.

O teste pré-existente `tests/retention_policy_missing.rs` continua sendo `ignored` por
desenho (ele altera e restaura a política de retenção e precisa rodar sozinho). Executado
explicitamente:

```
$ DATABASE_URL=… cargo test -p godmesh-server --test retention_policy_missing -- --ignored
running 1 test
test colombian_tenant_without_policy_is_refused_with_retention_policy_missing ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```

### 6.2 `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets
    Checking godmesh-sync v0.1.0 (…/crates/godmesh-sync)
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.44s
```

Exit code `0`, zero warnings (`clippy::all` é `deny` no workspace).

### 6.3 Invariantes de banco (`migrations/tests/run_all.sh`)

Banco descartável criado do zero, `0001`…`0006` aplicadas:

```
===== 0006_invariants.sql =====
NOTICE:  OK 1: list_seq atribuída pelo banco cresce na inserção (8 -> 9)
NOTICE:  OK 2: list_seq não aceita escrita explícita
NOTICE:  OK 3: índice sos_incident_listagem (tenant_id, list_seq DESC) presente

RESULTADO: OK — 33 invariantes verificados em banco criado do zero.
```

### 6.4 Testes de integração desta fatia (o que cada um prova)

`tests/incidents_list.rs`:

1. `lista_somente_incidentes_do_tenant_do_grant` — tenant A vê só A (`[a2, a1]`, mais novo
   primeiro), B vê só B; id de B não aparece na resposta de A;
2. `duas_paginas_com_cursor_nao_repetem_nem_perdem` — 3 incidentes, `limit=2`: página 1 =
   `[inc2, inc1]` + `next_cursor`; página 2 = `[inc0]` com `next_cursor` nulo; união = os 3,
   sem repetição;
3. `cursor_de_outro_tenant_e_cursor_incomparable` — cursor de A com token de B → `400
   cursor_incomparable`;
4. `cursor_de_outro_filtro_e_cursor_incomparable` — cursor sem filtro reapresentado com
   `state=persisted_local` → `400 cursor_incomparable`;
5. `cursor_ilegivel_e_cursor_invalid` — `nao-e-cursor`, `v1.!!!`, `v9.…` → `400 cursor_invalid`;
6. `limit_acima_do_teto_e_recusado_e_nao_truncado` — `101`, `0`, ausente → `422`; `100` → `200`;
7. `filtro_de_estado_restrito_ao_conjunto_fechado` — filtro válido filtra; `state` inventado e
   `state=` → `422`; `tenant_id=` na query → `422`.

`tests/observability.rs`:

1. `metrics_exige_escopo_admin_e_publica_contadores` — `401` sem credencial, `403
   insufficient_scope` sem o escopo, `200` com `admin:metrics`; corpo contém contador da rota
   de incidentes (valor ≥ 1), os `401`/`403` da própria rota `/metrics`, histograma (`_bucket`,
   `_sum`, `_count`) e todos os gauges;
2. `log_registra_identidade_e_nunca_o_token` — linha `200` tem `request_id`, método, rota,
   status, `duration_ms`, `token_id=obs-log-read` e o tenant do grant; linha `401` tem
   `token_id=- tenant=-`; **nenhuma linha contém o token apresentado**;
3. `log_e_metrica_cobrem_rota_nao_casada` — `/v1/nao-existe` aparece como `route=unmatched` no
   log e no contador (nunca como caminho cru).

### 6.5 Demonstração ao vivo (binário real, não teste)

```
$ ./target/debug/godmesh-server    # BIND_ADDR=127.0.0.1:8099, GODMESH_OUTBOX=off
godmesh-server: migrações em migrations — 0 aplicada(s) [], 6 já aplicada(s)
  [0001_core.sql, 0002_incident_tenancy.sql, 0003_outbox_publisher.sql,
   0004_grant_revocation.sql, 0005_sync_inbox.sql, 0006_incident_listing.sql]
godmesh-server: escutando em http://127.0.0.1:8099 (2 grant(s) configurado(s))

$ curl -s -H 'Authorization: Bearer <TOKEN_DO_GRANT>' 'http://127.0.0.1:8099/v1/incidents?limit=1'
{"incidents":[{"incident_id":"inc-demo-2",…}],"next_cursor":"v1.MToxfDE0…"}

$ curl -s -H 'Authorization: Bearer <TOKEN_DO_GRANT>' 'http://127.0.0.1:8099/metrics'
… (séries da seção 4, com `godmesh_http_requests_total{route="/v1/incidents",status="200"} 1`)

$ curl -s -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:8099/metrics'
401
```

O servidor foi encerrado por `SIGTERM` no próprio PID: `godmesh-server: shutdown solicitado;
0 requisição(ões) em voo drenada(s)` / `godmesh-server: encerrado`.

## 7. Limitações e o que **não** foi feito

* Métricas são **por processo e em memória**: reinício zera os contadores; não há exportação
  durável, agregação entre réplicas, `exemplars` nem tracing. Não há SLI/SLO, alerta ativo,
  janela, limiar ou painel — esses parâmetros seguem não aprovados
  (`docs/05-operacao/observabilidade.md`).
* O histograma publica faixas, não percentis. p95/p99 exigiriam população, janela e método
  aprovados; não foram calculados.
* `godmesh_outbox_pending` conta **todas** as entradas não confirmadas do banco, sem recorte
  por tenant; e `godmesh_metrics_scrape_dependency_up` cobre apenas a consulta do outbox (o
  pool é lido do estado local do sqlx, sempre disponível).
* **Sem validação de carga, concorrência de escrita em massa, latência sob pressão ou
  capacidade.** Nenhum número de desempenho foi medido.
* A migração `0006` adiciona coluna e índice a `sos_incident`: em tabela grande o `ALTER TABLE`
  reescreve a tabela e o índice é criado com lock. Não foi ensaiado em volume real — em
  produção isso exige janela e plano de migração. Verificado apenas em banco vazio e no banco
  `godmesh_t2` (pequeno).
* Cursor **não é assinado** (ADR-057): o vínculo é verificado por comparação de escopo +
  filtragem obrigatória por tenant, não por criptografia. Forjar posição dentro do próprio
  escopo é possível; ampliar escopo não é.
* Log vai para `stderr` em texto `chave=valor`; não há coletor, rotação, retenção nem destino
  externo configurado, e não foi medido volume de log por requisição.
* Nenhuma revisão independente de código foi feita nesta fatia (pendente, conforme AGENTS.md).