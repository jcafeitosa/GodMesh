# Integração SEC1 (cotas/admissão) sobre main com federação — worker I2

**Branch:** `room/i2-integra-sec1-com-federacao-vsbe`
**Base:** `main` @ `2cedccb` (federação M1+M2, boot com identidade de nó, roteador dividido por domínio)
**Commit integrado:** `6b21211` (branch `room/sec1-retry-cotas-e-admissao-ybxh`)
**Executado:** 18/11/2026, macOS arm64, `rustc 1.99.1` / `cargo 1.99.1`, Postgres 18.7 em `localhost:55432` (bancos **descartáveis** próprios desta execução: `godmesh_i2_sec1_r1`, `godmesh_i2_sec1_r2`, `godmesh_i2_smoke`). **Sem contêiner.** **Sem push.**

> Este documento prova **execução**. A evidência original da fatia está em
> [`2026-11-18-sec-cotas-admissao.md`](2026-11-18-sec-cotas-admissao.md); o que mudou aqui é
> a composição com o que já estava no main.

---

## 1. O problema e como foi resolvido

`6b21211` foi feito sobre `f6125bd`. O main acrescentou depois dele: `db2bed4` (transporte
QUIC/mTLS), `49127f8` (diagnóstico de enlace + `router_with_federation`), `033faa4` (boot com
`GODMESH_NODE_KEY`, listener QUIC e manutenção de pares) e `e19e6d2` (e2e com identidade).
Cherry-pick deu conflito em **`http/mod.rs`** e **`main.rs`** — os dois pontos onde as duas
fatias escreveram o mesmo Estado e a mesma composição.

Método: `git cherry-pick -x --no-commit 6b21211` (sem `--ours`/`--theirs`), depois cada bloco
conflitante foi resolvido **à mão**, por região, mantendo os dois lados quando os dois lados
são contrato. Resultado: `admission.rs`, `http/gate.rs`, `tests/limits.rs`, `lib.rs` e
`runtime.rs` entraram sem alteração; `http/mod.rs` e `main.rs` foram reescritos nos conflitos;
`admission.rs` recebeu **uma** mudança deliberada (tabela de classes de federação — §3).

### 1.1 `AppState` — os dois campos coexistem

```rust
pub struct AppState {
    pub pool: PgPool,
    pub authorization: Arc<Authorization>,
    pub metrics: Metrics,
    pub log: Arc<dyn RequestLog>,
    pub join_window: JoinWindow,
    pub federation: Option<Arc<PeerLinks>>,   // M1/M2 (main)
    pub admission: Arc<Admission>,            // SEC1
}
```

### 1.2 Assinatura final de `build_router` e dos construtores

`build_router` (privado) é a única composição; os públicos só escolhem anexos:

```rust
fn build_router(
    pool: PgPool,
    authorization: Authorization,
    log: Arc<dyn RequestLog>,
    federation: Option<Arc<PeerLinks>>,
    admission: AdmissionConfig,
) -> Router
```

| público | assinatura | usado por |
|---|---|---|
| `router` | `(pool, grants)` | testes de contrato/eventos/inbox/lifecycle/e2e/runtime |
| `router_with` | `(pool, authorization)` | `tests/authorization.rs`, `tests/admin_grants.rs` |
| `router_with_log` | `(pool, authorization, log)` | `tests/federation.rs`, `tests/observability.rs` |
| `router_with_federation` | `(pool, authorization, links: Arc<PeerLinks>)` | `tests/federation_link.rs` |
| `router_with_admission` | `(pool, grants, admission)` | `tests/limits.rs` |
| `router_with_federation_and_admission` | **`(pool, grants, federation: Option<Arc<PeerLinks>>, admission)`** | **binário (`main.rs`)** — novo |

Nenhuma assinatura pública existente mudou: `router_with_federation` continua com 3 parâmetros
e `router_with_admission` continua com 3. O binário passou a usar o construtor que anexa os
dois, o que **substituiu** o `match &runtime_federation { Some(..) => router_with_federation,
None => router }` que, com o `match`, serviria com `AdmissionConfig::default()` e ignoraria o
ambiente validado no boot — exatamente o default silencioso que a regra proíbe.

