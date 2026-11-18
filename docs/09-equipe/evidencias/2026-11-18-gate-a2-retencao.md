# Evidência — gate A2: retenção no banco e idempotência (18/11/2026)

**Origem:** achados do gate `wfr_414e32e9` sobre `crates/godmesh-server/src/api.rs:86-94`,
`migrations/0001_core.sql:45-56` e o caminho de escrita do incidente SOS.
**Escopo:** migração `0002_incident_tenancy.sql`, resolução da política dentro da
transação de escrita, idempotência por tenant e testes de contrato.
**Fora de escopo:** autenticação/autorização multi-tenant, `deploy/`,
`crates/godmesh-sync`, `crates/godmesh-emergency`, SPEC nova, aprovação de produto.

**Ambiente:** macOS arm64; `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1`;
Postgres **18.6** (`psql` do libpq) contra o contêiner `postgres:18-alpine`
(PostgreSQL 18.0) em `localhost:55432`, banco `godmesh`, migrações 0001 e 0002
aplicadas; `sqlx` 0.9.1 e `axum` 0.8.9 resolvidos do `Cargo.lock` (ver
`2026-11-17-server-fatie-minima.md`; nenhuma dependência nova nesta fatia).

## 1. Achados e o que cada um virou

| Achado do gate | Tratamento |
|---|---|
| `api.rs` checava retenção com literal `'BR'` e ignorava `retention_copy` | Jurisdição vem de `tenant_region` (ADR-066); a consulta exige `retention_class` **e** `retention_copy.copy_scope='primary'`; nenhum literal de jurisdição no código |
| `sos_incident` não registrava qual política autorizou | Colunas `retention_class`, `retention_jurisdiction` e `retention_policy_id` (`incidente_sos@BR`), `NOT NULL` e `NOT NULL` na FK |
| `sos_incident` sem FK para `retention_class`; INSERT direto aceito | `FOREIGN KEY (retention_class, retention_jurisdiction) REFERENCES retention_class (class, jurisdiction)` + `NOT NULL` |
| Checagem fora da transação de escrita (TOCTOU) | `BEGIN` → `SELECT … FOR SHARE` da política → `INSERT` → `COMMIT` na mesma transação; a FK imediata fecha a janela |
| PK global de `incident_id` permitia squatting entre tenants | PK passa a `(tenant_id, id)`; `id` continua único no banco e a colisão de outro tenant vira `409 incident_id_conflict` |
| Retry do mesmo pedido devolvia 503 | `INSERT … ON CONFLICT (tenant_id, id) DO NOTHING` + leitura do estado persistido: replay → `200` com o `ack_class` original |

## 2. Arquivos alterados

```
migrations/0002_incident_tenancy.sql          (novo)
migrations/tests/0002_invariants.sql           (novo)
migrations/tests/0001_invariants.sql           (ajustado: inserts de incidente passam a declarar a política)
crates/godmesh-server/src/api.rs               (política por tenant, transação única, idempotência, conflito)
crates/godmesh-server/src/http.rs              (201 cria / 200 replay; 409 policy_missing, scope_unresolved, id_conflict)
crates/godmesh-server/tests/contract.rs        (7 testes de contrato; replay, squatting, CO, sem jurisdição)
crates/godmesh-server/tests/retention_policy_missing.rs (novo: CO sem cópia autorizada → 409)
docs/09-equipe/evidencias/2026-11-18-gate-a2-retencao.md (este arquivo)
```

Nada em `deploy/`, `godmesh-sync`, `godmesh-emergency`, `godmesh-core`,
`godmesh-policy`, `godmesh-protocol` ou `godmesh-identity` foi tocado.

## 3. Decisões de esquema e o motivo

1. **`tenant_region` é nova e necessária.** "Jurisdição do tenant" não existia em
   lugar nenhum do banco nem do código; a única fonte era o literal `'BR'` no
   `api.rs`. A ADR-066 declara residência de dados **por tenant**, e a SPEC-005 §3
   diz que campo enviado pelo cliente é seleção, não prova de pertencimento.
   Por isso a jurisdição é lida do banco, nunca do corpo do pedido. Tenant sem
   linha em `tenant_region` falha fechado (`409 retention_scope_unresolved`).
2. **`retention_policy_id` é coluna gerada** (`retention_class || '@' ||
   retention_jurisdiction`, `STORED`). A política neste esquema é identificada
   pela chave composta `retention_class(class, jurisdiction)`; `retention_class`
   não tem coluna `policy_id` própria, e uma coluna única não pode referenciar
   chave de duas colunas. A integridade é da FK composta; a coluna existe para
   que o incidente diga, em uma string, **qual** política o autorizou.
3. **PK `(tenant_id, id)` + `UNIQUE (id)`.** São as duas coisas ao mesmo tempo, e
   isso é deliberado: a PK composta torna a tenancy explícita no agregado; o
   `UNIQUE (id)` mantém `sos_ack(incident_id)` referenciando `sos_incident(id)`
   sem precisar de `tenant_id` na trilha, e é o que transforma colisão entre
   tenants em `409` detectado em vez de um segundo incidente silencioso com o
   mesmo identificador. Sem o `UNIQUE`, `ON CONFLICT (tenant_id, id)` deixaria os
   dois tenants gravarem o mesmo `id` e o caso de squatting não seria observável.
