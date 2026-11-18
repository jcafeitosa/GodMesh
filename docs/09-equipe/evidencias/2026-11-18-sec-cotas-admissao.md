# Evidência — Cotas, limites de payload e admissão por classe (SEC1, 18/11/2026)

**Branch:** `room/sec1-retry-cotas-e-admissao-ybxh`
**Ambiente:** macOS arm64; `cargo 1.99.1 (5f94df478 2026-08-27)`; `rustc 1.99.1 (58cf8f9c3 2026-10-13)`;
PostgreSQL 18.7 em `localhost:55432`. Suíte nova sobre o banco `godmesh_se` (runner oficial de
migrações; nenhum `DROP`/`TRUNCATE` no banco compartilhado `godmesh`).

**Escopo entregue:** itens 1 e 2 do backlog de risco de
`docs/04-seguranca/abuso-e-malha-maligna.md` §4 — "um grant válido pode inundar o servidor".
Implementado como **guarda antes do handler**, num portão único, sem tocar nos casos de uso:

* `crates/godmesh-server/src/admission.rs` — configuração declarada, tabela rota→classe,
  token buckets por *(tenant, token, classe)*, contador de pressão.
* `crates/godmesh-server/src/http/gate.rs` — dois middlewares: `http_gate` (payload, IP
  pré-autenticação, timeout) e `grant_gate` (cotas por grant, eventos/s, admissão por classe).
* `crates/godmesh-server/src/http/mod.rs` — composição `observe → http_gate → require_grant →
  grant_gate → escopo → handler`; novo construtor `router_with_admission`.
* `crates/godmesh-server/src/main.rs` — `AdmissionConfig` validado na subida (config inválida
  derruba o processo com `EX_CONFIG`, nunca vira default silencioso).
* `crates/godmesh-server/src/runtime.rs` — `ConnectInfo<SocketAddr>` no serve de produção: é o
  que habilita a cota por IP para tráfego **sem** credencial.
* `crates/godmesh-server/tests/limits.rs` — 11 testes de integração no fio (HTTP/1.1 cru).

**Dependências:** nenhuma nova. Axum 0.8.9 e tokio 1.54.1 já resolvidos no `Cargo.lock`
(verificação 18/11/2026, docs.rs/crates.io: axum 0.8.9 é a linha estável usada; `to_bytes`,
`ConnectInfo` e `middleware::from_fn_with_state` são API pública dessa versão). O bucket é
token bucket clássico em `std::sync::Mutex<HashMap>` — sem crate de rate limit.

## 1. Limites → rota → código

Classe vem da rota; o cliente pode **declarar outra** via header `x-godmesh-class` com valores
`sos|ptt|incident|event|sync|metrics` (valor inválido: 422 `invalid_request`). Admin e
federação ficam na classe `incident`: operações que precisam continuar acessíveis sob pressão
para revogar exatamente o grant que inunda. `/healthz` e rotas não casadas ficam isentas de
cota (limitar o healthcheck do LB seria tiro no pé; 404 não tem efeito a limitar).