### 1.3 Ordem dos portões, preservada como anunciada

`http/mod.rs` agora declara a ordem no doc do módulo e ela é verificável no código:

```rust
let protected = incidents::routes()
    .merge(events::routes())
    .merge(sync::routes())
    .merge(admin::routes())
    .merge(federation::routes())
    .merge(metrics_read)
    .route_layer(middleware::from_fn_with_state(state.clone(), gate::grant_gate))
    .route_layer(middleware::from_fn_with_state(state.clone(), require_grant));
// em `route_layer`, a ÚLTIMA camada aplicada é a mais externa → require_grant roda antes.

Router::new()
    .route("/healthz", get(healthz))
    .merge(federation::public_routes())
    .merge(protected)
    .layer(middleware::from_fn_with_state(state.clone(), gate::http_gate))
    .layer(middleware::from_fn_with_state(state.clone(), observe))
    .with_state(state)
// idem: observe é a camada mais externa, http_gate logo abaixo.
```

Ordem por rota `/v1`: **`observe → http_gate → require_grant → grant_gate → portão de escopo →
handler`** — igual à anunciada pela SEC1. Consequência mantida e documentada: o portão de cotas
roda **antes** do portão de escopo, logo um grant válido mas sem escopo também paga cota antes
do 403 (recusar por escopo não é licença para inundar).

### 1.4 `main.rs` — boot com os dois

`Mode::Serve` carrega `federation: Box<FederationRuntime>` **e** `admission: Box<AdmissionConfig>`;
`Config::from_env` valida identidade de nó + listener QUIC + cotas, nesta ordem, e qualquer
falha sai com `EX_CONFIG` (78). `run()` mantém `spawn_peers_maintenance`, o publicador do outbox
e o dreno no SIGTERM, e compõe o router com `router_with_federation_and_admission(pool, grants,
federation_links, *admission)`. O doc do binário ganhou o bloco das variáveis de cota
(`GODMESH_MAX_BODY_BYTES`, `GODMESH_MAX_EVENT_BODY_BYTES`, `GODMESH_MAX_INBOX_BODY_BYTES`,
`GODMESH_REQUEST_TIMEOUT_MS`, `GODMESH_ADMISSION_PRESSURE_THRESHOLD`, `GODMESH_LIMIT_*`), sem
default silencioso declarado em código de teste.

---

## 2. `/v1/meta`

Não existe e não foi criado. `grep '"/v1/meta'` na árvore: nada. A rota de meta continua sendo
trabalho de outra fatia (A1/contrato de API).

---

## 3. Classe declarada das rotas de federação (a única decisão de produto aqui)

Como SEC1 ficou em `f6125bd`, sua tabela tinha um único ramo para `/v1/federation*` → classe
`incident` + teto geral de corpo. Na integração isso foi **aberto por rota**:

| rota | método | classe | por quê |
|---|---|---|---|
| `/v1/federation/join-requests` | `POST` | **`sync`** | é **pública** (sem credencial): um nó que ainda não é par não tem grant. Deixá-la em `incident` diria que o único endpoint anônimo do servidor nunca pode ser recusado por pressão — exatamente o vetor que um nó malicioso usa para inundar o ingresso. `sync` é o degrau mais baixo que ainda descreve "troca com o mundo de fora", e dá a ela bucket **por IP próprio**, separado do tráfego administrativo. |
| `/v1/federation/join-requests` | `GET` | `incident` | leitura de operação, autenticada (`federation:read`). |
| `/v1/federation/join-requests/{id}/approve` \| `/reject` | `POST` | `incident` | é a **decisão administrativa**: precisa continuar alcançável sob pressão para estancar o abuso (revogar o grant que inunda, rejeitar o nó que inunda). |
| `/v1/federation/peers`, `/peers/{id}/link` | `GET` | `incident` | diagnóstico e allowlist, mesmo motivo. |