4. **Semente CO.** `incidente_sos` foi semeado para `CO` com a mesma finalidade,
   prazo e gatilho da ADR-065 (dever de cuidado, 12 meses, `incident_closed +
   12m`) e cópia primária autorizada. As demais classes continuam sem política
   em CO e seguem falhando fechado — a ADR-066 só obriga declarar o que foi
   declarado; nenhum prazo novo foi inventado.
5. **Backfill.** Os 12 incidentes que já existiam no banco de dev receberam
   `incidente_sos`/`BR` antes do `NOT NULL`. Nenhum registro foi apagado.

## 4. Migração aplicada (Postgres 18.6, banco de dev)

```
$ psql -v ON_ERROR_STOP=1 -f migrations/0002_incident_tenancy.sql "$DATABASE_URL"
BEGIN
CREATE TABLE
COMMENT
INSERT 0 1
INSERT 0 1
ALTER TABLE
ALTER TABLE
ALTER TABLE
UPDATE 12
ALTER TABLE
ALTER TABLE
ALTER TABLE
ALTER TABLE
ALTER TABLE
ALTER TABLE
ALTER TABLE
ALTER TABLE
COMMIT
```

## 5. Invariantes provados **no banco**

```
$ psql -v ON_ERROR_STOP=1 -f migrations/tests/0002_invariants.sql "$DATABASE_URL"
NOTICE:  OK 1: incidente sem classe de retenção recusado
NOTICE:  OK 2a: jurisdição fora de BR/CO recusada
NOTICE:  OK 2b: incidente sem política recusado (FK)
NOTICE:  OK 3: incidente_sos semeado para CO com cópia primária
NOTICE:  OK 4: incidente registra a política aplicada (incidente_sos@BR)
NOTICE:  OK 5: colisão de id entre tenants recusada
NOTICE:  OK 6: PK de sos_incident é (tenant_id, id)
NOTICE:  OK 7: política em uso não pode ser removida (FK)
 resultado
-----------------------
 verificação concluída
```

O arquivo anterior continua passando:

```
$ psql -v ON_ERROR_STOP=1 -f migrations/tests/0001_invariants.sql "$DATABASE_URL"
NOTICE:  OK 3: posição duplicada recusada
NOTICE:  OK 4: ACK append-only (reescrita bloqueada)
NOTICE:  OK 5: estado inventado recusado
 resultado
-----------------------
 verificação concluída
```

Nenhum desses dois arquivos é auto-suficiente em reexecução: o bloco 2 do 0001 já
falhava na segunda execução antes desta fatia (insere a mesma posição canônica).
O 0001 foi ajustado apenas para declarar `retention_class`/`retention_jurisdiction`
nos dois `INSERT` de incidente, que a migração 0002 passou a exigir, e para gerar
um `id` novo no teste de `state` — sem isso o bloco 5 provava `NOT NULL` em vez do
`CHECK` de estado.

## 6. TOCTOU: o lock da política segura a remoção

Sessão A replica o que a API faz (classe + cópia primária com `FOR SHARE`, dentro
da transação) e segura por 3 s; sessão B tenta apagar a cópia autorizada:

```
$ # sessão A: BEGIN; SELECT … FOR SHARE OF rc, cp; SELECT pg_sleep(3); COMMIT;
$ # sessão B: DELETE FROM retention_copy WHERE class='a2_probe2' …
DELETE da cópia autorizada esperou 2496 ms pelo lock compartilhado da leitura
```

O `DELETE` não interleava com a leitura: esperou o lock e só então executou.
A segunda camada é a FK: no `INSERT`, o Postgres toma `KEY SHARE` na linha da
política e recusa apagar política referenciada (bloco OK 7 acima), então remover
a política entre checagem e escrita não deixa incidente órfão.

Não foi executado teste de carga concorrente nem corrida com `pgbench`; a
evidência é a espera medida acima mais a FK.

## 7. Testes de contrato (Postgres real, nada pulado)

```
$ cargo test -p godmesh-server --test contract -- --nocapture
running 7 tests
test tenant_without_declared_jurisdiction_fails_closed ... ok
test refuses_precise_location_offline ... ok
test denies_incident_without_trusted_freshness_source ... ok
test persists_incident_and_never_claims_more_than_persisted_local ... ok
test colombian_tenant_uses_colombian_policy ... ok
test identical_retry_is_idempotent_and_returns_the_original_ack ... ok
test incident_id_of_another_tenant_is_a_conflict_not_a_second_incident ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.16s
```

O caso de "CO sem política" altera o registro de política, então roda isolado:

```
$ cargo test -p godmesh-server --test retention_policy_missing -- --ignored --nocapture
running 1 test
test colombian_tenant_without_policy_is_refused_with_retention_policy_missing ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

Ele remove **só a cópia primária autorizada** de `incidente_sos`/`CO`, chama a
API, espera `409 retention_policy_missing` e repõe a semente, fechando com um
controle positivo (aceito de novo). A linha de `retention_class` não é apagada
porque incidentes de CO já gravados a referenciam — tentar apagá-la foi o que a
primeira versão fez, e a FK recusou com `23503`: o próprio achado do gate
funcionando ao contrário. `#[ignore]` existe porque o teste muda o registro de
política: rodá-lo junto da suíte paralela faria os outros testes de CO verem uma
política que não está lá. Os testes de contrato ainda garantem a própria
precondição (`ON CONFLICT DO NOTHING` da semente), então uma execução
interrompida no meio não deixa o banco inutilizável.

## 8. Suíte completa e lint

```
$ cargo test --workspace
test result: ok. 7 passed (godmesh-core)
test result: ok. 10 passed (godmesh-emergency)
test result: ok. 8 passed (godmesh-identity)
test result: ok. 6 passed (godmesh-policy high_water)
test result: ok. 6 passed (godmesh-policy retention)
test result: ok. 4 passed (godmesh-protocol ack_class)
test result: ok. 5 passed (godmesh-protocol ack_composition)
test result: ok. 5 passed (godmesh-protocol cursor)
test result: ok. 6 passed (godmesh-protocol version)
test result: ok. 1 passed (godmesh-server unit: replay 200 / criação 201)
test result: ok. 7 passed (godmesh-server contract)
test result: ok. 0 passed; 1 ignored (godmesh-server retention_policy_missing)
test result: ok. 5 passed (godmesh-sync inbox)
test result: ok. 7 passed (godmesh-sync outbox)
test result: ok. 4 passed (godmesh-sync position)
```

0 falhas, 0 pulados por falta de `DATABASE_URL` (a variável foi passada).

```
$ cargo clippy --workspace --all-targets
    Checking godmesh-server v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.70s
```

0 erros, 0 avisos (`clippy::all` é `deny` no workspace).

`cargo fmt --all --check` **não passa** neste repositório: há divergência de
formatação pré-existente em `crates/godmesh-core/src/lib.rs` e outros arquivos
que esta fatia não toca. Não rodei `cargo fmt` para não reformatar código de
outros donos.

## 9. Smoke por HTTP (processo real, banco real)

`DATABASE_URL=… BIND_ADDR=127.0.0.1:18099 ./target/debug/godmesh-server`, com
`tenant-smoke` declarado em `BR`:

```
--- 1o POST (cria) ---
HTTP 201
{"incident_id":"inc-smoke-a2-1","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
--- 2o POST identico (replay) ---
HTTP 200
{"incident_id":"inc-smoke-a2-1","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
--- 3o POST mesmo id, outro tenant com jurisdicao declarada (squatting) ---
HTTP 409
{"ack_class":"transport_accepted","detail":"incident_id já pertence a outro tenant","error":"incident_id_conflict"}
--- 4o POST tenant sem jurisdicao declarada ---
HTTP 409
{"ack_class":"transport_accepted","detail":"tenant sem jurisdição declarada","error":"retention_scope_unresolved"}
```

Estado no banco depois dos quatro pedidos:

```
       id       |   tenant_id   |      state      | retention_class | retention_jurisdiction | retention_policy_id
----------------+---------------+-----------------+-----------------+------------------------+---------------------
 inc-smoke-a2-1 | tenant-smoke  | persisted_local | incidente_sos   | BR                     | incidente_sos@BR
 acks
------
    1
```

Um incidente, um ACK: o replay não duplicou nada e o vizinho não criou um segundo
registro com o mesmo `id`.

## 10. O que isto **não** prova

- **Não há autenticação.** "Tenant autenticado" aqui é o `tenant_id` do corpo do
  pedido; `tenant_region` só decide a jurisdição, não prova pertencimento. Trocar
  o `tenant_id` do corpo troca o tenant. Autenticação e autorização continuam
  pendentes e são pré-requisito para o endpoint sair de dev.
- **Não é deploy nem migração de produção.** A migração roda em um passo único
  (`BEGIN … COMMIT`, sem `IF NOT EXISTS`): não é idempotente por desenho, ao
  contrário do resto do repositório que não tem ferramenta de migração. Não houve
  ensaio em cópia de produção, nem medição de tempo de lock em tabela grande.
- **Só `incidente_sos` foi resolvido por jurisdição.** As outras classes e o
  caminho de eventos (`event_position`) continuam com a política resolvida à mão.
- **CO só tem `incidente_sos`.** As demais classes em CO seguem sem política e
  devem falhar fechado em qualquer caminho que as use.
- **`sos_ack` continua sem `tenant_id`.** A trilha herda o tenant do incidente por
  `incident_id`; uma futura PK composta em `sos_ack` exigiria migração própria.
- **Nada foi validado juridicamente** (ADR-065/066 pendentes de revisão
  profissional). Prazos e jurisdições continuam declaração de produto.