# Evidência — integração do sync/inbox no main e numeração de migrações (18/11/2026)

**Branch:** `room/sy-integra-sync-e-migracoes-rct9` (rebaseado sobre `main` = `79f8ea5`)
**Ambiente:** macOS 27.0.1 arm64; `rustc 1.99.1 (58cf8f9c3 2026-10-13)`,
`cargo 1.99.1 (5f94df478 2026-08-27)`; `psql (PostgreSQL) 18.6`;
servidor `PostgreSQL 18.7 on aarch64-unknown-linux-musl` em
`postgres://godmesh:godmesh_dev_only@localhost:55432`. Desenvolvimento local-first,
**sem contêiner**.

**Escopo:** integrar o commit `dcd6142` (`room/se-sync-inbox-no-servidor-v7t3`) na base atual
sem perder nenhuma das fatias concorrentes; renumerar a migração de sync para `0005`; fazer os
testes de integração aplicarem o esquema pelo **runner oficial**; recompor `run_all.sh`.

## 1. Ordem final das migrações

Um arquivo por número, sem colisão. O runner aplica em ordem lexicográfica do nome.

| # | Arquivo | Origem | Objeto |
|---|---------|--------|--------|
| 0001 | `migrations/0001_core.sql` | base | `event_position`, `sos_incident`, `sos_ack`, retenção semeada |
| 0002 | `migrations/0002_incident_tenancy.sql` | base | tenancy do incidente, jurisdição, PK `(tenant_id, id)` |
| 0003 | `migrations/0003_outbox_publisher.sql` | SD | `outbox_entry.attempts`, `outbox_entry_pendente`, `outbox_sink` |
| 0004 | `migrations/0004_grant_revocation.sql` | SF | `grant_revocation(token_id, revoked_at, reason)` |
| 0005 | `migrations/0005_sync_inbox.sql` | **SE, renumerada** | `sync_inbox_state(tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix, updated_at)` |

Invariantes verificados na mesma ordem:

| Arquivo | Blocos `OK` |
|---------|-------------|
| `migrations/tests/0001_invariants.sql` | 6 |
| `migrations/tests/0002_invariants.sql` | 8 |
| `migrations/tests/0003_invariants.sql` | 6 (outbox) |
| `migrations/tests/0004_invariants.sql` | 4 (revogação) |
| `migrations/tests/0005_invariants.sql` | 6 (Inbox) |
| **total** | **30** |

Antes desta integração existiam duas `0003` (outbox do SD e sync do SE) e três versões
concorrentes de `migrations/tests/run_all.sh` / `0003_invariants.sql`. Depois: cinco migrações,
cinco arquivos de invariantes, um `run_all.sh`.

## 2. O que mudou

| Arquivo | Mudança |
|---------|---------|
| `crates/godmesh-server/src/http.rs` | **conflito resolvido**: mantidos os grupos de rota com escopo (SF) e `healthz` com pool; as rotas de sync entram como dois grupos novos — `sync_write` (`POST /v1/sync/inbox`, escopo `sync:write`) e `sync_read` (`GET /v1/sync/state`, escopo `sync:read`). Todas sob o mesmo `require_grant`. |
| `crates/godmesh-server/src/inbox.rs` | novo (SE): `receive_batch`, `read_state`, `InboxGap` |
| `crates/godmesh-server/src/api.rs` | novo `ApiError::InboxGap { expected, received, accepted, duplicates, contiguous_prefix }`; `extra()` passa a devolver `Vec` (estado da transição + campos da lacuna) |
| `crates/godmesh-server/src/events.rs` | `resolve_jurisdiction`, `resolve_retention_class`, `contiguous_prefix` viram `pub(crate)` para o Inbox reusar a mesma resolução de retenção (nada duplicado) |
| `migrations/0005_sync_inbox.sql` | renumerada de `0003_sync_inbox.sql`; **sem `BEGIN`/`COMMIT`** (o runner é dono da transação, igual a 0003/0004) |
| `migrations/tests/0005_invariants.sql` | renumerada de `0003_invariants.sql`; mesma lista de 6 `OK`, cabeçalho aponta para a nova ordem |
| `migrations/tests/run_all.sh` | reconstruído: aplica 0001, 0002, 0003_outbox, 0004_grant_revocation, 0005_sync_inbox; verifica os **cinco** arquivos de invariantes com listas explícitas de `OK`; comentário duplicado de `--help` removido |
| `crates/godmesh-server/tests/common/mod.rs` | **novo**: suporte de teste que migra o banco pelo runner oficial |
| `tests/{authorization,contract,events,inbox,lifecycle,retention_policy_missing}.rs` | usam `tests/common/mod.rs`; nenhum assume esquema pronto |
| `deploy/README.md` | ordem das migrações e `RESULTADO` esperado (30 invariantes) |
| `docs/09-equipe/evidencias/2026-11-18-servidor-sync-inbox.md` | referências renumeradas para `0005` |

