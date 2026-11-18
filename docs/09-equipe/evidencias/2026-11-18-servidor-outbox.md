# Evidência — outbox publicador durável (18/11/2026)

**Worker:** SD — outbox publicador durável. **Branch:** `room/sd-outbox-publicador-duravel-yigp`.
**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`, Postgres **18.7** local (`localhost:55432`, sem Docker).
**Bancos usados:** `godmesh_sd` (testes de integração do outbox, criado do zero) e `godmesh` (contrato existente do servidor). Banco descartável `godmesh_inv_*` para os invariantes de esquema.

Escopo entregue: migração `0003`, contrato do outbox persistente, laço de publicação no servidor, testes de integração e esta evidência.

## 1. Arquivos

| Arquivo | Estado |
|---|---|
| `migrations/0003_outbox_publisher.sql` | novo — `attempts`, `CHECK` de confirmação, índice de pendências, `outbox_sink` |
| `migrations/tests/0003_invariants.sql` | novo — 6 invariantes de banco |
| `migrations/tests/run_all.sh` | alterado — aplica 0003 e exige os 6 blocos novos |
| `crates/godmesh-sync/src/pg_outbox.rs` | novo — `PgOutbox`, `PartitionKey`, `PgSink`, `Sink` |
| `crates/godmesh-sync/src/lib.rs` | alterado — módulo e limite declarado |
| `crates/godmesh-sync/Cargo.toml` | alterado — `sqlx` como dependência, `tokio` como dev-dependency |
| `crates/godmesh-server/src/publisher.rs` | novo — configuração, passagem, laço tokio |
| `crates/godmesh-server/src/lib.rs`, `src/main.rs` | alterados — módulo e ligação do laço |
| `crates/godmesh-server/Cargo.toml` | alterado — dependência interna `godmesh-sync`, feature `time` do tokio |
| `crates/godmesh-server/tests/outbox_publicador.rs` | novo — 7 testes de integração |
| `deploy/README.md`, `docs/CHANGELOG.md` | alterados — 20 invariantes, registro do delta |

## 2. Dependências — nenhuma nova

`sqlx 0.9.1` e `tokio 1.54.1` **já estavam** em `Cargo.lock` (usados por `godmesh-server`); `godmesh-sync` passou a declará-las e `godmesh-server` ganhou a dependência interna `godmesh-sync` e a feature `time` do tokio. Nenhuma versão mudou, nenhum crate externo novo, nenhum upgrade. `git diff Cargo.lock` contém apenas a lista de dependências do pacote `godmesh-sync`.

APIs usadas, conferidas na documentação oficial das versões resolvidas:

- `SELECT … FOR UPDATE SKIP LOCKED` e reavaliação de `WHERE` sob `READ COMMITTED` — PostgreSQL 18, [SELECT](https://www.postgresql.org/docs/18/sql-select.html) e [Concurrency Control](https://www.postgresql.org/docs/18/transaction-iso.html);
- `sqlx::raw_sql`, `query`, `query_scalar`, `Query::bind`, `SqlSafeStr` (só string estática) — [docs.rs/sqlx 0.9.1](https://docs.rs/sqlx/0.9.1/sqlx/);
- `tokio::time::interval` e `MissedTickBehavior` — [docs.rs/tokio 1.54.1](https://docs.rs/tokio/1.54.1/tokio/time/fn.interval.html);
- `tokio::sync::watch` para o sinal de término compartilhado entre HTTP e publicador.

## 3. Contrato implementado

| Regra da tarefa | Onde | Como |
|---|---|---|
| append transacional | `PgOutbox::append` | `INSERT` em transação, precedido de `pg_advisory_xact_lock` por grupo e checagem `última + 1` |
| publicar só `published_at` | `PgOutbox::publish_next` | `WITH proximo (… FOR UPDATE SKIP LOCKED) UPDATE … RETURNING`; nada de `confirmed_at` |
| confirmar só depois do efeito | `confirm` + publisher | `confirm` exige `published_at IS NOT NULL` **e** `seq = prefixo + 1`; o publisher só chama depois de `deliver` devolver `Ok` |
| leitura de pendentes com prefixo contíguo | `PgOutbox::pending` | lê `seq > prefixo` e para na primeira sequência ausente (`missing`) |
| (a) `confirm` recusa sem `published_at` | código + `CHECK outbox_entry_confirmed_requires_published` | teste `confirm_sem_publish_falha` + invariante `OK 1`/`OK 4` |
| (b) lacuna não é publicada | `publish_next` só considera `seq = prefixo + 1` | teste `lacuna_interrompe_publicacao` → `BlockedByGap { missing: 2 }` |
| (c) `irreversible` nunca reenviado | `publish_next` só reivindica `published_at IS NULL`; `record_failure` só limpa `published_at` para `event` | teste `efeito_irreversivel_nao_e_reenviado` → `AwaitingReconciliation` 3×, sink com 0 linhas |
| (d) duas instâncias não publicam a mesma linha | `FOR UPDATE SKIP LOCKED` + guarda `o.published_at IS NULL` no `UPDATE` | teste `duas_instancias_nao_duplicam_publicacao` (12 sequências, 2 instâncias, sink com PK) |
| (e) falha do efeito mantém pendente e incrementa `attempts` | `record_failure` | teste `falha_do_sink_mantem_pendente_com_attempts` |
| laço tokio com intervalo e desligamento | `publisher.rs` + `main.rs` | `GODMESH_OUTBOX`, `GODMESH_OUTBOX_INTERVAL_MS` (default **1000 ms**), log de publicados/confirmados/falhas |
| efeito real desta fase = sink de teste | `PgSink` → tabela `outbox_sink` | sem broker, bridge, rádio ou notificação |

Semântica de `published_at` que os testes fixam: reivindicar é `published_at = now()`. Falha de efeito irrigável **limpa** `published_at` (reentrega permitida, at-least-once) e incrementa `attempts`. Falha de efeito irreversível **mantém** `published_at` (resultado externo desconhecido) e o publicador passa a devolver `AwaitingReconciliation`, bloqueando o prefixo do grupo — nada depois é publicado ou confirmado. Confirmar continua exigindo `published_at`, então a linha irreversível pendente pode ser confirmada por decisão explícita de reconciliação, mas não é reenviada.

## 4. `cargo test --workspace` — saída literal

Comando (os dois bancos declarados; sem eles os testes de banco são pulados pela convenção do repositório):

```
$ DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  GODMESH_TEST_DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sd \
  cargo test --workspace