Sobre a palavra "admin" do briefing: **não existe classe `admin`** na taxonomia aprovada pela
SEC1 (`sos > ptt > incident > event > sync > metrics`), nem no header `x-godmesh-class`, e a
hierarquia é invariante testado (`classes_are_ordered_sos_above_the_rest`). Adicionar uma 7.ª
classe mudaria contrato de header + matriz de escopo de cotas por causa de nome. O degrau que a
fatia já definiu para tráfego administrativo não-emergencial é `incident` (comentário original
da tabela diz exatamente isso), então aprovação de ingresso fica nele. Se o CTO quiser `admin`
como classe separada, é decisão de ADR/taxonomia, não desta integração.

**Limite declarado, não escondido:** o `x-godmesh-class` do cliente é aceito e a classe declarada
troca o bucket usado (SEC1 já documentou isso em `admission.rs`: forjar classe compra
*prioridade de admissão*, não escopo nem cota alheia). Adicionalmente, a recusa por pressão hoje
só existe em `grant_gate`, que roda **depois** de `require_grant` — logo o `POST` público de
ingresso **não é recusado por pressão**, apenas limitado por IP. A classe `sync` o posiciona
corretamente na taxonomia e separa seu bucket; tornar o ingresso anônimo também recusável sob
pressão é mudança de porta de entrada e fica como backlog explícito, não foi feito aqui.

Teste unitário adicionado à tabela (novo, sem apagar nada):
`admission::tests::rota_publica_de_ingresso_e_sync_e_decisao_de_federacao_e_incidente`.

---

## 4. Verificações — saídas literais

### 4.1 `cargo test --workspace` (rodada 1, banco `godmesh_i2_sec1_r1`)

```
$ DATABASE_URL="postgres://godmesh:***@localhost:55432/godmesh_i2_sec1_r1" cargo test --workspace
TOTAL passed=329 failed=0
```

### 4.2 `cargo test --workspace` (rodada 2, banco `godmesh_i2_sec1_r2`)

```
$ DATABASE_URL="postgres://godmesh:***@localhost:55432/godmesh_i2_sec1_r2" cargo test --workspace
SEGUNDA RODADA: passed=329 failed=0 ignored=1
```

### 4.3 Caça a flake — terceira rodada, comparação das listas

Rodou uma terceira vez (mesmo banco `godmesh_i2_sec1_r2`) e comparou a **lista ordenada de nomes** das três:

```
$ for f in run1 run2 run2b; do grep -E '^test .* \.\.\. ok$' /tmp/$f.txt | sort > /tmp/$f.names; done
      329 /tmp/run1.txt.names
      329 /tmp/run2.txt.names
      329 /tmp/run2b.txt.names
IDENTICOS nas 3 rodadas: 329 testes, mesma lista, 0 falha
```

Três rodadas, mesma lista, mesma contagem, 0 falha: **nenhum flake observado**. O `1 ignored` é
`tests/retention_policy_missing.rs` (já era `#[ignore]` no main, roda isolado com `--ignored`).

### 4.4 Contas de teste — nada foi perdido, nada foi enfraquecido

Contagem mecânica de funções de teste (`#[test]`/`#[tokio::test]`, com ou sem argumentos) por
revisão, `crates/` do workspace:

```
main : 307 declaradas  (= 306 executadas + 1 `#[ignore]`)
árvore integrada: 330 declaradas  (= 329 executadas + 1 `#[ignore]`)
TOTAL delta: 23
  crates/godmesh-server/src/admission.rs:  main=0  agora=12  delta=12   (11 da SEC1 + 1 da integração)
  crates/godmesh-server/tests/limits.rs:   main=0  agora=11  delta=11   (SEC1, 11 de fio)
