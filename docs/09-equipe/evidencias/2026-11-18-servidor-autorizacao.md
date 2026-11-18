# Evidência — autorização real do grant no servidor (18/11/2026)

**Objetivo:** substituir "autenticado = está no registro `GODMESH_GRANTS`" por um
modelo de grant com escopo, janela de validade e revogação persistente, sem
inventar criptografia (ADR-026/ADR-031).

**Escopo desta entrega**

1. Modelo `GrantRecord` carregado de `GODMESH_GRANTS` (JSON) com validação de
   esquema.
2. `VerifiedGrant` com sujeito + escopos injetado pelo middleware; cada rota
   declara o escopo exigido.
3. Revogação persistente em `grant_revocation` (migração 0004), consultada em
   banco com cache curto e invalidação explícita.
4. Testes de recusa por caso, caminho feliz e segredo em log/erro.
5. `cargo test --workspace` e `cargo clippy --workspace --all-targets`.

**O que esta evidência NÃO prova**

- Não há emissão, rotação, introspecção nem prova de posse da credencial
  (DPoP/mTLS). O token é opaco e resolvido por igualdade em tempo constante
  contra o registro.
- Não há criptografia própria, JWT, HMAC ou assinatura: nenhuma dependência nova
  foi adicionada (ADR-026/ADR-031).
- Não há endpoint administrativo de revogação; a revogação é uma linha em
  `grant_revocation`, inserida por operação/SQL. O servidor só a **lê**.
- O cache de revogação é por processo: com TTL de 5 s, uma revogação pode levar
  até 5 s para valer em cada réplica, salvo invalidação explícita via
  `Authorization::invalidate_revocation`.
- Nada aqui foi validado em produção, com múltiplas réplicas ou sob carga.

## Modelo do grant

`crates/godmesh-server/src/auth.rs`:

```rust
pub struct GrantRecord {
    pub token_id: String,
    pub subject: Subject,          // tenant_id + principal
    pub scopes: Vec<Scope>,        // incident/event/sync : write/read
    pub not_before: i64,           // segundos Unix
    pub expires_at: i64,           // segundos Unix, obrigatório
    pub revoked_at: Option<i64>,   // revogação declarada na configuração
    pub authority_epoch: u64,
    pub predicate: GrantPredicate, // ADR-059/ADR-064, preservado
}
```

- `expires_at` é obrigatório e precisa ser posterior a `not_before`: grant sem
  prazo é recusado na carga da configuração.
- `token_id` é obrigatório e não se repete; token vazio ou repetido, escopo
  vazio ou repetido e campo desconhecido também são erro de configuração.
- A comparação do token continua sendo varredura completa em tempo constante por
  entrada, sem saída antecipada (comportamento preservado).
- O token nunca aparece em `Debug`, em erro, na auditoria ou em log. O `Debug` de
  `RegistryEntry` redige o campo e a auditoria registra apenas `token_id` + motivo.

## Aplicação por rota

O portão de autenticação (`require_grant`) roda antes de tudo e injeta
`VerifiedGrant`. Cada grupo de rotas declara o escopo na construção do router:

| Rota | Escopo exigido |
|---|---|
| `POST /v1/incidents` | `incident:write` |
| `POST /v1/incidents/{id}/replicate` | `incident:write` |
| `POST /v1/incidents/{id}/ack-technical` | `incident:write` |
| `POST /v1/incidents/{id}/acknowledge` | `incident:write` |
| `POST /v1/incidents/{id}/close` | `incident:write` |
| `GET /v1/incidents/{id}` | `incident:read` |
| `POST /v1/events` | `event:write` |
| `GET /v1/events` | `event:read` |

`sync:write`/`sync:read` existem no modelo; não há rota de sync nesta fatia.

## Códigos de erro por caso

| Caso | HTTP | `error` |
|---|---|---|
| Sem header, esquema errado ou token desconhecido | 401 | `unauthenticated` |
| `not_before` no futuro | 401 | `credential_not_yet_valid` |
| `expires_at` no passado | 401 | `credential_expired` |
| Revogado na configuração ou em `grant_revocation` | 401 | `credential_revoked` |
| Grant válido sem o escopo da rota | 403 | `insufficient_scope` |
| Sujeito verificado incompleto (comportamento preservado) | 422 | `invalid_credential` |
| Revogação ilegível no banco | 503 | `revocation_unavailable` |

A ordem é deliberada: credencial inválida (401) é decidida antes de qualquer
juízo de escopo (403). Consulta de revogação que falha **nega**; nunca aceita.

## Revogação persistente

`migrations/0004_grant_revocation.sql`:

```sql
CREATE TABLE grant_revocation (
    token_id   text        PRIMARY KEY,
    revoked_at timestamptz NOT NULL DEFAULT now(),
    reason     text        NOT NULL CHECK (btrim(reason) <> '')
);
```

- `PgRevocationSource` consulta a tabela por `token_id`; a ausência de linha
  significa "não revogado".
- `RevocationCache` guarda o resultado por 5 s (`REVOCATION_CACHE_TTL_SECONDS`).
  TTL 0 desliga a reutilização. `Authorization::invalidate_revocation` e
  `invalidate_all_revocations` forçam releitura.
