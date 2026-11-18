# Evidência — integração da fatia de grants emitidos (18/11/2026)

**Worker:** T3 — integração da fatia de grants emitidos e renumeração da migração.
**Branch:** `room/t3-integra-grants-emitidos-e-migracao-00-vkxu`.
**Base integrada:** `main` @ `51be18e` + T1 `room/t1-emissao-e-revogacao-de-grant-tkuy` @ `f3db92c`.
**Ambiente:** macOS arm64, Postgres 18.7 em `localhost:55432`, Rust do workspace. **Sem contêiner** para a verificação (ADR-070).
**Natureza deste documento:** relato do que foi executado. **Não** é aprovação de SPEC, de contrato publicado ou de prontidão para produção.

## 1. Problemas encontrados e decisão

| Problema | Decisão |
| --- | --- |
| T1 numerou a migração de grants como `0006`, já ocupado por `0006_incident_listing.sql` (T2) | Migração renomeada para `migrations/0007_grant_issuance.sql`; teste para `migrations/tests/0007_invariants.sql`; referências em `auth.rs`, nos cabeçalhos SQL e no cabeçalho da evidência de T1 atualizadas |
| `run_all.sh` com duas listas `esperado_0006` (T1 e T2) | Reconstruído: aplica `0001`…`0007` na ordem, uma lista `esperado_000N` por arquivo, `verificar` de `0001`…`0007` |
| `auth.rs`/`http.rs` em conflito com escopos do SF, `admin:metrics`, listagem da T2 e grants | Preservados os dois escopos (`AdminMetrics`, `AdminGrants`), `Scope::ALL` com 8, todos os handlers e rotas |
| `http.rs` passaria de 750 linhas e o conflito era estrutural | Roteador dividido por domínio: `http/mod.rs`, `http/incidents.rs`, `http/events.rs`, `http/sync.rs`, `http/admin.rs`; `require_grant`/`require_scope` e `ApiErrorResponse` ficam em `mod.rs` |

Nenhum teste foi removido, nenhuma asserção foi enfraquecida e nenhum retry foi adicionado para mascarar falha.

## 2. Ordem final das migrações

```
0001_core.sql
0002_incident_tenancy.sql
0003_outbox_publisher.sql
0004_grant_revocation.sql
0005_sync_inbox.sql
0006_incident_listing.sql
0007_grant_issuance.sql
```

Testes de invariantes correspondentes: `0001_invariants.sql` … `0007_invariants.sql`.

## 3. Invariantes de banco — `run_all.sh`

Comando:

```
PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only bash migrations/tests/run_all.sh
```

Saída literal (trecho final, incluindo o arquivo novo):

```
===== 0006_invariants.sql =====
NOTICE:  OK 1: list_seq atribuída pelo banco cresce na inserção (8 -> 9)
NOTICE:  OK 2: list_seq não aceita escrita explícita
NOTICE:  OK 3: índice sos_incident_listagem (tenant_id, list_seq DESC) presente

===== 0007_invariants.sql =====
NOTICE:  OK 1: token_hash fora do formato sha256 hex recusado (CHECK)
NOTICE:  OK 2: hash de token duplicado recusado (UNIQUE)
NOTICE:  OK 3: grant sem escopo recusado (CHECK)
NOTICE:  OK 4: janela invertida recusada (CHECK)
NOTICE:  OK 5: grant_issued guarda token_hash e nenhuma coluna de token

RESULTADO: OK — 38 invariantes verificados em banco criado do zero.
```

Contagem: 6 (0001) + 8 (0002) + 6 (0003) + 4 (0004) + 6 (0005) + 3 (0006) + 5 (0007) = 38.

## 4. Testes — três execuções completas

Comando em cada execução:

```
DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh' cargo test --workspace
```

| Execução | Resultado agregado | Falhas |
| --- | --- | --- |
| 1 | `passed=235 failed=0` | nenhuma |
| 2 | `passed=235 failed=0` | nenhuma |
| 3 | `passed=235 failed=0` | nenhuma |

Inclui `tests/admin_grants.rs` com 6 testes:

