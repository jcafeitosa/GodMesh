# Evidência — suíte E2E HTTP automatizada (18/11/2026)

**Branch:** `room/sz-e2e-automatizado-y293`
**Ambiente:** macOS arm64; `rustc 1.99.1 (58cf8f9c0 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`;
Postgres `18.7 on aarch64-unknown-linux-musl` em OrbStack —
`postgres://godmesh:godmesh_dev_only@localhost:55432` — desenvolvimento **local-first, sem
contêiner** (ADR-070).

**Escopo entregue:** `crates/godmesh-server/tests/e2e.rs`, a suíte que transforma as **21
verificações** do roteiro manual de `2026-11-18-e2e-http.md` em asserções executáveis (portão),
sem `curl`. Nenhuma linha de `src/` foi alterada: a suíte usa a API existente
(`http::router`, `auth::GrantRegistry::from_entries`, `migrate::migrate`).

## 1. Dependências

**Nenhuma dependência nova — nem de produção, nem de teste.**

A tarefa pedia `reqwest` como dev-dependency *se* houvesse necessidade de cliente HTTP, com
alternativa sem dependência. O menor custo foi **zero dependência**: o repositório já fala
HTTP/1.1 cru sobre `std::net::TcpStream` em `tests/authorization.rs`, `tests/events.rs`,
`tests/inbox.rs` e `tests/runtime.rs`. Reusar esse padrão:

* não muda `Cargo.lock` nem a árvore de build do workspace;
* exercita o **fio** (status, headers, corpo), que é justamente o contrato observável;
* evita trazer `hyper`/`rustls`/`tower` como dependência de teste só para emitir 20 requisições.

Versões consultadas no registry em **18/11/2026** (`https://crates.io/api/v1/crates/<crate>`,
campo `max_stable_version`), para registrar a decisão com data e fonte:

```
reqwest: 0.13.5     (consultada e NÃO adotada: custo sem benefício para este caso)
axum:    0.8.9      (já em crates/godmesh-server/Cargo.toml — sem mudança)
tokio:   1.54.1     (já em crates/godmesh-server/Cargo.toml — sem mudança)
sqlx:    0.9.1      (já em crates/godmesh-server/Cargo.toml — sem mudança)
```

Uso das APIs nas versões instaladas, conforme documentação oficial (docs.rs, 18/11/2026):
`axum::serve` + `Router` em porta efêmera (`tokio::net::TcpListener::bind("127.0.0.1:0")`),
`Router::merge`/`route_layer` como em `src/http.rs`, `sqlx::AssertSqlSafe` para o DDL do banco
descartável (identificador não aceita bind), `tokio::sync::OnceCell` para o setup único.

## 2. Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `crates/godmesh-server/tests/e2e.rs` | **novo**: 7 testes de integração que cobrem as 21 verificações do roteiro manual + banco descartável + cliente HTTP/1.1 cru + verificação de efeito por SQL |
| `docs/09-equipe/evidencias/2026-11-18-e2e-automatizado.md` | **novo**: esta evidência |

Nada em `src/`, `migrations/` ou em outro teste foi tocado ou enfraquecido. `Cargo.toml` /
`Cargo.lock` inalterados.

## 3. Como a suíte monta o ambiente

1. **Banco descartável** `godmesh_e2e_<pid>`, criado do zero: `DROP DATABASE IF EXISTS … WITH
   (FORCE)` + `CREATE DATABASE …` pela conexão de administração (banco `postgres`). O banco de
   desenvolvimento `godmesh` nunca é tocado.
2. **Migrações pelo runner oficial** — `common::pool_para` chama
   `godmesh_server::migrate::migrate` (a mesma função de `--migrate-only` e da inicialização),
   com o diretório `migrations/` do repositório. Ordem, checksum e `schema_migration` são os de
   produção.
3. `tenant_region` para **dois tenants** (`tenant-e2e-br-a`, `tenant-e2e-br-b`), jurisdição
   `BR`; classes `mensagem` e `incidente_sos` conferidas com política + cópia primária.