- Revogar não exige reiniciar: o teste HTTP usa **um único router** e prova a
  mudança de 401 depois do `INSERT` em `grant_revocation`.

## Testes adicionados

`crates/godmesh-server/tests/authorization.rs` (10 testes, HTTP real em porta
efêmera):

| Teste | Prova |
|---|---|
| `valid_token_without_route_scope_is_403_insufficient_scope` | 403 `insufficient_scope` |
| `event_routes_declare_their_own_scopes` | `POST /v1/events` e `GET /v1/events` cobram escopos distintos |
| `expired_grant_is_401_credential_expired` | 401 `credential_expired` |
| `future_not_before_is_401_credential_not_yet_valid` | 401 `credential_not_yet_valid` |
| `unknown_token_is_401_unauthenticated` | 401 `unauthenticated` |
| `revocation_in_database_is_401_without_restart` | 401 `credential_revoked` após `INSERT`, mesmo router |
| `revocation_from_source_is_401_credential_revoked` | fonte de revogação injetada nega |
| `valid_grant_with_scope_is_201` | grant válido continua 201 e persiste |
| `no_token_appears_in_log_or_error_body` | auditoria capturada e corpo não contêm o token |
| `verified_grant_exposes_scopes_from_the_record` | `VerifiedGrant` carrega escopos |

Testes unitários em `auth.rs` cobrem janela, escopo, revogação declarada, TTL do
cache, invalidação explícita, validação de esquema e redação do token em
`Debug`/erro.

## Verificações executadas

Ambiente: macOS arm64, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`,
`cargo 1.99.1 (5f94df478 2026-08-27)`, `PostgreSQL 16.15 (Homebrew)` iniciado em
cluster temporário local (`initdb` em `/tmp`, porta 55499, **sem Docker**), com
`migrations/0001_core.sql`, `0002_incident_tenancy.sql` e
`0004_grant_revocation.sql` aplicadas.

### `cargo test --workspace`

```
$ DATABASE_URL='postgres://godmesh@localhost:55499/godmesh' cargo test --workspace
...
test result: ok. 154 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Somatório dos 32 binários de teste do workspace: **154 passed, 0 failed, 1
ignored**. O teste ignorado é preexistente
(`retention_policy_missing::colombian_tenant_without_policy_is_refused_with_retention_policy_missing`,
que roda isolado com `--ignored`).

Sem `DATABASE_URL`, os testes que exigem banco imprimem `SKIP` e retornam — não
fingem sucesso. A execução acima teve banco real.

### `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Exit 0, zero warnings (o workspace nega `clippy::all`).

### Invariantes de banco (migração 0004)

```
$ PGHOST=/tmp PGPORT=55499 PGUSER=godmesh migrations/tests/run_all.sh
...
===== 0004_invariants.sql =====
NOTICE:  OK 1: token_id duplicado recusado (PK)
NOTICE:  OK 2: motivo vazio recusado (CHECK)
NOTICE:  OK 3: revoked_at preenchido por default
NOTICE:  OK 4: consulta distingue ausente de revogado

RESULTADO: OK — 18 invariantes verificados em banco criado do zero.
```

`run_all.sh` agora aplica a migração 0004 e exige explicitamente os 4 blocos
novos; remover uma asserção quebra o script.

## Arquivos alterados

- `crates/godmesh-server/src/auth.rs` — modelo, escopos, relógio, revogação,
  cache, auditoria, autorização.
- `crates/godmesh-server/src/http.rs` — portões de autenticação/escopo por rota,
  composição `router`/`router_with`, tradução dos novos erros.
- `crates/godmesh-server/src/api.rs` — variantes de erro e conversão a partir de
  `AuthError`.
- `crates/godmesh-server/tests/authorization.rs` — novos testes HTTP.
- `crates/godmesh-server/tests/{contract,events,lifecycle,retention_policy_missing}.rs`
  — fixtures passam a preencher os campos novos de `VerifiedGrant`.
- `migrations/0004_grant_revocation.sql` — tabela de revogação.
- `migrations/tests/0004_invariants.sql` — invariantes de banco.
- `migrations/tests/run_all.sh` — aplica e exige a migração 0004.

## Dependências

Nenhuma dependência nova. Versões mantidas: `axum 0.8.9`, `serde 1.0.229`,
`serde_json 1.0.154`, `sqlx 0.9.1`, `tokio 1.54.1` (já resolvidas em
`Cargo.lock`). Não foi necessário JWT/HMAC; a decisão registrada é token opaco +
banco, compatível com ADR-026/ADR-031.

## Limitações e pendências

- Revogação por processo com TTL de 5 s; sem invalidação distribuída entre
  réplicas.
- Sem emissão, rotação, auditoria durável de uso ou endpoint administrativo.
- `sync:write`/`sync:read` modelados, sem rota correspondente nesta fatia.
- Testes de banco dependem de `DATABASE_URL` e das migrações aplicadas; o script
  de invariantes cobre o esquema da revogação, não a concorrência entre réplicas.