| Limite | Valor default | Rota | Recusa | Quando |
|---|---|---|---|---|
| Corpo geral | 64 KiB (`GODMESH_MAX_BODY_BYTES`) | `POST/… /v1/incidents*`, `/v1/admin/*`, `/v1/federation/*` | **413 `payload_too_large`** | antes de ler/desserializar (Content-Length) ou lendo com teto (chunked) |
| Corpo de evento | 64 KiB (`GODMESH_MAX_EVENT_BODY_BYTES`) | `POST /v1/events` | **413 `payload_too_large`** | idem |
| Corpo de lote inbox | 256 KiB (`GODMESH_MAX_INBOX_BODY_BYTES`) | `POST /v1/sync/inbox` | **413 `payload_too_large`** | idem (o teto de 100 posições por lote continua sendo do handler, 422) |
| Timeout | 10 s (`GODMESH_REQUEST_TIMEOUT_MS`) | toda rota com política | **504 `request_timeout`** | corpo estável, sem promessa de ausência de efeito; reenvio idempotente é a estratégia declarada |
| Cota de requisições | 1000 rps / burst 1000 por *(tenant, token, classe)* (`GODMESH_LIMIT_REQUESTS_PER_SEC`/`_BURST`, override `GODMESH_LIMIT_<ESCOPO>_RPS/_RBUCKET`) | todas com política | **429 `rate_limited`** + `Retry-After` | antes do handler; sem efeito |
| Cota de eventos | 1000 eps / burst 1000 por *(tenant, token, classe)* (`GODMESH_LIMIT_EVENTS_PER_SEC`/`_BURST`, override `..._EPS/_EBUCKET`) | `POST /v1/events` (custo 1), `POST /v1/sync/inbox` (custo = nº de posições) | **429 `rate_limited`** + `Retry-After` | custo lido do corpo **antes** da desserialização; lote recusado por inteiro, nunca parcial |
| Cota por IP (sem credencial) | 500 rps / burst 1000 (`GODMESH_LIMIT_IP_RPS/_BURST`) | rotas com política, pedido sem `Authorization` | **429 `rate_limited`** + `Retry-After` | requer `ConnectInfo` (produção); ausente, o teto é do edge — declarado |
| Admissão sob pressão | limiar 64 em voo (`GODMESH_ADMISSION_PRESSURE_THRESHOLD`; `0` = sempre) | classe `event`, `sync`, `metrics` | **503 `admission_control`** + `Retry-After` | `sos`, `ptt`, `incident`/admin **nunca** recusados por pressão; buckets por classe = flood de `event` não consome a cota de SOS |

Ordem das classes: `sos > ptt > incident > event > sync > metrics`
(`RequestClass::rank`, com teste unitário que trava a ordem).

**Por que estes valores:** os defaults são tetos altos e *declarados* — calibrados para que a
rajada curta dos E2E existentes (laços de até ~100 pedidos por token por servidor, sequenciais,
com round-trip de banco de ~5–80 ms) não esbarre no portão; baixar qualquer cota é
configuração explícita por escopo. Ninguém calibrou nada por teste de carga — ver §4.

## 2. O que está provado (no fio, HTTP/1.1 cru)

`tests/limits.rs` (11 testes) + 11 testes unitários em `admission.rs`. Evidência bruta do
`--nocapture` (logs do servidor durante a suíte):

```
status=503 route=/v1/sync/state token_id=tok-sync     (classe baixa sob pressão)
status=503 route=/metrics        token_id=tok-metrics (idem)
status=503 route=/v1/events      token_id=tok-eventos (idem, e sem INSERT provado)
status=201 route=/v1/incidents   token_id=tok-a       (SOS sob a MESMA pressão — invariante)
status=413 route=/v1/events      duration_ms=0        (antes de ler corpo; sem INSERT)
status=413 route=/v1/sync/inbox  duration_ms=0        (acima de 256 KiB)
status=422 route=/v1/sync/inbox  duration_ms=9        (200 KiB < 256 KiB: passou pelo portão)
status=401,401,429 /v1/incidents sem Authorization   (cota por IP com ConnectInfo)
status=429 route=/v1/incidents   token_id=tok-a       (tenant A esgotado; tenant B: 201)
status=429 route=/v1/events      duration_ms=0        (rajada; posição 3 não existe no banco)
status=429 route=/v1/sync/inbox  (lote de 20 com burst 10; 0 linhas escritas)
status=504 route=/v1/incidents   duration_ms=301      (pool retido; corpo estável, sem ACK)
```

Contrato antigo preservado: **todos os 263 testes anteriores continuam passando**; o total do
workspace passou a 285 com os novos. As recusas do portão usam o mesmo formato de corpo de
erro do resto da API (`error`, `detail`, `ack_class: "transport_accepted"` — piso honesto:
nada confirmado).

## 3. Saídas literais dos gates

`cargo test --workspace` (com `GODMESH_TEST_DATABASE_URL`/`DATABASE_URL` apontando para
`godmesh_se`):

```
passed: 285 failed: 0        (43 suítes "test result: ok"; 0 FAILED)
```

`cargo clippy --workspace --all-targets`:

```
Finished `dev` profile [unoptimized + debuginfo] target(s) — nenhuma alerta (lints do
workspace: clippy::all deny, inclusive nos alvos de teste)
```

`bash migrations/tests/run_all.sh` (com `PGHOST=localhost PGPORT=55432 PGUSER=godmesh`):