```

| Binário | Resultado |
|---|---|
| `godmesh-core` unittests | `0 passed` |
| `godmesh-core` tests/integration.rs | `7 passed` |
| `godmesh-emergency` unittests | `0 passed` |
| `godmesh-emergency` tests/incident.rs | `19 passed` |
| `godmesh-identity` unittests | `0 passed` |
| `godmesh-identity` tests/grant_predicate.rs | `8 passed` |
| `godmesh-policy` unittests | `0 passed` |
| `godmesh-policy` tests/high_water.rs | `6 passed` |
| `godmesh-policy` tests/retention.rs | `6 passed` |
| `godmesh-protocol` unittests | `0 passed` |
| `godmesh-protocol` tests/ack_class.rs | `4 passed` |
| `godmesh-protocol` tests/ack_composition.rs | `5 passed` |
| `godmesh-protocol` tests/cursor.rs | `5 passed` |
| `godmesh-protocol` tests/version.rs | `6 passed` |
| `godmesh-server` unittests (lib) | `6 passed` |
| `godmesh-server` unittests (bin) | `6 passed` |
| `godmesh-server` tests/contract.rs | `17 passed` |
| `godmesh-server` tests/outbox_publicador.rs | `7 passed` |
| `godmesh-server` tests/retention_policy_missing.rs | `0 passed; 1 ignored` |
| `godmesh-sync` unittests | `0 passed` |
| `godmesh-sync` tests/inbox.rs | `5 passed` |
| `godmesh-sync` tests/outbox.rs | `13 passed` |
| `godmesh-sync` tests/position.rs | `4 passed` |
| 7× Doc-tests | `0 passed` |

**Total: 124 aprovados, 0 falhas, 1 ignorado** (o ignorado é `retention_policy_missing`, pré-existente e alheio a esta entrega). Os 30 blocos `test result: ok` confirmam que nenhum binário falhou. Os 6 testes novos de banco de `outbox_publicador` (o sétimo é de configuração/laço) passaram contra `godmesh_sd`.

Binário novo, saída literal:

```
$ GODMESH_TEST_DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sd \
  cargo test -p godmesh-server --test outbox_publicador
running 7 tests
test confirm_sem_publish_falha ... ok
test efeito_irreversivel_nao_e_reenviado ... ok
test lacuna_interrompe_publicacao ... ok
test append_publish_confirm_em_ordem ... ok
test falha_do_sink_mantem_pendente_com_attempts ... ok
test loop_publica_e_encerra_no_sinal ... ok
test duas_instancias_nao_duplicam_publicacao ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

O binário foi executado 6 vezes seguidas com `--test-threads` padrão: `7 passed` em todas (0 flaky). Cada teste usa um grupo `(origin, tenant, stream, partition, epoch)` próprio e limpa o próprio grupo; a passagem do publisher usada nos testes é `run_once_for(&key)` justamente para não publicar grupos de outros testes.

## 5. `cargo clippy --workspace --all-targets` — saída literal

```
$ cargo clippy --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s
```

0 ocorrências de `warning:` e 0 de `error` na saída (`-D clippy::all` herdado do workspace).

## 6. Prova de que o teste de concorrência detecta a falha (sabotagem)

