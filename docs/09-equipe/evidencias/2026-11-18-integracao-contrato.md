# Evidência — integração do contrato de API (53ca2ea) no main com cotas/admissão

- **Data:** 2026-11-18
- **Branch:** `room/i3b-integra-contrato-api-82qh` (worktree desta tarefa)
- **Commits:** `e88dbdb` (cherry-pick resolvido de `53ca2ea` sobre `b3623d7`) + commit desta evidência
- **Base do main integrada:** `b3623d7 seg(server): integra cotas/admissão (6b21211) com federação e roteador por domínio`

## O que foi feito

Cherry-pick de `53ca2ea` (registro único de rotas, `GET /v1/meta`, `openapi.json`,
catálogo de erros com `hint`, validação de `Idempotency-Key` e `dry_run`) sobre o
main atual. Conflitos esperados e resolvidos **compondo os dois lados** (nunca
`--ours`/`--theirs` cego):

1. **`crates/godmesh-server/src/http/mod.rs`** — o roteador por domínio do main
   (`routes()`/`public_routes()`) foi substituído pelo `Registry` do contrato,
   **preservando** do main: `AppState.admission: Arc<Admission>`, os portões de
   abuso `gate::http_gate` (camada externa, abaixo do observador) e
   `gate::grant_gate` (entre `require_grant` e o portão de escopo por rota).
   Ordem resultante em rota protegida:
   `observe → http_gate → require_grant → grant_gate → escopo → portão semântico (idempotência/dry_run) → handler`
   — idêntica à que `tests/limits.rs` verifica (11/11 verdes).
   - `build_router(pool, authorization, log, federation, admission: AdmissionConfig, outbox_enabled)` recebe **os dois** anexos.
   - Assinaturas públicas preservadas: `router`, `router_with`, `router_with_log`,
     `router_with_admission`, `router_with_federation`,
     `router_with_federation_and_admission`. A porta do binário é
     `router_runtime(pool, grants, federation: Option<Arc<PeerLinks>>, admission, outbox_enabled)`
     (única que declara `capabilities.outbox_enabled = true`).
2. **`crates/godmesh-server/src/main.rs`** — o boot chama `router_runtime` com os
   três estados efetivos do processo: enlaces de malha (`Option`), cotas
   validadas no boot e flag real do publicador (`outbox.enabled`).
3. **`crates/godmesh-server/src/http/contract.rs`** — `limits.rate` do `/v1/meta`
   deixou de ser `null` fixo: agora é resolvido do **mesmo** `AdmissionConfig`
   que os portões aplicam (`route_class` → `limits_for(classe)` para rotas de
   escopo; `ip_limits` para a rota pública anônima; `null` só onde não há cota
   alguma: `/healthz`, `/v1/meta`, `/openapi.json`). O teto de corpo publicado
   (`routes[].limits.max_body_bytes`) também passou a ser o valor **efetivo** do
   portão (64 KiB default, configurável), não o `DefaultBodyLimit` do axum — a
   constante `MAX_BODY_BYTES` (2 MiB) do commit original foi removida porque com
   admissão ela mentia: quem recusa corpo é o `http_gate`, com outro número.
4. **Rotas de federação no registro** — `/v1/federation/join-requests` (POST
   público, GET `federation:read`), `.../{join_request_id}/approve|reject`
   (`federation:admin`, destrutivas com `dry_run`), `/v1/federation/peers` e
   `/v1/federation/peers/{node_id}/link` (`federation:read`) estão todas no
   `Registry`; o teste estrutural `rota_fora_do_registro_nao_compila` varre
   `src/http/*.rs` (inclusive o `gate.rs` do main) e continua valendo.

## Contagem: rotas meta × router × openapi

`24 × 24 × 24` — as três leituras do mesmo registro, provadas por:
- `tests/api_contract.rs::contrato_publicado_e_o_mesmo_registro_do_router`
  (meta = openapi = sonda no router real, 401 no portão protegido, resposta
  própria no público; `EXPECTED_ROUTE_COUNT = 24` conferido deliberadamente);
- leitura real de `/v1/meta` do binário (amostra abaixo: `rotas: 24`, com as 6
  rotas de federação e escopos corretos).

Amostra do `/v1/meta` **do binário real** (não de router de teste):

```text
rotas: 24
 FED POST /v1/federation/join-requests None
 FED GET /v1/federation/join-requests federation:read
 FED GET /v1/federation/peers federation:read
 FED GET /v1/federation/peers/{node_id}/link federation:read
 FED POST /v1/federation/join-requests/{join_request_id}/approve federation:admin
 FED POST /v1/federation/join-requests/{join_request_id}/reject federation:admin
caps: {'federation_enabled': False, 'host_metrics_available': True, 'outbox_enabled': True, 'retention_classes': [...]}
 LIM POST /v1/incidents {'max_body_bytes': 65536, 'rate': {'bucket': 'grant', 'class': 'incident', ..., 'requests_per_sec': 17.5}}  # GODMESH_LIMIT_REQUESTS_PER_SEC=17.5
```

Com `GODMESH_FEDERATION_BIND` ligado (QUIC real no processo):

```text
federation_enabled: True
POST {'max_body_bytes': 65536, 'rate': {'bucket': 'ip', 'class': 'sync', 'events_per_sec': 500.0, 'requests_per_sec': 500.0, ...}}
GET  {'max_body_bytes': 65536, 'rate': {'bucket': 'grant', 'class': 'incident', 'requests_per_sec': 1000.0, ...}}
godmesh-server: federação escutando em quic://127.0.0.1:56011 (0 par(es) no registro, ALPN godmesh/1)
```