```

306 preservados + 22 da SEC1 + 1 novo desta integração = **329 executando, 0 falha**. Nenhuma
asserção foi apagada, alterada ou enfraquecida: `tests/limits.rs` (796 linhas) e
`http/gate.rs` entraram **byte-a-byte** do `6b21211` (conferido com `diff`), e os 11 testes
unitários de `admission.rs` estão intactos lá dentro. A única mudança em `admission.rs` foi a
tabela `route_class` de federação (§3), o comentário dela, e um teste novo. **Nenhum conflito de asserção apareceu**
— os testes da SEC1 não cobriam `/v1/federation*`, e os 15 testes de federação do main
(`tests/federation.rs` 10, `tests/federation_link.rs` 5) não cobriam cotas. Os dois conjuntos
passaram juntos sem nenhuma reescrita.

### 4.5 `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] in 0.30s
contagem de linhas "warning:"/"error:" na saída: 0
```

### 4.6 `bash migrations/tests/run_all.sh` (duas vezes, mesmo resultado)

```
$ PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=… bash migrations/tests/run_all.sh
RESULTADO: OK — 44 invariantes verificados em banco criado do zero.
EXIT=0
```

### 4.7 O teste de invariante da SEC1, executado explicitamente

```
$ DATABASE_URL="…/godmesh_i2_sec1_r1" cargo test -p godmesh-server --test limits \
      sob_pressao_classe_baixa_e_503_enquanto_sos_continua_201 -- --exact --nocapture