```
RESULTADO: OK — 44 invariantes verificados em banco criado do zero.
```

(Sem migração nova: este portão não persiste nada.)

## 4. O que NÃO está provado (limites declarados)

* **Cota é por processo, não distribuída.** Os buckets vivem na memória de cada servidor; com
  N réplicas atrás de LB, cada uma aplica a sua cota (dano potencial = N × cota). Cota global
  exige estado compartilhado — não existe aqui, e nada nesta fatia finge que existe.
* **Sem teste de carga real.** Os defaults (1000 rps/eps por classe, burst 1000; IP 500/1000;
  pressão 64) são tetos declarados e documentados, não calibrados por medição. A conta de
  requisições em voo usa `AtomicU32` com `SeqCst` — comportamento sob saturação de CPU/threads
  não foi medido.
* **Confiança na classe declarada.** O header `x-godmesh-class` permite ao cliente pedir
  tratamento SOS. Isso não compra escopo nem cota de outra classe (bucket é keyed por classe e
  a autorização continua a mesma), mas o antiabuso de SOS propriamente dito — dedupe, limite
  de aberturas por principal/janela — é o **item 3 do backlog §4** e não foi entregue aqui.
* **Recusa de 413 com corpo não lido:** ao responder 413 por `Content-Length` antes de ler o
  corpo, o servidor fecha a conexão com bytes não lidos; clientes crus podem ver RST em vez
  do 413 (comportamento HTTP padrão de recusa por tamanho — nginx idem; um cliente real
  interrompe o upload ao receber a resposta). Os testes provam o caminho chunked (corpo
  realmente fluindo) e o caminho declarado.
* **Timeout não garante ausência de efeito.** 504 cancela o futuro do handler; uma transação
  já enviada ao Postgres pode ter cometido depois do cancelamento. O corpo diz exatamente
  isso e manda reenviar com a mesma idempotência — nenhuma promessa forte é feita.
* **Poda de buckets é aproximada:** acima de 8192 chaves, entradas ociosas (>5 min) são
  removidas; perder bucket só reinicia a janela daquela chave. Sem teste de esgotamento do
  mapa.
* **Cota por escopo via classe:** a config é por escopo (`Scope::ALL`), mas o bucket é por
  classe; a tabela classe→escopo (`RequestClass::limits_scope`) é escolha declarada, não
  derivada de ADR aprovado.
* Métricas de recusa (contadores `rate_limited_total` etc.) não foram adicionadas ao
  `/metrics` — fica como próximo passo natural de observabilidade do portão.

## 5. Rastreabilidade

| Requisito | Decisão/registrado em | Código | Teste |
|---|---|---|---|
| Payload por rota, 413 antes de desserializar | docs do módulo `admission` §1; envs §1 acima | `gate::http_gate` | `corpo_acima_do_teto…413…sem_insert`, `teto_de_payload_e_por_rota…`, `corpo_chunked…` |
| Timeout por rota, 504 estável | docs `AdmissionConfig::request_timeout` | `http_gate` (tokio::time::timeout) | `handler_lento_alcanca_o_teto_e_recebe_504…` |
| Cotas req/s e eventos/s por escopo, 429+Retry-After | `ScopeLimits`, envs por escopo | `grant_gate` + `Admission::check_*` | `rajada_acima_da_cota…`, `lote_grande_esgota_a_cota_de_eventos…` |
| Chave tenant+token; não autenticado por IP | docs do módulo; `ConnectInfo` em `runtime::serve` | `BucketKey`, `check_anonymous` | `cota_de_um_tenant_nao_afeta_outro_tenant`, `sem_credencial_a_cota_e_por_ip…` |
| Classes `sos>ptt>incident>event>sync>metrics`; guarda antes do handler; SOS nunca recusada por cota alheia | docs §3; tabela `route_class` | `grant_gate` (pressão+buckets por classe) | `sob_pressao_classe_baixa_e_503_enquanto_sos_continua_201`, `flood_de_evento_nao_consome_a_cota_de_sos`, `classes_are_ordered_…` (unit) |
| Não quebrar contrato existente (263) | defaults altos documentados | — | `cargo test --workspace`: 285/285 |
