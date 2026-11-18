# Evidência — servidor HTTP, fatia mínima do Programa A (18/11/2026)

**Escopo:** fechar o ciclo do crate `godmesh-server` — binário, build, clippy, testes de
contrato contra Postgres real e smoke test por HTTP. **Não** inclui autenticação,
TLS, autorização multi-tenant, deploy nem aprovação de SPEC.

**Origem do código:** as camadas `src/api.rs`, `src/http.rs`, `src/lib.rs` e
`tests/contract.rs` já existiam como trabalho não commitado fora deste worktree e foram
incorporadas a este branch. O `src/main.rs`, os ajustes de feature `serde` em
`godmesh-identity` e as correções de borda descritas abaixo são desta fatia.

**Ambiente:** macOS (arm64), `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1`,
Postgres 18-alpine em contêiner OrbStack (`localhost:55432`), migração
`migrations/0001_core.sql` aplicada.

## Versões usadas — conferidas no registry

Fonte: `curl https://crates.io/api/v1/crates/<crate>` (campo `max_stable_version`),
consulta em **2026-11-18T13:04Z**. Versões resolvidas lidas do `Cargo.lock`.

| Crate | Última estável (crates.io) | Resolvida no lock |
|---|---|---|
| axum | 0.8.9 | 0.8.9 |
| sqlx | 0.9.1 | 0.9.1 (`sqlx-core`, `sqlx-postgres` idem) |
| tokio | 1.54.1 | 1.54.1 |
| serde | 1.0.229 | 1.0.229 |
| serde_json | 1.0.154 | 1.0.154 |
| tower | 0.5.3 | 0.5.3 (transitiva) |
| hyper | — | 1.12.0 (transitiva) |

Documentação oficial consultada para a API usada: `axum::serve` + `Router::with_state`
e `PgPoolOptions::connect` (docs.rs das versões acima). **Não verificado** nesta fatia:
comportamento de `sqlx` 0.9 sob TLS/rustls-ring em produção — a feature está declarada,
mas nenhuma conexão TLS foi exercitada.

## Comandos e saídas literais

### 1. Build e lint

```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s

$ cargo clippy --workspace --all-targets
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.41s
```

0 erros, 0 avisos nas duas execuções.

### 2. Testes de contrato com banco real (nada pulado)

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  cargo test -p godmesh-server --test contract -- --nocapture

running 3 tests
test denies_incident_without_trusted_freshness_source ... ok
test refuses_precise_location_offline ... ok
test persists_incident_and_never_claims_more_than_persisted_local ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
```

`--nocapture` foi usado de propósito: o teste imprime `SKIP: DATABASE_URL ausente` quando
não conecta, e nenhuma linha `SKIP` apareceu. Confirmação independente no banco:

```
$ docker exec godmesh-pg psql -U godmesh -d godmesh \
    -c "select i.id, i.state, a.ack_class, a.source from sos_incident i
        join sos_ack a on a.incident_id = i.id order by i.created_at desc limit 5;"

    id     |      state      |    ack_class    |     source
-----------+-----------------+-----------------+----------------
 inc-33627 | persisted_local | persisted_local | inbox:tenant-1
 inc-32202 | persisted_local | persisted_local | inbox:tenant-1
 inc-32048 | persisted_local | persisted_local | inbox:tenant-1
 inc-1     | persisted_local | persisted_local | inbox:1
 inc-1     | persisted_local | persisted_local | inbox:1
```

### 3. Correção necessária antes do verde

O caso de uso montava o incidente com `Incident::triggered` e devolvia
`incident.ack_class()` — ou seja, o rótulo do estado *anterior* à persistência
(`transport_accepted`), enquanto o corpo já dizia `state: persisted_local`. O teste de
contrato pegou a contradição:

```
thread 'persists_incident_and_never_claims_more_than_persisted_local' panicked at
crates/godmesh-server/tests/contract.rs:45:5:
assertion `left == right` failed
  left: "transport_accepted"
 right: "persisted_local"
```

Correção no servidor (nada no domínio): chamar `Incident::persist`, que é a transição que
autoriza `persisted_local` (ADR-058), e derivar daí `state` e a linha de `sos_ack`. O nome
persistido do estado virou uma função local `state_name` que espelha o `CHECK` de
`sos_incident.state` — adaptador de armazenamento, não regra de domínio. O teste **não**
foi enfraquecido: só ganhou o campo obrigatório `retention_class_known: true` que faltava
para compilar.

### 4. Smoke test por HTTP

Binário em `BIND_ADDR=127.0.0.1:18080` (a porta 8080 estava ocupada por outro processo do
host; o default do binário continua `127.0.0.1:8080`).

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  BIND_ADDR=127.0.0.1:18080 ./target/debug/godmesh-server
godmesh-server: escutando em http://127.0.0.1:18080
```