4. **Router real** (`godmesh_server::http::router`, composição de produção: `SystemClock` +
   revogação em Postgres) em **porta efêmera**, com `GrantRegistry::from_entries` de dois
   grants (`g-e2e-a`, `g-e2e-b`, todos os escopos, `authority_epoch=7`).
5. **Cliente HTTP/1.1 cru** sobre `TcpStream` (`Connection: close`).
6. **Efeito verificado por SQL**, não pela resposta: todo caminho negativo confere
   `count(*)` em `sos_incident`, `sos_ack` e `event_position`.

Cada teste abre o próprio `PgPool` e fecha ao sair: um pool estático pertenceria ao runtime do
primeiro teste, e os demais herdariam conexões de um driver já derrubado.

**Sem `#[ignore]`** e sem asserção comentada. Sem URL de banco declarada
(`GODMESH_TEST_DATABASE_URL`/`DATABASE_URL`), os testes são pulados com aviso explícito — a
mesma convenção do restante da suíte; com URL declarada e banco inacessível, eles **falham**
(ambiente declarado e quebrado não pode parecer verde).

## 4. Mapa verificação → teste

7 testes cobrem as 21 verificações (a coluna "verificação" é a numeração do roteiro manual de
`2026-11-18-e2e-http.md`).

| # | Verificação do roteiro | Teste | Asserção executada |
|---|---|---|---|
| 1 | `POST /v1/incidents` sem credencial | `sem_credencial_e_401_e_nao_insere` | `401`, `error=unauthenticated`, `ack_class=transport_accepted`, `WWW-Authenticate: Bearer`, corpo não ecoa o `incident_id`; SQL: **0** linhas em `sos_incident` e **0** em `sos_ack` |
| 2 | `POST /v1/incidents` com credencial | `incidente_do_persistido_ao_closed_com_trilha_de_quatro_acks` | `201`, `state=persisted_local`, `ack_class=persisted_local`, `ui_label="registrado localmente, não enviado"` |
| 3 | replay do mesmo `incident_id` | idem | `200` (não `201`), mesmo estado; SQL: 1 incidente e 1 ACK (sem duplicar) |
| 4 | `/replicate` | idem | `200`, `state=replicated_to_authority` |
| 5 | `/ack-technical` | idem | `200`, `state=acknowledged_technical`, `ack_class=materialized`, trilha com 3 ACKs |
| 6 | `/acknowledge` sem `responder` | idem | `422`, `error=missing_human_proof`; SQL: estado ainda `acknowledged_technical` e 3 ACKs (recusa sem efeito) |
| 7 | `/acknowledge` com `responder` | idem | `200`, `state=acknowledged_human`, `ack_class=acknowledged_human` |
| 8 | `/close` | idem | `200`, `state=closed`, `ack_class=acknowledged_human`, `ui_label="encerrado"`, trilha com **4** ACKs (`persisted_local`, `replicated_to_authority`, `materialized`, `acknowledged_human`), todo `at_unix > 0` e `source` não vazio; SQL: 4 linhas em `sos_ack` |
| 9 | `POST /v1/events` seq 1 | `evento_persistido_replay_lacuna_classe_e_cursor` | `201`, `ack_class=persisted_local`, `seq=1`, `retention_class=mensagem` |
| 10 | evento duplicado | idem | `200`, mesmo `ack_class`; SQL: 1 linha |
| 11 | salto de sequência (seq 9) | idem | `409`, `error=sequence_gap`, `detail` contém `"espera 2"`; SQL: 1 linha (lacuna não é saltada nem preenchida) |
| 11b | lacuna no lote de Inbox (extra, exigido pelo pedido "409 com `expected`") | `inbox_aceita_duas_posicoes_e_le_o_prefixo` | `409`, `error=sequence_gap`, `expected=3`, `received=5`, `accepted=0`, `contiguous_prefix=2`; SQL: 2 linhas |
| 12 | classe de retenção inexistente | `evento_persistido_replay_lacuna_classe_e_cursor` | `409`, `error=retention_policy_missing`; SQL: 1 linha (nada gravado) |
| 13 | `GET /v1/events` | idem | `200`, `events[]` com 1 item (`seq=1`, `jurisdiction=BR`), `next_cursor` presente; retomada pelo cursor → `seq=2`, `next_cursor=null` |
| 14 | `POST /v1/sync/inbox` (2 posições) | `inbox_aceita_duas_posicoes_e_le_o_prefixo` | `200`, `accepted=2`, `duplicates=0`, `contiguous_prefix=2`, `ack_class=persisted_local`; SQL: 2 linhas |
| 15 | `GET /v1/sync/state` | idem | `200`, `contiguous_prefix=2`, `gaps=[]`, `last_recorded_at` presente |
| 16 | cursor ilegível | `evento_persistido_replay_lacuna_classe_e_cursor` | `400`, `error=cursor_invalid`; SQL: 2 linhas (leitura recusada não grava) |
| 17 | token desconhecido | `credencial_de_outro_tenant_nao_le_incidente_alheio` | `401`, `error=unauthenticated` |
| 18 | leitura com credencial de outro tenant | idem | **(a)** token de outro tenant **fora do registro** → `401 unauthenticated` (reproduz o roteiro manual: a credencial inválida barra antes da tenancy). **(b)** token de outro tenant **válido e registrado** → `404 incident_not_found`, sem nomear o tenant dono (a variante que de fato prova isolamento por tenant) |
| 19 | incidente inexistente | `transicao_apos_closed_e_409_e_incidente_inexistente_e_404` (+ o 404 cross-tenant de 18b) | `GET` → `404 incident_not_found`; `POST /close` em inexistente → `404 incident_not_found` (nunca `409`); SQL: 0 linhas |
| 20 | transição a partir de `closed` | idem | `POST /close` → `409 illegal_transition` com `state=closed` no corpo e `detail` citando o estado; `POST /replicate` → mesmo `409`; SQL: estado continua `closed` e **4** ACKs (nenhuma prova nova) |
| 21 | `SIGTERM` | `binario_encerra_por_sigterm_com_codigo_zero` | binário real (`CARGO_BIN_EXE_godmesh-server`) no mesmo banco, `BIND_ADDR=127.0.0.1:0`; `/healthz` `200 status=ok` antes do sinal; `kill -TERM` → **código 0**, stdout com `em voo drenada` e `encerrado` |

