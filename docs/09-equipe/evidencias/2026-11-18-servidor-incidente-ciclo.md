# Evidência — ciclo de vida do incidente no servidor (18/11/2026)

**Origem:** worker SC sob o CTO — rotas de ciclo de vida do incidente SOS
(`GET /v1/incidents/{id}`, `replicate`, `ack-technical`, `acknowledge`, `close`).
**Escopo:** `crates/godmesh-server/` (rotas, casos de uso, testes) e este registro.
**Fora de escopo:** `migrations/` (nenhuma alteração de esquema necessária),
`crates/godmesh-emergency`, `crates/godmesh-sync`, `deploy/`, Docker, SPEC nova.

**Ambiente:** macOS arm64; `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1`;
Postgres 18 (`psql (PostgreSQL) 18.6`) em `localhost:55432`, banco **próprio**
`godmesh_sc` criado do zero com `migrations/0001_core.sql` +
`migrations/0002_incident_tenancy.sql`. `axum` **0.8.9** e `sqlx` **0.9.1**
resolvidos no `Cargo.lock` (fonte: crates.io, consultados em 18/11/2026).
**Nenhuma dependência nova.**

## 1. O que foi implementado

| Rota | Transição | ACK gravado na trilha | Recusas |
|---|---|---|---|
| `GET /v1/incidents/{id}` | — | — | `404 incident_not_found` (inexistente **e** de outro tenant) |
| `POST /v1/incidents/{id}/replicate` | `persisted_local → replicated_to_authority` | `replicated_to_authority` | `409 illegal_transition` |
| `POST /v1/incidents/{id}/ack-technical` | `replicated_to_authority → acknowledged_technical` | `materialized` | `409 illegal_transition` |
| `POST /v1/incidents/{id}/acknowledge` | `acknowledged_technical → acknowledged_human` | `acknowledged_human` | `422 missing_human_proof`, `409 illegal_transition` |
| `POST /v1/incidents/{id}/close` | `acknowledged_human → closed` | nenhum (o ACK humano permanece) | `409 illegal_transition` |

Resposta de sucesso (todas as rotas de estado, incluindo `GET`):

```json
{"incident_id":"…","state":"…","ack_class":"…","ui_label":"…",
 "acks":[{"ack_class":"…","source":"…","at_unix":1770000000}]}
```

Corpo de erro (inalterado na forma, com `state` só quando ajuda a agir):

```json
{"error":"illegal_transition","detail":"transição ilegal a partir de persisted_local",
 "ack_class":"transport_accepted","state":"persisted_local"}
```

## 2. Regras duras e onde cada uma vive

1. **Tenancy só do grant.** `tenant_id` sai de `VerifiedGrant`; o caminho HTTP
   traz apenas o `id`. Incidente de outro tenant e inexistente devolvem o mesmo
   `404` — `403` confirmaria a existência do identificador alheio.
2. **`ack_class` derivado do estado.** `state`/`ack_class`/`ui_label` vêm de
   `godmesh_emergency::incident::Incident` reidratado a partir do estado
   persistido (`api::rehydrate`). Nenhum campo do pedido escolhe classe.
3. **Trilha append-only.** Só `INSERT` em `sos_ack`; cada transição grava
   `at_unix` real do relógio do host (> 0) e `source` do emissor da evidência:
   `authority:godmesh-server` nas fronteiras técnica/autoridade,
   `responder` em `acknowledged_human`. O trigger `sos_ack_append_only` do banco
   recusa `UPDATE`/`DELETE` (provado no teste 6).
4. **Concorrência.** `UPDATE sos_incident SET state = $destino
   WHERE tenant_id = $1 AND id = $2 AND state = $esperado RETURNING …`. A linha é
   travada pelo `UPDATE`; a segunda chamada reavalia o `WHERE` contra o estado já
   commitado (READ COMMITTED/EvalPlanQual), recebe 0 linhas e vira
   `409 illegal_transition`. O ACK só é inserido depois do `UPDATE` bem-sucedido,
   na mesma transação, então nunca há ACK duplicado.
5. **Sem promessa de entrega/atendimento.** `close` não cria ACK de entrega;
   `ui_label` de `acknowledged_technical` continua "recebido por um nó, sem
   pessoa"; nenhuma rota aceita ou produz `delivered_to_device`.
6. **Prova de pessoa.** `acknowledge` com corpo ausente, vazio, ilegível ou com
   `responder` só de espaços → `422 missing_human_proof`, antes de qualquer
   efeito (nem lock, nem transição, nem ACK).

## 3. Arquivos alterados

```
crates/godmesh-server/src/lifecycle.rs          (novo)
crates/godmesh-server/src/api.rs                (3 variantes de erro; helpers pub(crate))
crates/godmesh-server/src/http.rs               (5 rotas, handlers, `state` no corpo de erro)
crates/godmesh-server/src/lib.rs                (módulo `lifecycle`)
crates/godmesh-server/tests/lifecycle.rs        (novo; 6 testes de integração)
docs/09-equipe/evidencias/2026-11-18-servidor-incidente-ciclo.md (este arquivo)
```

Nada em `migrations/`, `deploy/`, `godmesh-emergency`, `godmesh-core`,
`godmesh-policy`, `godmesh-protocol`, `godmesh-identity`, `godmesh-sync` foi tocado.

## 4. Banco de teste próprio

