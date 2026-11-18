# Evidência — teste de outbox migra pelo runner oficial (18/11/2026)

**Branch:** `room/sx-testes-outbox-com-runner-de-migracoes-q87n`
**Base:** `876a68d` (`feat(sync): outbox publicador durável em Postgres`)
**Ambiente:** macOS arm64; contêiner `godmesh-pg` (`postgres:18-alpine`) em
`127.0.0.1:55432`; Rust/sqlx **0.9.1** (`cargo tree -i sqlx` = 0.9.1). Nenhuma
dependência nova. O banco de dev compartilhado (`godmesh`) não foi tocado: os testes
usam banco descartável próprio.

**Escopo:** `crates/godmesh-server/tests/outbox_publicador.rs` e
`migrations/0003_outbox_publisher.sql`. `src/migrate.rs` (o runner) **não** foi
alterado. Nenhuma asserção de comportamento do outbox foi removida ou afrouxada;
nenhum `#[ignore]` foi adicionado.

## 1. Causa

`outbox_publicador.rs` detectava o esquema por `to_regclass('public.outbox_sink') IS
NULL` e, nesse caso, reaplicava **inline** uma lista local `MIGRATIONS`
(`include_str!` de 0001, 0002 e 0003). A lista duplicava o runner e ignorava o estado
real do banco: num banco que já tinha os objetos de 0001/0002 — sem `outbox_sink`,
sem `schema_migration` — o `CREATE TABLE retention_class` de 0001 colidia com a tabela
existente. O erro saía como `42P07 relation "retention_class" already exists` em todos
os 7 testes.

Defeito estrutural: **duas implementações de migração** no repositório (o runner em
`src/migrate.rs` e a lista do teste), com regras diferentes de idempotência. A do teste
era "existe `outbox_sink`?"; a do runner é `schema_migration` (nome + checksum). Só a
segunda é o caminho de produção.

## 2. Antes (reprodução literal)

Banco `godmesh_sx_repro` recriado do zero e migrado **só** com 0001 e 0002 (estado que
a condição `outbox_sink IS NULL` considerava "precisa migrar"):

```
$ psql …/postgres -c 'DROP DATABASE IF EXISTS godmesh_sx_repro WITH (FORCE)' \
                 -c 'CREATE DATABASE godmesh_sx_repro'
$ psql …/godmesh_sx_repro -q -f migrations/0001_core.sql -f migrations/0002_incident_tenancy.sql
$ GODMESH_TEST_DATABASE_URL=…/godmesh_sx_repro cargo test -p godmesh-server --test outbox_publicador
…
---- lacuna_interrompe_publicacao stdout ----

thread 'lacuna_interrompe_publicacao' (128852439) panicked at crates/godmesh-server/tests/outbox_publicador.rs:71:18:
migração aplicada: Database(PgDatabaseError { severity: Error, code: "42P07",
  message: "relation \"retention_class\" already exists", detail: None, hint: None,
  position: None, where: None, schema: None, table: None, column: None, data_type: None,
  constraint: None, file: Some("heap.c"), line: Some(1488),
  routine: Some("heap_create_with_catalog") })

failures:
    append_publish_confirm_em_ordem
    confirm_sem_publish_falha
    duas_instancias_nao_duplicam_publicacao
    efeito_irreversivel_nao_e_reenviado
    falha_do_sink_mantem_pendente_com_attempts
    lacuna_interrompe_publicacao
    loop_publica_e_encerra_no_sinal

test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
```

## 3. Depois (correção)

`outbox_publicador.rs`:

* a lista `MIGRATIONS` e `ensure_schema` (com o `pg_advisory_lock` próprio) **saíram**;
* `pool()` resolve o servidor de `GODMESH_TEST_DATABASE_URL` (ou `DATABASE_URL`),
  **troca só o nome do banco** para `godmesh_sx_<pid>` (configurável por
  `GODMESH_SX_TEST_DB`), cria o banco do zero se não existir e chama
  `godmesh_server::migrate::migrate(&pool, &dir)` — o runner de produção, que aplica em
  ordem, registra nome + checksum em `schema_migration`, é idempotente e usa advisory
  lock próprio;
* o nome do banco entra em `CREATE DATABASE` (DDL não aceita bind), então é **validado**
  em `[A-Za-z0-9_]` antes de ir para o `AssertSqlSafe`;
