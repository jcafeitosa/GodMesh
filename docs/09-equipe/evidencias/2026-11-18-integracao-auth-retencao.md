# Evidência — integração das fatias de auth e de retenção no servidor (18/11/2026)

**Worker:** INT (integração auth + retenção/idempotência), sob o CTO.
**Branch:** `room/int-integra-auth-retencao-rdvi`.
**Origem do trabalho:** dois workers corrigiram a mesma fatia (`crates/godmesh-server`) em paralelo e os commits conflitavam em `src/api.rs`. O objetivo desta integração é **compor** as duas correções, não escolher uma.

- Auth: `ed87154` — exige credencial, deriva tenancy do grant verificado, `match` exaustivo.
- Retenção/idempotência: `2f44d9f` — jurisdição por tenant, política resolvida dentro da transação, `ON CONFLICT (tenant_id, id)`.

Ordem aplicada: `2f44d9f` (sem conflito de contrato) e depois `ed87154` (reescreve a fronteira de entrada). A ordem inversa produziria o mesmo resultado final com mais conflito textual, porque `ed87154` remove campos que `2f44d9f` ainda referencia.

## 1. O que o conflito era

Os dois commits foram escritos sobre a mesma base (`0fb133a`) e não enxergavam `511157a` (que mudou a assinatura de domínio: `Incident::persist` passou a exigir `at_unix`). Conflito real, portanto, em **três eixos**:

| Eixo | `ed87154` (auth) | `2f44d9f` (retenção) | Base (`511157a`) |
|---|---|---|---|
| `IncidentPayload` | só `incident_id`, `deny_unknown_fields` | `incident_id` + `tenant_id` + `principal` + 6 cláusulas do predicate | idem `2f44d9f` |
| Assinatura | `create_incident(pool, &VerifiedGrant, payload) -> IncidentResponse` | `create_incident(pool, payload) -> CreateOutcome` | `-> IncidentResponse` |
| `persist(..)` | 2 argumentos (base velha) | 2 argumentos (base velha) | **3** argumentos (`at_unix`) |

Nenhum dos dois lados podia ser tomado inteiro: o lado auth perde idempotência/retenção; o lado retenção aceita tenancy do corpo e monta o `GrantPredicate` a partir do JSON, que é exatamente a vulnerabilidade do gate `wfr_414e32e9`.

## 2. Como foi resolvido, trecho a trecho (`crates/godmesh-server/src/api.rs`)

**(a) Tenancy e predicado vêm do grant verificado (auth, preservado).** A construção do `GrantPredicate` a partir de `payload.operation/freshness/revocation/...` foi **removida**; no lugar entrou a avaliação do grant:

```rust
grant.validate().map_err(ApiError::InvalidCredential)?;
if payload.incident_id.trim().is_empty() {
    return Err(ApiError::InvalidRequest(
        "incident_id vazio não identifica incidente".to_owned(),
    ));
}
match grant.predicate.evaluate() {
    GrantDecision::Allow => {}
    denied => return Err(ApiError::GrantDenied(denied)),
}
let tenant_id = &grant.subject.tenant_id;
let principal = &grant.subject.principal;
```

O `match` exaustivo de `ed87154` foi mantido: `Allow` é o único caminho de aceitação e o braço padrão nega qualquer variante — inclusive uma futura. `IncidentPayload` ficou só com `incident_id` + `deny_unknown_fields`.

**(b) Jurisdição é do tenant autenticado (retenção, preservado).** O `SELECT ... FOR SHARE` em `tenant_region` continuou igual, mas a chave passou a ser `tenant_id` do grant, não do corpo:

```rust
// Jurisdição do tenant autenticado vem do banco, nunca do pedido (ADR-066).
let jurisdiction: Option<String> =
    sqlx::query_scalar("SELECT jurisdiction FROM tenant_region WHERE tenant_id = $1 FOR SHARE")
        .bind(tenant_id)
```

**(c) Política lida dentro da transação (retenção, preservado).** O `pool.begin()` + `FOR SHARE OF rc, cp` (classe **e** cópia primária) + FK composta `sos_incident_retention_class_fkey` ficaram intactos. A checagem fora da escrita foi eliminada, como em `2f44d9f`; o `retention_policy_missing` agora é por jurisdição, não pelo literal `'BR'`.

