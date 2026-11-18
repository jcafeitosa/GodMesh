# Evidência — emissão e revogação de grant por API (18/11/2026)

**Worker:** T1 — emissão e revogação de grant por API. **Branch:** `room/t1-emissao-e-revogacao-de-grant-tkuy`.
**Ambiente:** macOS arm64, Postgres 18.7 em `localhost:55432`, Rust `cargo`/`clippy` do workspace. **Sem contêiner** (ADR-070).
**Natureza deste documento:** relato de implementação e de verificação executada. **Não** é aprovação de SPEC, de contrato publicado ou de prontidão para produção.

> **Nota de integração (T3, 18/11/2026):** ao integrar no `main`, esta migração foi renumerada de `0006_grant_issuance.sql` para `0007_grant_issuance.sql` (e o teste para `0007_invariants.sql`) porque `0006` já pertence à listagem de incidentes (T2). As saídas literais abaixo são as da execução original de T1, antes da renumeração; a execução integrada está em `2026-11-18-integracao-grants.md`.

## 1. O que existia e o que faltava

Existia: `GrantRegistry` a partir de `GODMESH_GRANTS`, middleware `require_grant`/`require_scope`, revogação persistente em `grant_revocation` (migração 0004) com cache de 5 s e invalidação explícita.

Faltava: ciclo administrativo — emitir credencial sem reeditar configuração e reiniciar, consultar metadados e revogar por API com efeito imediato no processo.

## 2. Decisões

| Tema | Decisão | Justificativa |
|---|---|---|
| Aleatoriedade do token | `getrandom` 0.4.3, `fill(&mut [u8])`, 32 bytes; token `gm_` + 64 hex; `token_id` `gtr_` + 32 hex (16 bytes) | RNG do sistema operacional; nenhuma semente de relógio/contador/configuração. Sem entropia a emissão falha (503), não degrada |
| Hash do token | `sha2` 0.11.1 (`Sha256::digest`), hex minúsculo de 64 caracteres com `hex` 0.4.3 | Crate auditada (RustCrypto), já presente no grafo do workspace antes desta mudança. ADR-026/ADR-031: nenhuma cifra própria |
| Alternativa descartada | `sha256()`/`digest()` do Postgres, como em `migrate.rs` | O hash teria que ser calculado na consulta, o que envia o **token em claro** como parâmetro ao banco (risco de aparecer em `log_statement`). Aqui o banco nunca vê o token |
| Comparação | hash calculado × hash gravado, laço com `|`/`^` sobre todos os bytes | Comparação em tempo constante; a decisão não fica dependente de curto-circuito por posição |
| Escopo novo | `admin:grants`, separado dos escopos de domínio | Rota administrativa não herda escopo de incidente/evento/sync |
| `admin:grants` emitível? | **Não** pela rota | Credencial administrativa nasce fora da API (`GODMESH_GRANTS`). Evita escalada de privilégio por API; corpo pedindo esse escopo é 422 |
| `authority_epoch` | Constante `ISSUED_AUTHORITY_EPOCH = 1` | O corpo definido pela tarefa não tem epoch. Valor fixo e declarado é mais honesto do que derivar de grant alheio; subir epoch continua operação de autoridade |
| Revogação | `grant_revocation` (0004) + invalidação explícita do cache após o commit | Efeito imediato no mesmo processo, sem reiniciar nem esperar os 5 s de TTL |
| Idempotência da revogação | `ON CONFLICT (token_id) DO NOTHING` | Repetir a chamada não reescreve o instante original nem apaga a evidência |

### Versões, fonte e data

| Dependência | Versão resolvida | Fonte consultada | Data | Licença declarada |
|---|---|---|---|---|
| `getrandom` | 0.4.3 (já no `Cargo.lock` como indireta; agora direta) | `https://docs.rs/getrandom/0.4.3/getrandom/fn.fill.html` (HTTP 200) | 18/11/2026 | MIT OR Apache-2.0 |
| `sha2` | 0.11.1 (já no `Cargo.lock` como indireta; agora direta) | `https://docs.rs/sha2/0.11.1/sha2/` (HTTP 200) | 18/11/2026 | MIT OR Apache-2.0 |
| `hex` | 0.4.3 (já no `Cargo.lock` como indireta; agora direta) | crate já resolvida no lock do workspace | 18/11/2026 | MIT OR Apache-2.0 |
| `const-oid` | 0.10.3 | **nova** entrada transitiva exigida por `digest 0.11` ao habilitar `sha2` como dependência direta; sem uso direto no código | 18/11/2026 | Apache-2.0 OR MIT |