* falha de conexão/migração agora **estoura** (`expect`) em vez de pular o teste
  silenciosamente: URL presente e banco inacessível não é sucesso;
* novo teste `runner_e_idempotente_no_mesmo_banco`: chama o runner de novo no mesmo
  banco e exige `applied` vazio, `skipped` igual à lista de `*.sql` do diretório e uma
  linha de controle por arquivo.

`migrations/0003_outbox_publisher.sql`: removidos `BEGIN;`/`COMMIT;`. O contrato do
runner é explícito — ele envia arquivo + `INSERT` de controle em **um** script (bloco
transacional implícito), e a migração não abre transação própria. 0001/0002 já
seguiam essa regra; 0003 não seguia, e isso quebrava o teste `runtime::
migracoes_nao_abrem_transacao_propria` (falha **pré-existente** do `main`, reproduzida
abaixo). Consequência registrada: o checksum de 0003 mudou de
`daa3bf8cae12…` para `e70443ce10c0…`; qualquer banco que já tenha 0003 registrado com o
checksum antigo precisa ser recriado — o runner **falha alto** com `ChecksumDrift` em
vez de fingir que está atualizado. Neste ambiente o único banco nesse estado era
`godmesh_sa`, que o próprio `tests/runtime.rs` recria a cada execução
(`DROP DATABASE … WITH (FORCE)`). O banco de dev `godmesh` tinha apenas 0001/0002
registrados, então a 0003 entra limpa no próximo start.

## 4. Saídas literais — depois

### 4.1 Os 7 testes (+1 de idempotência), primeira execução, banco criado do zero

```
$ GODMESH_SX_TEST_DB=godmesh_sx_prova cargo test -p godmesh-server --test outbox_publicador
running 8 tests
test append_publish_confirm_em_ordem ... ok
test confirm_sem_publish_falha ... ok
test lacuna_interrompe_publicacao ... ok
test efeito_irreversivel_nao_e_reenviado ... ok
test duas_instancias_nao_duplicam_publicacao ... ok
test falha_do_sink_mantem_pendente_com_attempts ... ok
test loop_publica_e_encerra_no_sinal ... ok
test runner_e_idempotente_no_mesmo_banco ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.90s
```

### 4.2 Segunda execução no **mesmo** banco (idempotência provada entre processos)

```
$ GODMESH_SX_TEST_DB=godmesh_sx_prova cargo test -p godmesh-server --test outbox_publicador
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.26s
```

`schema_migration` depois das duas execuções — `aplicado_em` continua o da **primeira**
(16:01:17), prova de que a segunda execução não reaplicou nem reescreveu nada:

```
           nome            |                             checksum                             |          aplicado_em
---------------------------+------------------------------------------------------------------+-------------------------------
 0001_core.sql             | 7bb58f96393ed9e19da106293a6b914e90e90da454a5fe907561c718340ad182 | 2026-11-18 16:01:17.762856+00
 0002_incident_tenancy.sql | b6197271384c882bf588cab64dc48fe1eb62ff08a107edd088a35bd25e9c8d80 | 2026-11-18 16:01:17.788107+00
 0003_outbox_publisher.sql | e70443ce10c093063df714dcba13c02f2b8b788413d4dca5ff376052200df0bc | 2026-11-18 16:01:17.806711+00
(3 rows)
```

### 4.3 Banco legado (objetos sem histórico) — o runner falha alto, não colide

Aplicar o runner sobre `godmesh_sx_repro` (0001/0002 aplicadas à mão, sem
`schema_migration`) agora dá erro nomeado em vez de `42P07` cru:

```
$ DATABASE_URL=…/godmesh_sx_repro GODMESH_MIGRATIONS_DIR=migrations ./target/debug/godmesh-server --migrate-only
godmesh-server: migração falhou (schema_migration v1, banco não subiu): a primeira migração
(0001_core.sql) falhou porque o banco já contém os objetos dela, mas não há registro em
schema_migration: banco criado antes do controle de migrações. Não há adoção automática de
histórico — em dev, recrie o banco (0001_core.sql: error returned from database:
relation "retention_class" already exists at line 1488)
exit=1
```

### 4.4 `cargo test --workspace`

`DATABASE_URL=postgres://…@localhost:55432/godmesh`,
`GODMESH_TEST_DATABASE_URL=postgres://…@localhost:55432/godmesh_sd`.