Cobertura: **21 de 21** verificações, mais 11b (lacuna de Inbox com `expected` explícito) e 18a
(reprodução literal do 401 do roteiro). Nenhuma verificação foi omitida, reduzida a `#[ignore]`
ou trocada por "smoke test".

## 5. Verificações realmente executadas

### 5.1 Suíte E2E isolada

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_gate \
    cargo test -p godmesh-server --test e2e -- --nocapture

running 7 tests
migrações (postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_e2e_99404): aplicadas ["0001_core.sql", "0002_incident_tenancy.sql", "0003_outbox_publisher.sql", "0004_grant_revocation.sql", "0005_sync_inbox.sql"], já presentes 0
e2e: banco descartável pronto em postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_e2e_99404
test sem_credencial_e_401_e_nao_insere ... ok
test credencial_de_outro_tenant_nao_le_incidente_alheio ... ok
test inbox_aceita_duas_posicoes_e_le_o_prefixo ... ok
test incidente_do_persistido_ao_closed_com_trilha_de_quatro_acks ... ok
test evento_persistido_replay_lacuna_classe_e_cursor ... ok
test transicao_apos_closed_e_409_e_incidente_inexistente_e_404 ... ok
test binario_encerra_por_sigterm_com_codigo_zero ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.89s
```

As linhas `godmesh-server: auth event=…` do audit sink (stderr) foram omitidas desta transcrição;
a saída bruta está em `/tmp/e2e-run.log` da execução. Elas só trazem `token_id`, nunca o token.

### 5.2 `cargo test --workspace`

Banco dedicado da execução (`godmesh_gate`, criado do zero antes de rodar) para que o portão não
dependa de estado do banco de desenvolvimento:

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_gate \
    cargo test --workspace
…
     Running tests/authorization.rs (target/debug/deps/authorization-391e79303f50e23a)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running tests/contract.rs (target/debug/deps/contract-b95952bf844d7a7d)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.41s
     Running tests/e2e.rs (target/debug/deps/e2e-cf1822e666430498)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
     Running tests/events.rs (target/debug/deps/events-507b945ac39bb1c7)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/inbox.rs (target/debug/deps/inbox-1c47ce6ebf508ad6)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
     Running tests/lifecycle.rs (target/debug/deps/lifecycle-873869d5960d783c)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
     Running tests/outbox_publicador.rs (target/debug/deps/outbox_publicador-f9985b50c15f609a)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.37s
     Running tests/retention_policy_missing.rs (target/debug/deps/retention_policy_missing-87118095263ee653)
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/runtime.rs (target/debug/deps/runtime-95f1907507fd233f)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.34s
…
```