Nenhuma migração destrutiva, nenhum upgrade de dependência existente: só arestas diretas para crates já resolvidas.

## 3. Esquema — `migrations/0006_grant_issuance.sql`

`grant_issued(token_id PK, tenant_id, principal, token_hash UNIQUE, scopes text[], not_before, expires_at, authority_epoch, created_at)` com CHECKs: identificadores não vazios, `token_hash ~ '^[0-9a-f]{64}$'`, `cardinality(scopes) > 0`, `expires_at > not_before`, `authority_epoch >= 0`.

Não existe coluna para o token. A consulta de metadados não seleciona `token_hash`, então o hash não sai por essa rota nem por engano. Invariantes de banco novos em `migrations/tests/0006_invariants.sql` (5 blocos), registrados em `run_all.sh`.

## 4. Rotas e códigos

| Rota | Escopo | Sucesso | Recusas |
|---|---|---|---|
| `POST /v1/admin/grants` | `admin:grants` | **201** `{token_id, token, tenant_id, principal, scopes, not_before, expires_at, authority_epoch}` — token aparece **só aqui** | 401 `unauthenticated`; 403 `insufficient_scope`; 422 `invalid_request` (identificador vazio/longo, `scopes` vazio/repetido/desconhecido, `admin:grants` no corpo, TTL ≤ 0 ou > 30 dias); 503 `randomness_unavailable`; 503 `storage_unavailable` |
| `POST /v1/admin/grants/{token_id}/revoke` | `admin:grants` | **200** `{token_id, revoked: true, revoked_at}` (idempotente) | 401; 403; 404 `grant_not_found`; 422 `invalid_request` (`reason` vazio/só espaço/acima de 1024) |
| `GET /v1/admin/grants/{token_id}` | `admin:grants` | **200** metadados sem token e sem hash (`revoked`, `revoked_at`, `revocation_reason`) | 401; 403; 404 `grant_not_found` |

Uso do token emitido nas rotas normais: aceito pelo middleware **ao lado** dos tokens de `GODMESH_GRANTS`; revogado → **401 `credential_revoked`**; expirado → **401 `credential_expired`**; sem o escopo da rota → **403 `insufficient_scope`**. Falha ao consultar `grant_issued` → **503 `issuance_unavailable`** (falha fechada).

Auditoria: `event=grant_issued|grant_revoked|grant_issue_denied token_id=… by=<token_id de quem agiu>`. Nunca o token, nunca o hash.

Arquivos: `crates/godmesh-server/src/{auth,admin,api,http,lib}.rs`, `crates/godmesh-server/Cargo.toml`, `migrations/0006_grant_issuance.sql`, `migrations/tests/{0006_invariants.sql,run_all.sh}`, `crates/godmesh-server/tests/admin_grants.rs`.

## 5. Verificação executada

### 5.1 `cargo test --workspace` (com `DATABASE_URL` no Postgres local)

```
suites: 37   passed: 218   failed: 0   ignored: 1
```

Suítes do servidor (recorte):

```
     Running unittests src/lib.rs (godmesh-server)
test result: ok. 29 passed; 0 failed; 0 ignored
     Running unittests src/main.rs (godmesh-server)
test result: ok. 8 passed; 0 failed; 0 ignored
     Running tests/admin_grants.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
     Running tests/authorization.rs
test result: ok. 10 passed; 0 failed; 0 ignored
     Running tests/e2e.rs
test result: ok. 7 passed; 0 failed; 0 ignored
     Running tests/inbox.rs
test result: ok. 12 passed; 0 failed; 0 ignored
     Running tests/outbox_publicador.rs
test result: ok. 8 passed; 0 failed; 0 ignored
     Running tests/runtime.rs
test result: ok. 8 passed; 0 failed; 0 ignored
     Running tests/retention_policy_missing.rs
test result: ok. 0 passed; 0 failed; 1 ignored
```

O único `ignored` é o teste de política ausente, que já era ignorado por contrato antes desta mudança. Três execuções completas consecutivas terminaram com exit 0.

**Ocorrência honesta:** em uma execução completa anterior, `tests/runtime.rs` falhou 1 de 8 (`5,13 s`). A suíte isolada e as três execuções completas seguintes passaram; a falha não se reproduziu e o log daquela execução não foi preservado. Não foi atribuída a esta mudança nem considerada resolvida.

Testes novos (`tests/admin_grants.rs`, banco descartável `godmesh_admin_<pid>` migrado pelo runner oficial):