```
running 6 tests
test rotas_administrativas_exigem_escopo_admin_grants ... ok
test token_expirado_e_401_credential_expired ... ok
test grant_so_de_leitura_nao_escreve_incidente ... ok
test emissoes_invalidas_e_token_desconhecido_sao_recusados ... ok
test token_nao_aparece_em_resposta_posterior_nem_em_log ... ok
test emite_usa_revoga_e_token_morre_sem_reiniciar ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

O flake de `tests/runtime.rs` relatado por T1 **não** se reproduziu nas três execuções. Não foi adicionado retry.

### 4.1 Falha inicial de ambiente (registrada, não mascarada)

A primeira execução falhou em dois testes de `tests/authorization.rs` antes de qualquer teste de domínio:

```
runner oficial de migrações falhou (.../godmesh): a primeira migração (0007_grant_issuance.sql) falhou porque o banco já contém os objetos dela, mas não há registro em schema_migration: banco criado antes do controle de migrações. ... relation "grant_issued" already exists
```

Causa: o banco de dev compartilhado tinha `grant_issued` e o registro `0006_grant_issuance.sql` (aplicado pela fatia T1 antes da renumeração), mas não tinha `0007_grant_issuance.sql`. O runner recusa adoção automática de histórico — comportamento correto. Correção aplicada **apenas no banco descartável de dev**, com o checksum do arquivo renumerado:

```sql
BEGIN;
DELETE FROM schema_migration WHERE nome = '0006_grant_issuance.sql';
INSERT INTO schema_migration (nome, checksum)
VALUES ('0007_grant_issuance.sql', '1a6d506ec27871454d0246ce525c6497f178b645ba7e694413047206bc0269fa');
COMMIT;
```

O checksum confere com `sha256` do arquivo em `migrations/0007_grant_issuance.sql`. Nenhuma tabela de domínio foi recriada ou apagada. As três execuções acima ocorreram depois dessa correção.

## 5. Clippy

```
$ cargo clippy --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.62s
```

Exit `0`, zero linhas `warning:`. Nenhum `#[allow]` novo foi adicionado.

## 6. Migração no banco de dev pelo binário

Banco de dev já reconciliado com a ordem final:

```
$ DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh' cargo run -q -p godmesh-server -- --migrate-only
godmesh-server: migrações em migrations — 0 aplicada(s) [], 7 já aplicada(s) [0001_core.sql, 0002_incident_tenancy.sql, 0003_outbox_publisher.sql, 0004_grant_revocation.sql, 0005_sync_inbox.sql, 0006_incident_listing.sql, 0007_grant_issuance.sql]
exit 0
```

Prova adicional de que o binário aplica a ordem final do zero, em banco temporário criado e removido na mesma execução:

```
$ DATABASE_URL='postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_t3_evidence' cargo run -q -p godmesh-server -- --migrate-only
godmesh-server: migrações em migrations — 7 aplicada(s) [0001_core.sql, 0002_incident_tenancy.sql, 0003_outbox_publisher.sql, 0004_grant_revocation.sql, 0005_sync_inbox.sql, 0006_incident_listing.sql, 0007_grant_issuance.sql], 0 já aplicada(s) []
exit 0
```

## 7. O que foi preservado no roteador

- `require_grant` (autenticação) aplicado a todo `/v1/*` e `/metrics`.
- `require_scope` por rota, incluindo `admin:grants` (emissão/consulta/revogação) e `admin:metrics` (`/metrics`).
- Rotas de incidente: criação, item, `replicate`, `ack-technical`, `acknowledge`, `close` e listagem paginada da T2.
- Eventos: `POST /v1/events` e `GET /v1/events`.
- Sync: `POST /v1/sync/inbox` e `GET /v1/sync/state`.
- `/healthz` com `SELECT 1` no pool e timeout de 500 ms.
- Observador de requisição (`observe`), métricas e log injetável (`router_with_log`).

## 8. Limitações e o que não foi provado

- Não foi executado teste de carga, concorrência de emissão/revogação sob múltiplos processos, nem failover de banco.
- O flake de `tests/runtime.rs` não foi reproduzido; a causa permanece desconhecida e não foi investigada além das três execuções.
- A correção do histórico de migração no banco de dev foi manual e específica daquele banco descartável; não há script de adoção automática no repositório, por decisão do runner.
- Nenhum push foi feito. Nenhuma aprovação de SPEC ou de produção é declarada por este documento.