Ou seja: `federation_enabled` vem do runtime anexado; `rate` vem da cota
configurada (17.5 no primeiro boot, default 1000/IP 500 no segundo); o bucket
publicado é o bucket que o portão realmente mede (ip para a rota anônima,
grant para as de escopo).

## Verificações executadas (saída literal)

Ambiente: Postgres local em `localhost:55432` (instância já existente; nenhum
cluster temporário foi criado). `DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh`.

### 1) `cargo test --workspace` — rodada 1

```text
ROUND1_EXIT=0
passed=341 failed=0 ignored=1
```

Detalhe por suíte (todas `ok`; totais conferidos no log):

```text
 unittests src/lib.rs (godmesh_server)          test result: ok. 52 passed; 0 failed
 unittests src/main.rs (godmesh_server)         test result: ok. 15 passed; 0 failed
 tests/api_contract.rs                          test result: ok. 8 passed; 0 failed
 tests/contract.rs                              test result: ok. 17 passed; 0 failed
 tests/limits.rs                                test result: ok. 11 passed; 0 failed
 tests/federation.rs                            test result: ok. 10 passed; 0 failed
 tests/federation_link.rs                       test result: ok. 5 passed; 0 failed
 tests/runtime.rs                               test result: ok. 8 passed; 0 failed
 tests/e2e.rs                                   test result: ok. 7 passed; 0 failed
 tests/admin_grants.rs 6 | authorization.rs 10 | events.rs 8 | inbox.rs 12 | incidents_list.rs 7 | lifecycle.rs 6 | observability.rs 3 | outbox_publicador.rs 8 | retention_policy_missing.rs 0 (+1 ignored, pré-existente)
 demais crates (core, emergency, federation 39 + quic 10, identity, policy, protocol, sync) — todas ok, 0 failed
```

### 2) `cargo test --workspace` — rodada 2

```text
ROUND2_EXIT=0
passed=341 failed=0 ignored=1
```

### 3) `cargo clippy --workspace --all-targets` (após `cargo clean -p godmesh-server -p godmesh-federation`, verificação completa)

```text
CLIPPY_EXIT=0
aviso=0
    Checking godmesh-federation v0.1.0
    Checking godmesh-sync v0.1.0
    Checking godmesh-server v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] (target(s) in 8.12s)
```

### 4) `PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only bash migrations/tests/run_all.sh`

```text
RUNALL_EXIT=0

RESULTADO: OK — 44 invariantes verificados em banco criado do zero.
```

### 5) Sabotagem — rota fora do registro FALHA o teste de contrato

Patch temporário em `mod.rs` (um único `.route()` manual além do registro):

```diff
     let protected = out
         .protected
+        .route("/v1/sonda-sabotagem", axum::routing::get(|| async { "ok" }))
         .route_layer(
```

Saída de `cargo test -p godmesh-server --test api_contract`:

```text
thread 'rota_fora_do_registro_nao_compila' panicked at crates/godmesh-server/tests/api_contract.rs:619:13:
mod.rs:329: rota montada fora do Registry — registre em `contract::Registry` para que /v1/meta e openapi.json nasçam com ela

failures:
    rota_fora_do_registro_nao_compila

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

A sabotagem foi revertida (`git checkout -- crates/godmesh-server/src/http/mod.rs`) e
as duas rodadas acima rodam **depois** da reversão, com a árvore idêntica ao
commit `e88dbdb`.

## Preservação de testes

- **Nenhum teste apagado.** Contagem de atributos `#[test]`/`#[tokio::test]` em
  `crates/`: base `b3623d7` = **330**; após a integração = **342** (+8 de
  `tests/api_contract.rs`, +4 unitários de `http/contract.rs`). A tarefa citava
  329 existentes; o main real tinha 330 e todos continuam executando.
- Os 8 testes de contrato de `53ca2ea` rodam integralmente contra o router
  montado pelo registro com cotas anexadas.

## Diferenças deliberadas em relação a `53ca2ea` (não é cópia cega)

1. `limits.rate` publicado a partir do `AdmissionConfig` efetivo (o commit
   original publicava `null` porque ainda não havia limitador; agora há — o
   `grant_gate`/`http_gate` do main — e `null` seria mentira contratual).
2. `limits.max_body_bytes` idem: teto efetivo do portão (64 KiB default), e a
   constante `MAX_BODY_BYTES` de 2 MiB foi removida (não correspondia mais ao
   que o servidor aplica).
3. `router_runtime` do original foi absorvido pelo parâmetro `admission` e pela
   `GrantRegistry` (mesma porta dos demais construtores do main); o binário tem
   uma única chamada.

## Limitações e não-executado

- Deduplicação de efeito por `Idempotency-Key` continua sendo a próxima fatia
  (validação de fronteira implementada e testada; o documento OpenAPI declara
  isso explicitamente na descrição das rotas mutantes).
- O `?dry_run=true` responde `would_do/applied` sem executar o handler; não há
  plano de efeito detalhado por rota.
- `utoipa-axum` segue inutilizado como no commit original (extrator próprio a
  partir do registro).
- Smoke de `/v1/meta` foi feito contra o banco de dev local (`55432`) e um
  processo efêmero do binário (morto ao final; nenhum cluster Postgres
  temporário criado).