```
test rotas_administrativas_exigem_escopo_admin_grants ... ok
test token_expirado_e_401_credential_expired ... ok
test grant_so_de_leitura_nao_escreve_incidente ... ok
test token_nao_aparece_em_resposta_posterior_nem_em_log ... ok
test emissoes_invalidas_e_token_desconhecido_sao_recusados ... ok
test emite_usa_revoga_e_token_morre_sem_reiniciar ... ok
```

O teste de ciclo completo usa TTL de cache de revogação de **300 s**: a recusa após revogar só pode vir da invalidação explícita, não de expiração do cache.

### 5.2 `cargo clippy --workspace --all-targets`

```
    Checking godmesh-server v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
```

Sem erro nem aviso.

### 5.3 Invariantes de banco — `migrations/tests/run_all.sh`

```
===== 0006_invariants.sql =====
NOTICE:  OK 1: token_hash fora do formato sha256 hex recusado (CHECK)
NOTICE:  OK 2: hash de token duplicado recusado (UNIQUE)
NOTICE:  OK 3: grant sem escopo recusado (CHECK)
NOTICE:  OK 4: janela invertida recusada (CHECK)
NOTICE:  OK 5: grant_issued guarda token_hash e nenhuma coluna de token

RESULTADO: OK — 35 invariantes verificados em banco criado do zero.
```

### 5.4 Binário real (composição de produção `Authorization::new`), banco descartável

Servidor `./target/debug/godmesh-server` em `127.0.0.1:18086`, `GODMESH_GRANTS` com um grant `admin:grants` de bootstrap, banco `godmesh_smoke_t1` criado e migrado pelo próprio binário, removido ao fim.

| # | Operação | Resultado observado |
|---|---|---|
| 1 | `POST /v1/admin/grants` (`incident:write`, ttl 600) | **201** com `token` e `token_id` |
| 2 | `POST /v1/incidents` com o token emitido | **201** `state=persisted_local` |
| 3 | `GET /v1/admin/grants/{token_id}` | **200** metadados; sem `token`, sem `token_hash` |
| 4 | `POST /v1/admin/grants/{id}/revoke` (`reason=smoke`) | **200** `revoked=true` |
| 5 | `POST /v1/incidents` com o mesmo token, sem reiniciar | **401** `credential_revoked` |
| 6 | emissão com `scopes=["incident:read"]` + `POST /v1/incidents` | **403** `insufficient_scope` |
| 7 | `POST /v1/admin/grants` usando token sem `admin:grants` | **403** `insufficient_scope` |
| 8 | `GET /v1/admin/grants/gtr-nao-existe` | **404** `grant_not_found` |
| 9 | procura dos três tokens no log do processo | **0** ocorrência |
| 10 | `SELECT token_id, length(token_hash), scopes FROM grant_issued` | `1` linha, `64`, `{incident:write}` |
| 11 | `SELECT token_id, reason FROM grant_revocation` | `1` linha com o motivo |
| 12 | log de auditoria | `grant_issued token_id=… by=g-admin-smoke`; `grant_revoked token_id=… by=g-admin-smoke` |

## 6. Limitações e o que NÃO foi provado

- **Revogação de grant de configuração:** a rota exige linha em `grant_issued`; token de `GODMESH_GRANTS` continua revogável apenas por SQL/reeleição de configuração. Fora do escopo pedido, declarado aqui.
- **Propagação entre processos:** o efeito imediato é do processo que atendeu a revogação. Outro processo com cache quente propaga em até 5 s (`REVOCATION_CACHE_TTL_SECONDS`). Nenhum teste com múltiplas réplicas foi executado.
- **`authority_epoch` fixo em 1** para grants emitidos; não há negociação de epoch nesta rota.
- **Cross-tenant:** um detentor de `admin:grants` emite para qualquer `tenant_id`. É a autoridade que o escopo declara; não há restrição por tenant de origem.
- **Hash sem sal:** desnecessário para token de 256 bits aleatórios (não há dicionário viável); declarado para não parecer esquecimento.
- **Sem SPEC formal aprovada** para `/v1/admin/grants`: o contrato desta evidência vem da tarefa do worker. `docs/07-contratos/identidade-autorizacao.md` não lista essas rotas — lacuna documental, não suprida aqui.
- **Não executado:** carga, latência, TLS, rate limiting de emissão, auditoria durável em banco (hoje `stderr`), rotação de credencial administrativa, revogação em massa, teste de RNG degradado.
- **Revisão independente** de código: pendente.

## 7. Como reproduzir

```bash
DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  cargo test -p godmesh-server --test admin_grants -- --nocapture

DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  cargo test --workspace

cargo clippy --workspace --all-targets

PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
  migrations/tests/run_all.sh
```