```console
$ psql postgres://…@localhost:55432/postgres -c 'CREATE DATABASE godmesh_sc'
CREATE DATABASE
$ psql postgres://…@localhost:55432/godmesh_sc -v ON_ERROR_STOP=1 \
      -f migrations/0001_core.sql -f migrations/0002_incident_tenancy.sql
… (0001) CREATE TABLE … CREATE TRIGGER … INSERT 0 7 … INSERT 0 8
… (0002) BEGIN … UPDATE 0 … COMMIT
```

Os testes usam `DATABASE_URL` do banco `godmesh_sc`; sem ela são ignorados em vez
de fingir sucesso. O banco de dev compartilhado (`godmesh`) não foi tocado.

## 5. `cargo test --workspace` (literal)

```console
$ DATABASE_URL=postgres://godmesh:go…@localhost:55432/godmesh_sc cargo test --workspace
     Running tests/integration.rs (target/debug/deps/integration-…)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/incident.rs (target/debug/deps/incident-…)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/grant_predicate.rs (target/debug/deps/grant_predicate-…)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/high_water.rs (target/debug/deps/high_water-…)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/retention.rs (target/debug/deps/retention-…)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_class.rs (target/debug/deps/ack_class-…)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_composition.rs (target/debug/deps/ack_composition-…)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cursor.rs (target/debug/deps/cursor-…)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/version.rs (target/debug/deps/version-…)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_server-…)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/godmesh_server-…)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/contract.rs (target/debug/deps/contract-…)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.14s
     Running tests/lifecycle.rs (target/debug/deps/lifecycle-…)
running 6 tests
test concurrent_same_transition_yields_exactly_one_success_and_one_409 ... ok
test ack_trail_is_append_only_and_records_real_instants ... ok
test incident_of_another_tenant_is_404_not_403 ... ok
test acknowledge_without_responder_is_422_missing_human_proof ... ok
test full_happy_path_derives_every_label_from_the_state ... ok
test transition_out_of_order_is_409_with_the_current_state ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.35s
     Running tests/retention_policy_missing.rs (target/debug/deps/retention_policy_missing-…)
running 1 test
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/inbox.rs (target/debug/deps/inbox-…)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/outbox.rs (target/debug/deps/outbox-…)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/position.rs (target/debug/deps/position-…)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests godmesh_core / godmesh_emergency / godmesh_identity / godmesh_policy /
   godmesh_protocol / godmesh_server / godmesh_sync
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Nenhuma falha. `retention_policy_missing` continua `ignored` por desenho (altera o
registro de política de CO e roda isolado com `--ignored`); a fatia não mudou isso.

## 6. `cargo clippy --workspace --all-targets` (literal)

```console
$ cargo clippy --workspace --all-targets
    Checking sqlx v0.9.1
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.59s
```

Zero avisos (o workspace nega `clippy::all`).

## 7. Cobertura dos testes de integração (`tests/lifecycle.rs`)

| Teste | Prova |
|---|---|
| `full_happy_path_derives_every_label_from_the_state` | 4 transições em sequência; `state`/`ack_class`/`ui_label` derivados do estado; trilha na ordem `persisted_local → replicated_to_authority → materialized → acknowledged_human`; `source` humano = respondente; todos `at_unix > 0` |
| `transition_out_of_order_is_409_with_the_current_state` | pular para `acknowledge` a partir de `persisted_local` → `409 illegal_transition` com `state=persisted_local`; estado e trilha intactos |
| `acknowledge_without_responder_is_422_missing_human_proof` | corpo `{}`, `responder` só espaços e corpo ausente → `422 missing_human_proof`; estado continua `acknowledged_technical`, nenhum ACK humano |
| `incident_of_another_tenant_is_404_not_403` | GET e POST de outro tenant → `404`; inexistente → `404` idêntico; dono intacto |
| `concurrent_same_transition_yields_exactly_one_success_and_one_409` | duas chamadas simultâneas a `replicate` → `{200, 409}`; exatamente uma linha `replicated_to_authority` e uma `persisted_local` na trilha |
| `ack_trail_is_append_only_and_records_real_instants` | ordem e `at_unix > 0`; `UPDATE` e `DELETE` em `sos_ack` recusados pelo trigger do banco |

## 8. Limitações e bloqueios

- **Sem verificação criptográfica de credencial.** `Bearer` continua resolvido
  contra o registro `GODMESH_GRANTS` em memória, como nas fatias
  anteriores; emissão/revogação seguem pendentes de ADR de identidade.
- **`replicated_to_authority` é um rótulo de fronteira, não prova criptográfica
  de réplica da autoridade.** O emissor registrado é `authority:godmesh-server`,
  o servidor que executou a transição; a prova de réplica real (D8) segue OPEN na
  documentação.
- **Ordem da trilha** usa `ORDER BY recorded_at, at_unix, ack_class`. `recorded_at`
  é `now()` (início da transação), portanto transições concorrentes distintas têm
  instantes distintos; o desempate por `at_unix`/`ack_class` só atua em empate.
  Não há coluna de sequência própria em `sos_ack` — mudá-la exigiria migração e
  ficou fora do escopo desta fatia.
- **Sem paginação de ACKs.** Incidentes desta máquina de estados têm no máximo 4
  ACKs; acima disso o contrato precisará de cursor.
- **Nenhuma rota de `outcome_unknown`/`not_persisted`.** A máquina de estados os
  prevê, mas não são transições deste contrato de fio.
- **Testes dependem de Postgres local.** Sem `DATABASE_URL`, o binário de
  integração é ignorado; nenhum resultado acima é simulado.