**(d) Idempotência por `(tenant_id, id)` (retenção, preservado).** `INSERT ... ON CONFLICT (tenant_id, id) DO NOTHING`, `rows_affected()` decide criação × replay, `is_incident_id_conflict` traduz a violação de `sos_incident_id_key` em `IncidentIdConflict`, e o replay relê o estado persistido em vez de reescrever ACK (trilha append-only, ADR-058). O `INSERT` grava as colunas novas `retention_class`/`retention_jurisdiction` com os valores devolvidos pelo `SELECT ... FOR SHARE`, e `tenant_id`/`principal` do grant.

**(e) `at_unix` real (autoridade da base `511157a`).** Ambos os commits carregavam a chamada antiga de `persist` com 2 argumentos. Prevaleceu a assinatura da base: relógio do host, nunca zero. Na reidratação de replay o instante é sintético (`1`, nunca `0`) e documentado — não é persistido, só serve para calcular rótulo.

**(f) `CreateOutcome` + `VerifiedGrant` convivem.** A assinatura final é `create_incident(pool, &VerifiedGrant, payload) -> CreateOutcome`. `created` alimenta 201/200; o erro continua sem promessa de efeito.

## 3. `crates/godmesh-server/src/http.rs`

Uma conflito só, no `post_incident`. Resolução compondo:

```rust
// Corpo inválido é recusado na fronteira, antes de qualquer efeito.
let Json(payload) = payload.map_err(|rejection| {
    ApiErrorResponse::from(ApiError::InvalidRequest(rejection.body_text()))
})?;
match create_incident(&state.pool, &grant, payload).await {
    Ok(outcome) => Ok((created_status(outcome.created), Json(outcome.response))),
    Err(e) => Err(ApiErrorResponse::from(e)),
}
```

Preservado de `ed87154`: `route_layer(require_grant)`, `Extension<VerifiedGrant>`, `Result<Json<_>, JsonRejection>`, 401 + `WWW-Authenticate`, 422 `invalid_credential`/`invalid_request`, `status_and_code()`. Preservado de `2f44d9f`: `created_status(created)` → 201/200 e os 409 `retention_policy_missing`, `retention_scope_unresolved`, `incident_id_conflict`. A linha morta `let _ = GrantDecision::Allow;` da base (e o import de `GrantDecision`) desapareceu com a resolução.

## 4. Testes: as duas suítes preservadas (`tests/contract.rs`, 17 testes)

Nenhum teste foi apagado. As três funções que existiam **nas duas** suítes foram unificadas (mesmo nome, mesmas asserções do núcleo, agora no formato grant + `CreateOutcome`): `denies_incident_without_trusted_freshness_source`, `persists_incident_and_never_claims_more_than_persisted_local`, `refuses_precise_location_offline`. 13 (auth) + 7 (retenção) − 3 duplicatas = **17**.

Ajustes necessários, com a decisão que os autoriza — nenhum enfraquece asserção:

1. **Testes de auth que esperavam 201/`Ok` passaram a declarar a jurisdição do tenant.** `tenant_and_principal_come_from_the_verified_grant` e `persists_incident_and_never_claims_more_than_persisted_local` chamam `declare_tenant(&pool, "tenant-a", "BR")` antes. Motivo: ADR-066 torna a jurisdição **dado do servidor** e a ausência dela bloqueia persistência (`retention_scope_unresolved`). Um teste que quer o caminho feliz precisa declarar o tenant que o grant identifica — isso é fixture, não relaxamento. As asserções de auth continuam todas: `tenant_id='tenant-a'`, `principal='principal-42'`, e o teste HTTP ganhou a asserção extra `retention_jurisdiction='BR'`, provando que a jurisdição vem de `tenant_region` do tenant autenticado.
2. **Testes de retenção que chamavam o caso de uso direto passaram a passar um `VerifiedGrant`** (`allow_grant(TENANT_BR)` / `allow_grant(TENANT_CO)`), porque o corpo não carrega mais tenancy nem predicado. As asserções de retenção/idempotência ficaram idênticas: `created`, igualdade de `IncidentResponse` no replay, `count(*)` de incidente e de ACK, `IncidentIdConflict` no squatting, `RetentionScopeUnresolved` sem jurisdição, `(retention_class, retention_jurisdiction) = ('incidente_sos','CO')`.
3. **`tests/retention_policy_missing.rs`** (caso negativo, `#[ignore]`) recebeu `co_grant()` com sujeito e predicado completos, para que a recusa medida seja a de **retenção** e não a de autorização. Continua isolado e restaurando a semente de CO.
4. Todos os testes que tocam banco seguram o `DB_LOCK` (herdado de `ed87154`), inclusive os de retenção: `count(*)` global só é evidência com o binário serializado.