Total: **36 blocos `test result:`**, nenhum `FAILED`; **201 testes passaram**, 0 falharam, e o
único `ignored` é o `retention_policy_missing` que **já** era `#[ignore]` antes desta fatia (ele
altera o registro de política em volta da chamada, por desenho). Log completo:
`/tmp/workspace-test.log`.

### 5.3 `cargo clippy --workspace --all-targets`

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_gate \
    cargo clippy --workspace --all-targets
…
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.03s
```

Sem nenhum diagnóstico. O workspace nega `clippy::all` (`Cargo.toml`, `[workspace.lints.clippy]`),
então "finished sem diagnóstico" é o resultado limpo, incluindo o novo `tests/e2e.rs`.

### 5.4 Prova de que o portão morde (mutação controlada)

Um teste que passa não prova que ele detecta regressão. Duas asserções foram mutadas de
propósito e **falharam**, e o arquivo foi restaurado em seguida:

```
assertion `left == right` failed: MUTACAO: 401 nao pode inserir incidente
test sem_credencial_e_401_e_nao_insere ... FAILED
assertion `left == right` failed: MUTACAO: trilha precisa ter 4 ACKs: {"ack_class":"acknowledged_human","acks":[{"ack_class":"persisted_local","at_unix":1795018773,"source":"inbox:tenant-e2e-br-a"},…],"state":"closed"}
test incidente_do_persistido_ao_closed_com_trilha_de_quatro_acks ... FAILED
test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.82s
```

Depois da restauração, a suíte volta a `7 passed; 0 failed` (§5.1).

## 6. Limitações (o que esta evidência NÃO prova)

* **Banco de desenvolvimento não é alvo**: o E2E usa `godmesh_e2e_<pid>`; nada aqui mede dados
  reais nem migração de banco existente com dados.
* **O banco descartável permanece no Postgres local** ao fim da execução (nome com `pid`) e é
  recriado do zero na execução seguinte. Não há `DROP` automático no fim porque o binário de
  teste não tem um ponto único de encerramento confiável; o `DROP … WITH (FORCE)` no início é o
  que garante estado limpo.
* **Uma réplica, um processo, loopback HTTP/1.1**: sem TLS, sem HTTP/2, sem carga, sem latência,
  sem concorrência entre clientes reais. A concorrência de transições é coberta por
  `tests/lifecycle.rs`, não aqui.
* **Revogação em banco durante o fluxo** continua coberta por `tests/authorization.rs`; esta
  suíte usa `router` com revogação real, mas não revoga no meio do roteiro.
* **Outbox desligado** no processo do teste 21 (`GODMESH_OUTBOX=off`), igual ao roteiro manual;
  o publicador é coberto por `tests/outbox_publicador.rs`.
* **Sem URL de banco a suíte é pulada** (convenção do repositório). O portão de CI precisa
  exportar `DATABASE_URL`/`GODMESH_TEST_DATABASE_URL` com Postgres acessível; sem isso o
  resultado "verde" **não** significa E2E verificado.
* `SIGTERM` depende de Unix (`#[cfg(unix)]`) e do utilitário `kill` no PATH — no macOS e Linux
  usados aqui, ambos presentes.

## 7. Rastreabilidade

requisito (roteiro manual de 18/11/2026, 21 verificações) → decisão (ADR-070 local-first, sem
contêiner; nenhuma dependência nova) → código (`crates/godmesh-server/tests/e2e.rs`) → teste
(§4) → evidência (§5).