### O teste que falhava por esquema ausente

`crates/godmesh-server/tests/authorization.rs::revocation_in_database_is_401_without_restart`
conectava num banco e assumia `grant_revocation` presente — falhava com esquema ausente em vez
de exercitar revogação. Correção: o setup aplica `godmesh_server::migrate::migrate` (o mesmo
runner de `--migrate-only` e da inicialização) no diretório `migrations/` do repositório,
resolvido por `CARGO_MANIFEST_DIR` — não pelo `cwd` do `cargo test`. Migra só uma vez por URL,
sob o advisory lock do próprio runner. O teste continua igual e continua provando o mesmo:
revogação em banco, sem reinício.

Sem URL de banco o teste é pulado; **com** URL e banco inacessível ele falha (antes pulava em
silêncio — banco configurado e inalcançável não pode parecer verde).

Prova numa base vazia (0 tabelas antes):

```
$ psql ... -d postgres -c "CREATE DATABASE godmesh_sy_vazia"
banco vazio criado: 0 tabelas
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sy_vazia \
    cargo test -p godmesh-server --test authorization -- --nocapture
test valid_grant_with_scope_is_201 ... ok
test revocation_in_database_is_401_without_restart ... ok
migrações (postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sy_vazia): aplicadas \
["0001_core.sql", "0002_incident_tenancy.sql", "0003_outbox_publisher.sql", \
"0004_grant_revocation.sql", "0005_sync_inbox.sql"], já presentes 0

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

## 3. Aplicação no banco de dev pelo próprio binário

```
$ DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh' \
    cargo run -q -p godmesh-server -- --migrate-only
godmesh-server: migrações em migrations — 1 aplicada(s) [0005_sync_inbox.sql], \
4 já aplicada(s) [0001_core.sql, 0002_incident_tenancy.sql, 0003_outbox_publisher.sql, \
0004_grant_revocation.sql]
EXIT=0
```

Nenhum `DROP`/`TRUNCATE` no banco compartilhado; nenhuma migração editada depois de aplicada
(o runner teria falhado por divergência de checksum).

## 4. Verificações executadas

### `cargo test --workspace` (0 falhas)

Saídas literais dos binários deste workspace:

```
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.29s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.18s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Total: **194 aprovados, 0 falhas, 1 ignorado** (não há binário com teste falhando). O ignorado
é `retention_policy_missing`, que altera o registro de política de CO e só roda com `--ignored`
por contrato — inalterado nesta fatia.

### `migrations/tests/run_all.sh`

```
$ PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
    migrations/tests/run_all.sh
== banco descartável: godmesh_inv_20261118111052_88255
== aplicando migrations/0001_core.sql
== aplicando migrations/0002_incident_tenancy.sql
== aplicando migrations/0003_outbox_publisher.sql
== aplicando migrations/0004_grant_revocation.sql
== aplicando migrations/0005_sync_inbox.sql
…
psql:…/0005_invariants.sql:28: NOTICE:  OK 1: namespace de Inbox duplicado recusado (PK)
psql:…/0005_invariants.sql:39: NOTICE:  OK 2: contiguous_prefix negativo recusado
psql:…/0005_invariants.sql:50: NOTICE:  OK 3: partição negativa recusada
psql:…/0005_invariants.sql:60: NOTICE:  OK 4: authority_epoch zero recusado
psql:…/0005_invariants.sql:78: NOTICE:  OK 5: partição, epoch e origem são dimensões do namespace (4)
psql:…/0005_invariants.sql:100: NOTICE:  OK 6: prefixo persiste a avançar no mesmo namespace (3 -> 4)

RESULTADO: OK — 30 invariantes verificados em banco criado do zero.
```

### `cargo clippy --workspace --all-targets`

```
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.64s
```

Sem avisos (`clippy::all = deny` no workspace).

## 5. Limitações e o que não foi executado

- `cargo fmt --all --check` **não** está limpo no repositório, e já não estava antes desta
  fatia (`godmesh-core` diverge). Não rodei `cargo fmt` para não misturar reformatação alheia
  com esta integração; o portão aplicado é `clippy`, que passa.
- `retention_policy_missing` (o único `--ignored`) não foi executado nesta rodada.
- Não há teste de carga/corrida entre dois processos migrando o mesmo banco além do advisory
  lock já coberto por `runtime.rs`.
- O Inbox herda a resolução de retenção/jurisdição de `events.rs`; nenhuma política nova foi
  introduzida aqui.