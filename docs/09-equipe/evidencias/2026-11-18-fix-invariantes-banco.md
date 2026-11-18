# Evidência — invariantes de banco coerentes com o esquema vigente (18/11/2026)

**Branch:** `room/f2-invariantes-de-banco-coerentes-shet`
**Ambiente:** macOS arm64; `psql` 18.6 (libpq) contra o contêiner `postgres:18-alpine`
(PostgreSQL 18.7, `SELECT version()` = `PostgreSQL 18.7 on aarch64-unknown-linux-musl`)
em `127.0.0.1:55432`; banco descartável criado pelo próprio script. Nenhuma dependência
nova. Nada de `DROP`/`TRUNCATE` no banco de dev compartilhado (`godmesh`): só bancos
`godmesh_inv_*`/`godmesh_f2*` criados para esta verificação.

**Escopo:** `migrations/tests/`. `migrations/0001_core.sql` e
`migrations/0002_incident_tenancy.sql` **não** foram alteradas; nenhum invariante foi
enfraquecido.

## 0. Correção de premissa (importante)

A tarefa descreve o `main` com `0001_invariants.sql` produzindo 3 `NOTICE: OK`. Neste
worktree (base `798d662`, que já contém `70f6c08`) o arquivo produzia **5** `OK` quando
0002 está aplicada. O estado de 3 `OK` **reproduz** exatamente, e por outro motivo: ele
acontece sobre o esquema **só com 0001**, quando `sos_incident` ainda não tem
`retention_class`/`retention_jurisdiction`. Nesse cenário os blocos 4 e 5 falham com
`column "retention_class" of relation "sos_incident" does not exist` — não com um
invariante violado. Registro a reprodução literal abaixo, porque ela expõe o defeito
real que faltava corrigir: **o script saía com exit 0 mesmo falhando**.

## 1. Antes — esquema só com `0001_core.sql` (a premissa reproduzida)

Banco `godmesh_f2_schema0001`, `psql` direto, arquivo de então:

```
psql:<stdin>:15: NOTICE:  OK 1: evento sem política recusado
DO
psql:<stdin>:23: NOTICE:  OK 2: evento com política aceito
DO
psql:<stdin>:33: NOTICE:  OK 3: posição duplicada recusada
DO
psql:<stdin>:41: ERROR:  column "retention_class" of relation "sos_incident" does not exist
LINE 1: ...TO sos_incident (id, tenant_id, principal, state, retention_...
                                                             ^
psql:<stdin>:43: ERROR:  insert or update on table "sos_ack" violates foreign key constraint "sos_ack_incident_id_fkey"
DETAIL:  Key (incident_id)=(inc-1) is not present in table "sos_incident".
psql:<stdin>:52: ERROR:  FALHA: ACK foi reescrito
CONTEXT:  PL/pgSQL function inline_code_block line 4 at RAISE
psql:<stdin>:64: ERROR:  column "retention_class" of relation "sos_incident" does not exist
LINE 1: ...TO sos_incident (id, tenant_id, principal, state, retention_...
QUERY:  INSERT INTO sos_incident (id, tenant_id, principal, state, retention_class, retention_jurisdiction)
    VALUES (novo_id, 't1', 'p1', 'enviado', 'incidente_sos', 'BR');
CONTEXT:  PL/pgSQL function inline_code_block line 5 at SQL statement
       resultado
-----------------------
 verificação concluída
(1 row)

exit=0
```

Diagnóstico exato:

| Bloco | Resultado sobre o esquema só com 0001 | Causa |
|---|---|---|
| 1, 2, 3 | `OK` | não tocam `sos_incident` |
| 4 (ACK append-only) | **ERROR** + nenhum `OK` | INSERT do fixture falha (`retention_class` inexistente), o `sos_ack` órfão viola a FK e o `UPDATE` afeta 0 linhas; o `RAISE 'FALHA: ACK foi reescrito'` dispara sobre uma trilha vazia |
| 5 (estado fechado) | **ERROR** + nenhum `OK` | INSERT falha antes do `CHECK`, pela coluna inexistente |

Dois defeitos, então: (a) blocos 4 e 5 não declaram a dependência de 0002; (b) `psql` com
`ON_ERROR_STOP off` devolve **0** depois de erros, então nada detectava a regressão
automaticamente — o `README` só dizia "esperado cinco OK" e confiava no olho humano.

## 2. Antes/depois por arquivo

`0001_invariants.sql`:

| Bloco | Antes (0001+0002) | Depois |
|---|---|---|
| 1 evento sem classe | `OK 1` | `OK 1` (inalterado) |
| 2 evento com classe válida | `OK 2` | `OK 2` (inalterado) |
| 3 posição duplicada | `OK 3` | `OK 3` (inalterado) |
| 4 ACK append-only (UPDATE) | `OK 4`, sem provar que havia linha para reescrever | `OK 4`, com `count(*) = 1` do fixture e `RAISE 'FALHA'` fora do handler |
| 5 ACK append-only (DELETE) | **não existia** | `OK 5` — o trigger é `BEFORE UPDATE OR DELETE`; o DELETE não era provado |
| 6 estado fechado | `OK 5`, id aleatório (`floor(random()*1e6)`) | `OK 6`, id fixo `inc-check-estado` (o INSERT nunca cria linha, então a reprovação é idempotente) |