running 1 test
migrações (…/godmesh_i2_sec1_r1): aplicadas [], já presentes 8
event=http_request request_id=… method=GET  route=/v1/sync/state status=503 duration_ms=3 token_id=tok-sync    tenant=tenant-limits-sync
event=http_request request_id=… method=GET  route=/metrics       status=503 duration_ms=2 token_id=tok-metrics tenant=tenant-limits-sync
event=http_request request_id=… method=POST route=/v1/events     status=503 duration_ms=0 token_id=tok-eventos tenant=tenant-limits-eventos
event=http_request request_id=… method=POST route=/v1/incidents  status=201 duration_ms=8 token_id=tok-a       tenant=tenant-limits-a
event=http_request request_id=… method=POST route=/v1/incidents  status=201 duration_ms=7 token_id=tok-a       tenant=tenant-limits-a
test sob_pressao_classe_baixa_e_503_enquanto_sos_continua_201 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.18s
```

Pressão declarada (limiar 0 = sempre sob pressão): `sync` 503, `metrics` 503, `event` 503 **com
zero linhas de evento gravadas**, incidente 201 e **SOS 201** (com a linha `incident_row = 1`
verificada dentro do teste).

### 4.8 Fumaça do binário real — federação e cotas no mesmo processo

Dois processos do binário compilado, banco descartável `godmesh_i2_smoke`, identidade de nó em
arquivo `0600` criado no boot, listener QUIC ligado, `GODMESH_ADMISSION_PRESSURE_THRESHOLD=0`.

(a) config de cota inválida **derruba o boot**, sem servir, sem default silencioso:

```
$ … GODMESH_MAX_BODY_BYTES=0 ./target/debug/godmesh-server
godmesh-server: configuração inválida: configuração de cotas/admissão: GODMESH_MAX_BODY_BYTES precisa ser maior que zero
exit=78
```

(b) boot completo com os dois anexos:

```
godmesh-server: migrações em migrations — 8 aplicada(s) […0001…0008…], 0 já aplicada(s) []
godmesh-server: escutando em http://127.0.0.1:18099 (1 grant(s) configurado(s))
godmesh-server: identidade de nó carregada: node_id=gmn_25657ec8… public_key=d84a4c45…
godmesh-server: federação escutando em quic://127.0.0.1:18443 (0 par(es) no registro, ALPN godmesh/1)
godmesh-server: outbox publicador ativo a cada 1000 ms (sink de teste outbox_sink)
```

(c) requests ao vivo, com a malha ligada e o servidor **sempre sob pressão**:

```
POST /v1/incidents x-godmesh-class: sos              -> 201  {"state":"persisted_local","ack_class":"persisted_local",…}
POST /v1/incidents (classe incident, sem header)     -> 201
POST /v1/events            (classe event)            -> 503  {"error":"admission_control",…}
GET  /v1/sync/state        (classe sync)             -> 503  {"error":"admission_control",…}
GET  /metrics              (classe metrics)          -> 503  {"error":"admission_control",…}
GET  /v1/federation/peers  (admin, classe incident)  -> 200  []
GET  /v1/federation/join-requests (leitura)          -> 200  []
GET  /v1/federation/peers/<id>/link                  -> 200  {"state":"unknown","last_seen_at":null,…}
POST /v1/federation/join-requests/jqr_x/approve      -> 404  {"error":"join_request_not_found",…}
POST /v1/federation/join-requests (pública)          -> 422  (manifesto ilegível recusado pelo handler)
POST /v1/incidents corpo de 70 000 bytes (teto 65 536) -> 413 {"error":"payload_too_large","detail":"corpo acima do teto declarado de 65536 bytes; nada foi lido ou gravado"}
GET  /healthz (sem política de cota)                 -> 200  {"status":"ok"}
```

`SIGTERM` → processo encerrado, dreno reportado. `/v1/federation/peers/<id>/link` respondeu
`unknown` porque o nó nunca discou para um par real (0 pares na allowlist do banco): é o piso
honesto que a fatia de federação prometeu, e ele continua de pé **atrás** do portão de cotas.

---

## 5. O que NÃO foi provado

- **Enlace QUIC sob o portão de cotas.** A fumaça não tem um segundo nó: nenhum par
  `ready`, nenhum handshake real com o tráfego de heartbeat correndo atrás do `http_gate`.
  O que foi provado é que as rotas de diagnóstico funcionam e respondem `unknown`.
- **Cota distribuída.** Os buckets são por processo (documentado em `admission.rs`); N
  réplicas aplicam N cotas. Nada foi provado com múltiplas réplicas.
- **Calibração.** Os defaults (64 KiB / 256 KiB / 10 s / 1000 rps / limiar 64) são tetos
  declarados, não medidos. Nenhum teste de carga foi executado; nenhum número de capacidade
  real foi observado.
- **Ingresso anônimo recusável sob pressão** — §3: hoje o shedding só existe depois de
  `require_grant`. O `POST /v1/federation/join-requests` é limitado por IP, não por admissão.
- **Limite de aberturas de SOS por principal/janela** (`abuso-e-malha-maligna.md` §4 item 3):
  backlog declarado pela SEC1, continua não implementado.
- **Porta de federação na classe `sync` provada por fio.** A escolha de classe tem teste
  unitário de tabela; não há teste de fio para flood de ingresso anônimo.
- **`/v1/meta`** não existe; nada foi inventado no lugar.
- CI, contêiner de produção e deploy: não executados (sem Docker, por instrução).
- `docs/04-seguranca/abuso-e-malha-maligna.md` não foi reescrito: o §4-2 continua o contrato
  que a tabela agora cumpre, e a lacuna do §3 acima fica registrada aqui, não no modelo de
  ameaça.

## 6. Arquivos alterados nesta integração

| arquivo | o que mudou |
|---|---|
| `crates/godmesh-server/src/http/mod.rs` | `AppState` com `federation` **e** `admission`; `build_router` com os dois; `router_with_federation_and_admission` novo; ordem dos portões escrita no doc; conflitos resolvidos |
| `crates/godmesh-server/src/main.rs` | `Mode::Serve` com `federation` **e** `admission`; boot usa o construtor com os dois; doc das variáveis de cota; conflitos resolvidos |
| `crates/godmesh-server/src/admission.rs` | tabela `route_class` aberta por rota de federação (§3) + 1 teste unitário novo |
| `crates/godmesh-server/src/{lib,runtime}.rs`, `src/http/gate.rs`, `tests/limits.rs`, `docs/…/2026-11-18-sec-cotas-admissao.md` | do `6b21211`, sem alteração |
| `docs/09-equipe/evidencias/2026-11-18-integracao-sec1.md` | este documento |