Contradição entre suítes: **não houve nenhuma de contrato.** Auth diz "tenant vem do grant"; retenção diz "jurisdição vem do tenant". São autoridades em série, não concorrentes. Onde a composição criou atrito foi só em fixture (declarar a região), nunca em regra.

## 5. Esquema

`migrations/0002_incident_tenancy.sql` continua sendo a fonte do esquema (`tenant_region`, colunas de retenção com FK composta, PK `(tenant_id, id)`, `UNIQUE (id)`, semente `incidente_sos@CO`). `migrations/tests/0001_invariants.sql` foi preservado na versão de `2f44d9f` (INSERTs declaram `retention_class`/`retention_jurisdiction`) e o `INSERT` do servidor usa as mesmas colunas novas. `0001_core.sql` não foi alterado por nenhum dos dois commits e continua igual.

## 6. Verificações realmente executadas

Banco criado **do zero** a partir de `migrations/0001_core.sql` + `migrations/0002_incident_tenancy.sql` (nessa ordem) num database novo do Postgres 18.7 em OrbStack (`postgres:18-alpine`, host `127.0.0.1:55432`). Nada de banco pré-existente.

### `cargo test --workspace`

```
DATABASE_URL=postgres://godmesh:godmesh_dev_only@127.0.0.1:55432/godmesh_int_43ae3e1_56639
```

```
     Running unittests src/lib.rs (target/debug/deps/godmesh_core-e5f996edd9a86423)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration.rs (target/debug/deps/integration-5cd9c74916df59b5)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_emergency-f39efea3153ac1de)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/incident.rs (target/debug/deps/incident-93aacc247794ba80)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_identity-5bf40ef0d8a25a72)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/grant_predicate.rs (target/debug/deps/grant_predicate-905fc92bb56df24b)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_policy-09762a999124f403)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/high_water.rs (target/debug/deps/high_water-7e45140ca16037cb)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/retention.rs (target/debug/deps/retention-a0c5472fc7771260)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_protocol-b8dc5401b00577b9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_class.rs (target/debug/deps/ack_class-3e00e907730063f1)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_composition.rs (target/debug/deps/ack_composition-402abbad330ac268)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cursor.rs (target/debug/deps/cursor-e12bb93bab62134c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/version.rs (target/debug/deps/version-1de034f4d18006a0)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_server-99e10b8959f1e906)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/godmesh_server-fca64c1fb2f5b40b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/contract.rs (target/debug/deps/contract-90c3361220963ba3)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.55s
     Running tests/retention_policy_missing.rs (target/debug/deps/retention_policy_missing-a73fb86c1214c056)
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_sync-32803dc125902f9b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/inbox.rs (target/debug/deps/inbox-d0ef9283da158834)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/outbox.rs (target/debug/deps/outbox-e4a655bb1877cb2f)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/position.rs (target/debug/deps/position-631b6363c3fec994)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
... (7 blocos de doc-test, todos 0 passed; 0 failed)
```

Nenhum `SKIP: DATABASE_URL ausente` na saída: os 17 contratos rodaram contra Postgres real.

Caso negativo de retenção, em banco próprio (`godmesh_int_neg_59554`), também criado do zero com 0001+0002:

```
$ DATABASE_URL=.../godmesh_int_neg_59554 cargo test -p godmesh-server --test retention_policy_missing -- --ignored --nocapture
running 1 test
test colombian_tenant_without_policy_is_refused_with_retention_policy_missing ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

### `migrations/tests/0001_invariants.sql`

```
psql:migrations/tests/0001_invariants.sql:15: NOTICE:  OK 1: evento sem política recusado
DO
psql:migrations/tests/0001_invariants.sql:23: NOTICE:  OK 2: evento com política aceito
DO
psql:migrations/tests/0001_invariants.sql:33: NOTICE:  OK 3: posição duplicada recusada
DO
INSERT 0 1
INSERT 0 1
psql:migrations/tests/0001_invariants.sql:52: NOTICE:  OK 4: ACK append-only (reescrita bloqueada)
DO
psql:migrations/tests/0001_invariants.sql:64: NOTICE:  OK 5: estado inventado recusado
DO
       resultado
-----------------------
 verificação concluída
(1 row)
```

### `migrations/tests/0002_invariants.sql`

```
psql:migrations/tests/0002_invariants.sql:15: NOTICE:  OK 1: incidente sem classe de retenção recusado
DO
psql:migrations/tests/0002_invariants.sql:25: NOTICE:  OK 2a: jurisdição fora de BR/CO recusada
DO
psql:migrations/tests/0002_invariants.sql:34: NOTICE:  OK 2b: incidente sem política recusado (FK)
DO
psql:migrations/tests/0002_invariants.sql:51: NOTICE:  OK 3: incidente_sos semeado para CO com cópia primária
DO
INSERT 0 1
psql:migrations/tests/0002_invariants.sql:69: NOTICE:  OK 4: incidente registra a política aplicada (incidente_sos@BR)
DO
psql:migrations/tests/0002_invariants.sql:79: NOTICE:  OK 5: colisão de id entre tenants recusada
DO
psql:migrations/tests/0002_invariants.sql:96: NOTICE:  OK 6: PK de sos_incident é (tenant_id, id)
DO
psql:migrations/tests/0002_invariants.sql:106: NOTICE:  OK 7: política em uso não pode ser removida (FK)
DO
       resultado
-----------------------
 verificação concluída
(1 row)
```

Ambos com `-v ON_ERROR_STOP=1`, exit 0.

### `cargo clippy --workspace --all-targets`

```
    Checking godmesh-server v0.1.0 (.../crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.77s
```

Sem nenhum warning emitido.

## 7. O que NÃO foi provado

- **Concorrência real** entre dois `POST` simultâneos para o mesmo `(tenant_id, id)` e entre o `INSERT` e a remoção da política. Os caminhos (`ON CONFLICT`, `FOR SHARE`, FK imediata) existem e são a razão de estarem dentro de uma transação, mas nenhum teste roda dois clientes em paralelo. Os locks não têm teste de contenção.
- **Verificação criptográfica de credencial.** O token continua opaco e resolvido por igualdade em `GrantRegistry` (configuração `GODMESH_GRANTS`); não há claim, assinatura, DPoP nem introspecção. "Autenticado" aqui significa "token presente no registro", não prova de posse.
- **Revogação e expiração de grant** não existem nesta fatia.
- **`at_unix` do replay** é sintético (`1`) e só serve para derivar rótulo; não há teste de reidratação para estados acima de `persisted_local` (o endpoint só produz `persisted_local`, e os demais ramos de `rehydrate` são código sem cobertura de contrato).
- **Binário completo** (`GODMESH_GRANTS` obrigatória, exit code) não foi exercitado de ponta a ponta nesta integração; foi apenas compilado (`cargo check --all-targets`, clippy). O smoke HTTP é feito pelo router em porta efêmera dentro dos testes.
- **PostGIS** continua fora; sem impacto nesta fatia.
- **Nenhuma medição de desempenho** foi feita. Nenhum número aqui é estimativa.
- **`0001_core.sql`** não foi reescrito para declarar as colunas de retenção; a evolução fica em `0002`, por decisão de manter migração incremental e reaplicável. Um banco criado só com `0001` continua válido como estado intermediário, mas o servidor exige `0002`.