`0002_invariants.sql`: nenhuma asserção mudou; o cabeçalho dizia "7 blocos" quando o
arquivo emite **8** (`OK 1, 2a, 2b, 3, 4, 5, 6, 7`). Corrigido.

Nada foi apagado. As verificações de tenancy que a 0002 introduziu (FK composta,
`tenant_region`, PK `(tenant_id,id)`, semente CO) continuam em `0002_invariants.sql`;
`0001_invariants.sql` cobre o núcleo (posição, ACK, estado) **sobre o esquema vigente**,
declarando no cabeçalho que exige 0001+0002.

## 3. `migrations/tests/run_all.sh` — sucesso

```
$ PGHOST=127.0.0.1 PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
    migrations/tests/run_all.sh
== banco descartável: godmesh_inv_20261118091200_77588
== aplicando migrations/0001_core.sql
== aplicando migrations/0002_incident_tenancy.sql

===== 0001_invariants.sql =====
...: NOTICE:  OK 1: evento sem política recusado
...: NOTICE:  OK 2: evento com política aceito
...: NOTICE:  OK 3: posição duplicada recusada
...: NOTICE:  OK 4: ACK append-only (UPDATE bloqueado pelo trigger)
...: NOTICE:  OK 5: ACK append-only (DELETE bloqueado pelo trigger)
...: NOTICE:  OK 6: estado inventado recusado

===== 0002_invariants.sql =====
...: NOTICE:  OK 1: incidente sem classe de retenção recusado
...: NOTICE:  OK 2a: jurisdição fora de BR/CO recusada
...: NOTICE:  OK 2b: incidente sem política recusado (FK)
...: NOTICE:  OK 3: incidente_sos semeado para CO com cópia primária
...: NOTICE:  OK 4: incidente registra a política aplicada (incidente_sos@BR)
...: NOTICE:  OK 5: colisão de id entre tenants recusada
...: NOTICE:  OK 6: PK de sos_incident é (tenant_id, id)
...: NOTICE:  OK 7: política em uso não pode ser removida (FK)

RESULTADO: OK — 14 invariantes verificados em banco criado do zero.
exit=0
```

(`...` substitui o prefixo absoluto do caminho do arquivo; as linhas completas saem na
execução.) O banco `godmesh_inv_*` é removido no `trap` de saída — verificado:
`SELECT datname FROM pg_database WHERE datname LIKE 'godmesh_inv_%'` → 0 linhas.

## 4. Sabotagem — a falha é detectada (exit != 0)

**4a. Remover a política de append-only da trilha de ACK.**

```
$ cat /tmp/sabotage_drop_trigger.sql
DROP TRIGGER sos_ack_append_only ON sos_ack;

$ ... run_all.sh --sabotage /tmp/sabotage_drop_trigger.sql
FALTA: 0001_invariants.sql -> "OK 4: ACK append-only (UPDATE bloqueado pelo trigger)"
FALTA: 0001_invariants.sql -> "OK 5: ACK append-only (DELETE bloqueado pelo trigger)"
ERRO: 0001_invariants.sql produziu ERROR no banco
...
RESULTADO: FALHOU — 3 verificação(ões) ausente(s)/inválida(s).
exit=1
```

**4b. Remover a cópia primária que autoriza a política de CO.**

```
$ cat /tmp/sabotage_remove_policy.sql
DELETE FROM retention_copy WHERE class = 'incidente_sos' AND jurisdiction = 'CO';

$ ... run_all.sh --sabotage /tmp/sabotage_remove_policy.sql
...: ERROR:  FALHA: semente de CO incompleta (classes=1, copies=0)
FALTA: 0002_invariants.sql -> "OK 3: incidente_sos semeado para CO com cópia primária"
ERRO: 0002_invariants.sql produziu ERROR no banco
RESULTADO: FALHOU — 2 verificação(ões) ausente(s)/inválida(s).
exit=1
```

Os arquivos de sabotagem são temporários (`/tmp`) e **não** foram versionados.

## 5. Arquivos alterados

```
migrations/tests/0001_invariants.sql   (cabeçalho com dependência de 0002; bloco DELETE; guarda de fixture; id determinístico)
migrations/tests/0002_invariants.sql   (só o cabeçalho: 7 -> 8 blocos, comando correto)
migrations/tests/run_all.sh            (novo: banco descartável + expectativa explícita por bloco)
deploy/README.md                       (comando único de verificação)
docs/09-equipe/evidencias/2026-11-18-fix-invariantes-banco.md (este arquivo)
```

## 6. Limitações e o que não foi executado

- Não existe job de CI neste repositório; `run_all.sh` é manual. A falha silenciosa do
  `psql` só fica coberta onde o script for executado.
- Os scripts continuam com `\set ON_ERROR_STOP off` de propósito (um bloco que falha não
  deve esconder os demais). Quem rodar `psql` direto, como no texto antigo do README,
  ainda recebe exit 0 em caso de falha. A contagem de `OK` é a prova; o README agora
  aponta para `run_all.sh`.
- `0002_invariants.sql` bloqueia `DELETE` de política referenciada pela FK, mas não
  protege `retention_class`/`retention_copy` por trigger: um `UPDATE` da política ainda
  não é exercitado. Fica registrado como lacuna, não como cobertura.
- Nenhum teste Rust/HTTP foi executado nesta fatia; a mudança é só de SQL de teste,
  script e documentação.