**Grant válido** (`open_sos_incident` + `f3_verified_online`):

```
HTTP 201
{"incident_id":"smoke-201","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
```

**`freshness: none`:**

```
HTTP 422
{"ack_class":"transport_accepted","detail":"DenyDeniedNoTrustedTime","error":"grant_denied"}
```

**`operation: share_precise_location`:**

```
HTTP 422
{"ack_class":"transport_accepted","detail":"DenyNeverOffline","error":"grant_denied"}
```

**`GET /healthz`:**

```
{"ack_class":"transport_accepted","status":"ok"} HTTP 200
```

Persistência do smoke conferida no banco:

```
 incident_id |    ack_class    |       source
-------------+-----------------+--------------------
 smoke-201   | persisted_local | inbox:tenant-smoke

    id      |      state
------------+-----------------
 smoke-201  | persisted_local
```

### 5. Retry com o mesmo `incident_id` (comportamento atual, não corrigido)

```
$ curl -s -w ' HTTP %{http_code}\n' -XPOST …/v1/incidents -d '{… "incident_id":"smoke-dup" …}'   # 1ª
{"incident_id":"smoke-dup","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"} HTTP 201
$ (mesmo corpo)                                                                                     # 2ª
{"ack_class":"transport_accepted","detail":"falha de persistência","error":"storage_unavailable"} HTTP 503
```

### 6. Configuração ausente e falhas de infraestrutura

```
$ env -u DATABASE_URL ./target/debug/godmesh-server
godmesh-server: configuração inválida: DATABASE_URL não definida; exemplo: DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh
exit=78

$ DATABASE_URL= ./target/debug/godmesh-server
godmesh-server: configuração inválida: DATABASE_URL está definida mas vazia; informe a URL do Postgres
exit=78

$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55999/godmesh ./target/debug/godmesh-server
godmesh-server: não foi possível conectar ao Postgres em DATABASE_URL: pool timed out while waiting for an open connection
exit=1

$ DATABASE_URL=… BIND_ADDR=127.0.0.1:18080 ./target/debug/godmesh-server
godmesh-server: não foi possível escutar em BIND_ADDR=127.0.0.1:18080: Address already in use (os error 48)
exit=1
```

Encerramento gracioso por `SIGTERM`:

```
godmesh-server: escutando em http://127.0.0.1:18080
godmesh-server: encerrado
```

## O que isso prova

- O workspace compila e passa `clippy --all-targets` sem avisos com o crate novo.
- O endpoint `POST /v1/incidents` persiste de verdade no Postgres: linha em
  `sos_incident` e trilha append-only em `sos_ack`.
- O caminho positivo só devolve `persisted_local`, e o rótulo vem da transição de
  domínio, não de constante local.
- `freshness: none` e `share_precise_location` são negados com 422 `grant_denied`, com o
  motivo explícito no `detail`.
- O binário falha com mensagem legível e código de saída != 0 quando configuração falta,
  banco não responde ou porta está ocupada; encerra por `SIGTERM`.

## O que isso NÃO prova

- **Não prova autorização nem multi-tenant real:** `tenant_id` e `principal` vêm do corpo
  sem validação de identidade. Qualquer cliente pode declarar qualquer tenant.
- **Não prova TLS, rate limit, idempotência de retry nem concorrência:** `POST` repetido com
  o mesmo `incident_id` retorna 503 `storage_unavailable`, não `200`/`409` idempotente
  (saída literal na seção 5). Esse comportamento não foi corrigido nesta fatia.
- **Não prova semântica de ACK ponta a ponta:** a trilha grava `at_unix = 0` porque o
  domínio ainda não recebe relógio; o instante real está em `recorded_at` do banco.
- **Não prova deploy:** `deploy/prod`, migrations automáticas, healthcheck de orquestrador
  e PostGIS continuam fora.
- **Não prova a suíte do workspace inteira verde:** ver pendência abaixo.
- **Não é aprovação de SPEC** — é evidência de implementação de uma fatia.

## Pendências e bloqueios

1. **`godmesh-sync` com 2 testes falhando, anterior a esta fatia.** `cargo test -p
   godmesh-sync --test outbox` falha de forma determinística (também com
   `--test-threads=1`) em `append_keeps_declaration_order` e
   `replay_never_reemits_irreversible_effect`, inclusive fora deste branch. Isso
   contradiz a evidência do ciclo 5, que registrava 62/62. Não foi tocado aqui porque
   está fora do escopo desta fatia; fica registrado como regressão aberta.
2. `BIND_ADDR` default é loopback; expor a interface exige configuração explícita e ainda
   não há TLS.
3. Porta 8080 do host ocupada por processo de terceiros durante o smoke; o teste usou
   18080 e o log está acima.