Sem a reivindicação atômica o teste falha. Sabotagem aplicada **temporariamente** em `publish_next` (removidos `AND e.published_at IS NULL`, `FOR UPDATE SKIP LOCKED` e a guarda `AND o.published_at IS NULL`), depois revertida:

```
---- duas_instancias_nao_duplicam_publicacao stdout ----
thread '…' panicked at crates/godmesh-server/tests/outbox_publicador.rs:388:49:
instância 1: error returned from database: duplicate key value violates unique constraint "outbox_sink_pkey" at line 666
test result: FAILED. 0 passed; 1 failed; 0 ignored; ...
```

Com a implementação real (restaurada) o mesmo teste volta a `ok` — a PK de `outbox_sink` é o detector de publicação duplicada, e o teste a exercita com duas instâncias concorrentes sobre 12 sequências.

## 7. Invariantes de banco — saída literal

```
$ PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
  migrations/tests/run_all.sh
== banco descartável: godmesh_inv_<timestamp>_<pid>
== aplicando migrations/0001_core.sql
== aplicando migrations/0002_incident_tenancy.sql
== aplicando migrations/0003_outbox_publisher.sql
…
psql:…/0003_invariants.sql:20: NOTICE:  OK 1: confirmed_at sem published_at recusado (CHECK)
psql:…/0003_invariants.sql:28: NOTICE:  OK 2: publicado sem confirmado aceito
psql:…/0003_invariants.sql:37: NOTICE:  OK 3: publicado e então confirmado aceito
psql:…/0003_invariants.sql:48: NOTICE:  OK 4: apagar published_at de linha confirmada recusado
psql:…/0003_invariants.sql:72: NOTICE:  OK 5: attempts inicia em 0 e recusa negativo
psql:…/0003_invariants.sql:84: NOTICE:  OK 6: entrega duplicada da mesma posição recusada
RESULTADO: OK — 20 invariantes verificados em banco criado do zero.
```

## 8. Binário: laço ligado, desligado e configuração inválida

```
$ GODMESH_GRANTS='{"grants":[]}' GODMESH_ENV=dev BIND_ADDR=127.0.0.1:0 \
  DATABASE_URL=postgres://…/godmesh_sd GODMESH_OUTBOX=off ./target/debug/godmesh-server
godmesh-server: escutando em http://127.0.0.1:63526 (0 grant(s) configurado(s))
godmesh-server: outbox publicador desligado (GODMESH_OUTBOX=off)
godmesh-server: encerrado
exit=0

$ … GODMESH_OUTBOX=on GODMESH_OUTBOX_INTERVAL_MS=200 ./target/debug/godmesh-server
godmesh-server: escutando em http://127.0.0.1:63529 (0 grant(s) configurado(s))
godmesh-server: outbox publicador ativo a cada 200 ms (sink de teste outbox_sink)
godmesh-server: outbox encerrado (publicados 0, confirmados 0, falhas 0)
godmesh-server: encerrado
exit=0

$ … GODMESH_OUTBOX=talvez ./target/debug/godmesh-server
godmesh-server: configuração inválida: configuração do outbox: GODMESH_OUTBOX=talvez não é reconhecido; use on ou off
exit=78
```

SIGTERM encerra HTTP e publicador com o mesmo sinal (um único `shutdown_signal` alimenta um `watch`). Contagem 0/0/0 porque o banco estava sem pendências no momento do teste manual (os testes de integração limpam os próprios grupos) — o caminho de publicação com trabalho real está coberto pelos testes da seção 4.

## 9. O que isso NÃO prova

- **Não há integração externa.** O efeito é a tabela `outbox_sink`. Nenhuma entrega a bridge, rádio, pagamento ou notificação foi exercitada, e nada aqui demonstra entrega ao destino real.
- **Não há payload.** `outbox_entry` guarda sequência e natureza do efeito; replay de projeção (`replay_from` do modelo em memória) continua sendo plano de contagem. Reconstrução de estado depende de SPEC-004/SPEC-021.
- **`pg_advisory_xact_lock` serializa por grupo dentro de um único banco**; não há fencing de líder nem coordenação entre bancos. Dois líderes com bancos distintos não são cobertos.
- **Retry é at-least-once por passagem**, limitado a uma tentativa por pendência por tick. Não há backoff, jitter, limite de tentativas nem dead-letter: a entrada falha fica pendente para sempre até um humano decidir. `attempts` conta, não expira.
- **Crash entre commit e publish** não foi exercitado com `kill -9`; o que existe é a marcação atômica de `published_at` e a confirmação separada.
- **Retenção do outbox não foi aplicada.** `outbox_entry`/`outbox_sink` não têm classe de retenção nem prazo (ADR-062/065) — pendente de decisão, não é regressão desta entrega porque a 0001 também não a tinha.
- `attempts` não guarda a mensagem de erro nem o instante da última falha: só a contagem. Observabilidade além do log de linha de comando depende de SPEC-018.
- O teste `retention_policy_missing` continua `#[ignore]` (pré-existente) e não foi executado nesta rodada.