```
$ DATABASE_URL=postgres://…@localhost:55432/godmesh \
  GODMESH_TEST_DATABASE_URL=postgres://…@localhost:55432/godmesh_sd \
  cargo test --workspace
exit=0
```

Binário por binário (literal, na ordem de execução; total **160 aprovados, 0
falhas, 1 ignorado**):

```
godmesh-core        unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-core        tests/integration.rs        7 passed;  0 failed; 0 ignored
godmesh-emergency   unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-emergency   tests/incident.rs          19 passed;  0 failed; 0 ignored
godmesh-identity    unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-identity    tests/grant_predicate.rs    8 passed;  0 failed; 0 ignored
godmesh-policy      unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-policy      tests/high_water.rs         6 passed;  0 failed; 0 ignored
godmesh-policy      tests/retention.rs          6 passed;  0 failed; 0 ignored
godmesh-protocol    unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-protocol    tests/ack_class.rs          4 passed;  0 failed; 0 ignored
godmesh-protocol    tests/ack_composition.rs    5 passed;  0 failed; 0 ignored
godmesh-protocol    tests/cursor.rs            10 passed;  0 failed; 0 ignored
godmesh-protocol    tests/version.rs            6 passed;  0 failed; 0 ignored
godmesh-server      unittests src/lib.rs        9 passed;  0 failed; 0 ignored
godmesh-server      unittests src/main.rs       8 passed;  0 failed; 0 ignored
godmesh-server      tests/contract.rs          17 passed;  0 failed; 0 ignored   (1.36s)
godmesh-server      tests/events.rs             8 passed;  0 failed; 0 ignored   (0.18s)
godmesh-server      tests/lifecycle.rs          6 passed;  0 failed; 0 ignored   (0.75s)
godmesh-server      tests/outbox_publicador.rs   8 passed;  0 failed; 0 ignored   (1.33s)
godmesh-server      tests/retention_policy_missing.rs
                                               0 passed;  0 failed; 1 ignored   (--ignored)
godmesh-server      tests/runtime.rs            8 passed;  0 failed; 0 ignored   (3.42s)
godmesh-sync        unittests src/lib.rs        0 passed;  0 failed; 0 ignored
godmesh-sync        tests/inbox.rs              5 passed;  0 failed; 0 ignored
godmesh-sync        tests/outbox.rs            13 passed;  0 failed; 0 ignored
godmesh-sync        tests/position.rs           7 passed;  0 failed; 0 ignored
```

**Total: 0 falhas, 0 erros.** O único `ignored` do workspace é
`retention_policy_missing` (`--ignored`, pré-existente: altera o registro de política e
precisa rodar sozinho). Antes desta correção, `--test runtime` falhava em
`migracoes_nao_abrem_transacao_propria` (0003 abria `BEGIN;`), e
`--test outbox_publicador` falhava nos 7 testes quando o banco já tinha 0001/0002.

### 4.5 `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets
    Checking godmesh-identity v0.1.0 …
    …
    Checking godmesh-server v0.1.0 …
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.96s
exit=0
```

Zero `warning`, zero `error` (as lints do workspace incluem `clippy::pedantic`).

## 5. Limites desta evidência

* O teste deixou de exercitar "banco de teste pré-existente em qualquer estado": ele
  **cria** o banco. O caso legado (objetos sem histórico) está coberto por
  `tests/runtime.rs` e demonstrado em 4.3 — o runner falha alto, por decisão, sem
  adoção automática de histórico.
* A mudança de checksum de 0003 é incompatível com bancos que já a registraram; a
  correção é recriar o banco. Fora deste ambiente isso precisa de aviso no deploy.
* A execução de `--test runtime` recria `godmesh_sa` com `DROP DATABASE … WITH (FORCE)`;
  é o comportamento pré-existente daquele binário, não alterado aqui.
* Cada execução do binário deixa um banco `godmesh_sx_<pid>` no servidor de teste (é
  ele que o teste procura na execução seguinte, sem apagar nada de outro processo —
  vários agentes usam a mesma máquina). Limpeza fica manual:
  `psql …/postgres -c 'DROP DATABASE godmesh_sx_<pid> WITH (FORCE)'`. Os dois bancos
  criados nesta verificação (`godmesh_sx_prova`, `godmesh_sx_repro` no estado legado) e
  os `godmesh_sx_<pid>` das execuções foram removidos ao final.
* Nada foi verificado contra Postgres gerenciado, CI ou réplica; só o contêiner local.