# Evidência — federação M1: identidade de nó, manifesto e registro de pares (18/11/2026)

**Worker:** M1 — identidade de nó e registro de pares (sob o CTO).
**Branch:** `room/m1-federacao-identidade-e-registro-7sv3`.
**Ambiente:** macOS arm64; Postgres 18.7 em `localhost:55432`; Rust 1.99.1 / `cargo`/`clippy` do workspace. **Sem contêiner** (ADR-070).
**Natureza deste documento:** relato de implementação e das verificações executadas. **Não** é aprovação de SPEC, de contrato publicado nem de prontidão para produção.

## 0. Lacuna de documento de decisão — declarada antes de tudo

A tarefa manda ler **ADR-071** em `docs/06-planejamento/decisoes.md`. **ADR-071 não existe neste repositório**: o arquivo termina em **ADR-069** (`## ADR-069 — Versões latest e documentação oficial como pré-requisito de implementação`). Uma busca por `ADR-071` em todos os `.md` do repositório não retorna nada.

O que foi feito diante disso: a implementação seguiu a **especificação da tarefa** (que fixa escopo, campos, rotas, estados e códigos) mais a documentação de arquitetura existente (`docs/01-arquitetura/descoberta-ingresso.md`, `docs/01-arquitetura/nos-regioes.md`). Nenhuma decisão foi inventada como se fosse ADR aprovada. **A ADR-071 precisa ser escrita/registrada** (decisão + versões) para que esta fatia tenha rastreabilidade normativa — está listada nas pendências desta evidência.

## 1. O que existia e o que faltava

Existia: servidor axum com portão único `require_grant` + `require_scope` por rota, escopos nomeados (`auth::Scope`), erro HTTP estável (`ApiError` → corpo `{error, detail, ack_class}`), grants administrativos (0007) e runner de migrações com checksum.

Faltava: qualquer noção de **nó** (identidade criptográfica), **manifesto assinado**, **registro de par** e **ingresso por API**. Nada de transporte de pares (isso é M2).

## 2. Decisões desta fatia

| Tema | Decisão | Justificativa |
|---|---|---|
| Criptografia | `ed25519-dalek` 3.0.0, `verify_strict` | Biblioteca auditada (RustCrypto). Nenhuma curva, hash ou esquema de assinatura escrito à mão. `verify_strict` recusa chave de pequena ordem e assinatura maleável que a verificação permissiva aceitaria |
| `node_id` | `gmn_` + hex minúsculo de `sha256(public_key)` | É um **compromisso** com a chave, não a chave: `verify()` recomputa a partir da `public_key` recebida e recusa manifesto que declare `node_id` de outro nó (`NodeIdMismatch`). Alternativa escolhida entre as permitidas (multibase/base32/hex); hex evita dependência de multibase e é o formato já usado no workspace |
| Chave privada | semente de 32 bytes em hex, arquivo `GODMESH_NODE_KEY`, modo **0600**, diretório pai **0700** | Arquivo com permissão mais aberta é **recusado** (`InsecureMode`), não corrigido em silêncio. Criação com `O_EXCL` (`create_new`): dois processos não sobrescrevem a chave um do outro. Falha de escrita remove o arquivo parcial |
| Segredo em log/Debug | `Debug` de `NodeIdentity` escrito à mão (mostra `node_id`, chave pública e caminho); sem `Display`, sem `Serialize`, sem getter de semente | Teste prova que `Debug` e erros não contêm a semente. `SigningKey` zeroiza no drop (feature `zeroize` default do dalek) |
| Serialização canônica | domínio `godmesh.federation.manifest.v1` + campos com **tag de nome + comprimento u32 BE** na ordem fixa; `capabilities`/`endpoints` ordenados e deduplicados antes de assinar e de verificar; `epoch` u64 BE; `issued_at` i64 BE | JSON não é canônico (ordem de chave, espaçamento). A tag por campo impede confusão por posição. A normalização de conjuntos evita que a ordem de envio mude a assinatura — testado nos dois sentidos |
| Frescor | `issued_at` dentro de `[now - max_age, now + max_age]`; padrão `300` s; ajustável por `GODMESH_FEDERATION_JOIN_MAX_AGE_SECONDS` | A tolerância vale para o futuro também: relógio adiantado não estica a validade. Janela inválida cai no padrão em vez de recusar tudo |
| Ingresso público | `POST /v1/federation/join-requests` montado **fora** do `require_grant`, por rota | Um nó que ainda não é par não tem credencial. O pedido não autoriza nada: cria `pending` |
| Idempotência | `UNIQUE (node_id, epoch)` + `ON CONFLICT DO NOTHING` + releitura | Reenviar devolve 200 com o mesmo `join_request_id` e o mesmo `requested_at`; geração nova (`epoch`) é pedido novo |
| Aprovação | par upsertado com `trust_state='authorized'`, máquina de estados do crate aplicada antes | Pedido aprovado é idempotente. Par `revoked` **não** volta direto a `authorized` (409 `illegal_trust_transition`): precisa de `pending` e nova decisão |
| Rejeição | `state='rejected'`, **nenhum** par criado | Aprovar depois é 409, não caminho alternativo |
| Escopos | `federation:read` (listar), `federation:admin` (aprovar/rejeitar) | Ver quem pediu ingresso não é decidir quem entra |
| Chave privada no servidor | **não existe** nesta fatia | O servidor não assina manifesto nem mantém identidade de nó; `GODMESH_NODE_KEY` é do processo de nó (crate). Fica para M2/operação |

### Versões, fonte e data

| Dependência | Versão resolvida | Fonte consultada | Data | Licença declarada |
|---|---|---|---|---|
| `ed25519-dalek` | **3.0.0** (nova dependência direta) | `https://index.crates.io/ed/25/ed25519-dalek` (3.0.0 estável; `3.0.0-rc.*` são anteriores); `https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/` (HTTP 200) | 18/11/2026 | BSD-3-Clause |
| `curve25519-dalek` | 5.0.0 (transitiva do dalek) | resolvida pelo `Cargo.lock` | 18/11/2026 | BSD-3-Clause |
| `ed25519` / `signature` | 3.0.0 (transitivas) | idem | 18/11/2026 | Apache-2.0 OR MIT |
| `zeroize` | 1.9.2 (transitiva, feature default do dalek) | idem | 18/11/2026 | Apache-2.0 OR MIT |
| `sha2` | 0.11.1 (já direta no servidor; agora também no crate para o `node_id`) | já no `Cargo.lock` | 18/11/2026 | Apache-2.0 OR MIT |
| `hex` | 0.4.3 (já no lock) | já no `Cargo.lock` | 18/11/2026 | Apache-2.0 OR MIT |
| `getrandom` | 0.4.3 (já direta no servidor) | `https://docs.rs/getrandom/0.4.3/getrandom/fn.fill.html` | 18/11/2026 | MIT OR Apache-2.0 |
| `axum` | 0.8.9 (sem mudança) | já resolvido | 18/11/2026 | MIT |
| `sqlx` | 0.9.1 (sem mudança; **não** foi habilitada a feature `json`) | já resolvido | 18/11/2026 | Apache-2.0 OR MIT |

`quinn`/`rustls`/`rcgen` **não** entram nesta fatia: não há transporte. Só `ed25519-dalek` + `serde`/`serde_json` são necessários para o manifesto, e `sha2`/`hex`/`getrandom` já estavam no grafo do workspace como dependências diretas do servidor. Nenhum upgrade de dependência existente; nenhuma feature nova habilitada.

## 3. Modelo — crate `godmesh-federation`

`crates/godmesh-federation/src/`

- `identity.rs` — `NodeIdentity` (`load_or_create`, `from_env`, `load`, `create`, `generate_in_memory`, `sign`, `sign_hex`, `public_key_hex`, `node_id`), `node_id_for`, `is_node_id`, `verify_node_id`, `parse_public_key`, `verify_signature`; `IdentityError` com `EnvMissing`, `Io`, `InvalidKeyMaterial`, `InsecureMode`, `NotARegularFile`, `RandomnessUnavailable`, `InvalidPublicKey`, `InvalidSignatureEncoding`, `SignatureInvalid`.
- `manifest.rs` — `SignedManifest { node_id, public_key, cluster, region, software_version, capabilities[], endpoints[], epoch, issued_at, signature }` com `canonical_bytes()`, `sign()`, `verify()`, `verify_fresh(now, max_age)`; `ManifestError` (`EmptyField`, `FieldTooLong`, `TooManyItems`, `InvalidNodeId`, `InvalidPublicKey`, `InvalidSignatureEncoding`, `SignatureInvalid`, `NodeIdMismatch`, `Expired`, `NotYetValid`); limites `MAX_LIST_ITEMS=128`, `MAX_FIELD_LEN=256`.
- `peer.rs` — `PeerRecord { peer_id, node_id, public_key, cluster, region, trust_state, endpoints, software_version, capabilities, epoch, last_seen_at }`, `TrustState { pending, authorized, revoked, unknown }` com `as_str`/`from_name`/`can_transition_to`, `TrustTransitionError`, `PeerRecord::from_manifest`/`transition_to`/`is_authorized`.

Transições permitidas (e **somente** elas; varredura completa no teste): `unknown→pending`, `pending→authorized`, `pending→revoked`, `authorized→revoked`, `revoked→pending`, mais idempotência `pending→pending` e `authorized→authorized`.

`node_id` na API/banco é `gmn_` + 64 hex; `signature` é hex de 64 bytes; `public_key` é hex de 32 bytes (CHECK no banco).

## 4. Esquema — `migrations/0008_federation_peers.sql`

`federation_peer(node_id PK, peer_id UNIQUE, public_key, cluster, region, trust_state CHECK IN ('pending','authorized','revoked','unknown'), endpoints text[], software_version, capabilities text[], epoch, last_seen_at, updated_at)`.

`federation_join_request(join_request_id PK CHECK '^fjr_[0-9a-f]{32}$', node_id, epoch, manifest jsonb CHECK object, requested_at, approved_at, approved_by, state CHECK IN ('pending','approved','rejected','expired'))` com:

- `UNIQUE (node_id, epoch)` — idempotência do ingresso;
- `CHECK ((state = 'approved') = (approved_at IS NOT NULL))`;
- `CHECK ((state = 'approved') = (approved_by IS NOT NULL))`;
- `CHECK (approved_by IS NULL OR btrim(approved_by) <> '')`.

Nenhuma coluna de chave privada em nenhuma das tabelas (verificado por invariante de banco). Índices: `federation_peer_trust_state_idx`, `federation_join_request_state_idx (state, requested_at DESC)`.

Invariantes novos em `migrations/tests/0008_invariants.sql` (6 blocos), registrados em `migrations/tests/run_all.sh`.

**Nota de honestidade sobre o banco:** os CHECKs garantem a *forma*; a ordem das transições de confiança é aplicada na aplicação (`TrustState::can_transition_to`), não por trigger. Um `UPDATE` manual no banco ainda pode ir de `revoked` para `authorized` — o caminho **por API** não pode.

## 5. Rotas e códigos

| Rota | Auth/escopo | Sucesso | Recusas |
|---|---|---|---|
| `POST /v1/federation/join-requests` | **pública** (sem Bearer) | **201** pedido criado / **200** replay de `(node_id, epoch)` — corpo `{join_request_id, node_id, epoch, state, requested_at, approved_at, approved_by, manifest}` | **401** `invalid_manifest_signature` (assinatura não confere, hex inválido, chave fora da curva); **422** `manifest_expired` (`issued_at` velho); **422** `manifest_not_yet_valid` (futuro além da tolerância); **422** `invalid_manifest` (`node_id` não derivado da chave, campo vazio/longo, corpo fora do contrato); **503** `storage_unavailable` |
| `GET /v1/federation/join-requests[?state=pending\|approved\|rejected\|expired]` | `federation:read` | **200** lista | **401** `unauthenticated`; **403** `insufficient_scope`; **422** `invalid_request` (estado de filtro desconhecido); **500** `federation_invariant_violation` |
| `POST /v1/federation/join-requests/{id}/approve` | `federation:admin` | **200** pedido `approved` (idempotente) | **401**; **403**; **404** `join_request_not_found`; **409** `join_request_state_conflict` (rejeitado/expirado), **409** `illegal_trust_transition` (par revogado não volta direto) |
| `POST /v1/federation/join-requests/{id}/reject` | `federation:admin` | **200** pedido `rejected`, **sem** peer | **401**; **403**; **404**; **409** `join_request_state_conflict` |
| `GET /v1/federation/peers` | `federation:read` | **200** `PeerRecord[]` (com `public_key`, sem nenhum campo secreto) | **401**; **403**; **503** |

`approved_by` recebe o `principal` do grant que aprovou (o `token_id` entra na auditoria). Auditoria: `event=federation_join_requested|federation_join_denied|federation_join_approved|federation_join_approve_denied|federation_join_rejected|federation_join_reject_denied` com `join_request_id`/`node_id`/`epoch` e `by=<token_id>`. Manifesto não verificado **não** gera `node_id` no log.

## 6. Verificação executada

### 6.1 `cargo test --workspace` (com `DATABASE_URL` no Postgres local)

```
passed=263 failed=0 ignored=1
```

O único `ignored` é `retention_policy_missing`, que já era ignorado por contrato antes desta mudança.

Suíte nova — `crates/godmesh-server/tests/federation.rs` (banco descartável `godmesh_federation_<pid>` migrado pelo runner oficial):

```
running 10 tests
test manifesto_emitido_no_futuro_e_recusado ... ok
test manifesto_com_issued_at_velho_e_recusado ... ok
test join_sem_assinatura_valida_e_recusado_sem_gravar ... ok
test rotas_de_federacao_exigem_escopo ... ok
test join_valido_cria_pending_e_nao_autoriza_par ... ok
test join_duplicado_por_node_e_epoch_nao_duplica ... ok
test chave_privada_nunca_aparece_em_resposta_nem_em_log ... ok
test aprovar_cria_peer_authorized ... ok
test rejeitar_nao_cria_peer ... ok
test trust_state_persistido_so_usa_o_contrato ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

Unidades do crate novo:

```
running 18 tests
test identity::tests::cria_com_permissao_600_e_recarrega_a_mesma_chave ... ok
test identity::tests::arquivo_mais_aberto_que_600_e_recusado ... ok
test identity::tests::debug_nao_expoe_a_chave_privada ... ok
test identity::tests::assinatura_verifica_e_bytes_alterados_nao ... ok
test identity::tests::node_id_e_derivado_da_chave_publica ... ok
test manifest::tests::mesmos_campos_mesmos_bytes_mesma_assinatura ... ok
test manifest::tests::ordem_de_capabilities_nao_muda_a_assinatura ... ok
test manifest::tests::um_byte_alterado_invalida_a_assinatura ... ok
test manifest::tests::node_id_de_outro_no_e_recusado ... ok
test manifest::tests::manifesto_exige_campos_obrigatorios ... ok
test manifest::tests::frescor_recusa_manifesto_velho_e_futuro ... ok
test peer::tests::manifesto_verificado_vira_par_pending ... ok
test peer::tests::pendente_nao_e_autorizado_ate_a_aprovacao ... ok
test peer::tests::revogar_impede_uso_e_exige_nova_aprovacao ... ok
test peer::tests::transicoes_legais_e_idempotentes ... ok
test peer::tests::transicoes_legais_sao_exatamente_as_declaradas ... ok
test peer::tests::revogado_nao_volta_direto_para_autorizado ... ok
test peer::tests::estado_fora_do_contrato_nao_e_lido_como_autorizado ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

O que cada teste de integração **prova no banco** (não só o status):

- assinatura de outra chave, assinatura adulterada e `node_id` alheio → nenhuma linha em `federation_join_request` nem em `federation_peer`;
- ingresso válido → `federation_join_request.state = 'pending'` (1 linha) e **nenhuma** linha em `federation_peer` (consulta SQL: nó pending não é autorizado);
- aprovação → `federation_peer.trust_state = 'authorized'`, `approved_at`/`approved_by` preenchidos, `GET /peers` confirma;
- rejeição → `state='rejected'` e **zero** peer; aprovar depois → 409; pedido inexistente → 404 nas duas decisões;
- replay `(node_id, epoch)` → 200, mesmo `join_request_id`, mesmo `requested_at`, 1 linha; `epoch` novo → 201 e 2 linhas;
- manifesto de 1 h → 422 `manifest_expired`, 0 linhas; o mesmo nó com manifesto recente → 201 (a recusa é de frescor, não de assinatura);
- escopos: sem Bearer 401, token sem escopo 403, `federation:read` não aprova (403) e lista, filtro inválido 422;
- segredo: o hex da semente lida do arquivo `0600` do teste não aparece em **nenhum** corpo de resposta nem em linha de auditoria/log de requisição; a chave pública aparece (é pública);
- `trust_state` persistido só usa o contrato; par revogado recusado com 409 `illegal_trust_transition` e permanece `revoked`.

### 6.2 `cargo clippy --workspace --all-targets`

```
    Checking godmesh-federation v0.1.0 (...)
    Checking godmesh-server v0.1.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.20s
```

Sem erro nem aviso.

### 6.3 Invariantes de banco — `migrations/tests/run_all.sh`

```
===== 0008_invariants.sql =====
NOTICE:  OK 1: approved_at em pedido não aprovado recusado (CHECK)
NOTICE:  OK 2: approved_by em pedido não aprovado recusado (CHECK)
NOTICE:  OK 3: estado inventado em join request recusado (CHECK)
NOTICE:  OK 4: pedido duplicado por (node_id, epoch) recusado (UNIQUE)
NOTICE:  OK 5: trust_state inventado recusado (CHECK)
NOTICE:  OK 6: tabelas de federação guardam apenas chave pública

RESULTADO: OK — 44 invariantes verificados em banco criado do zero.
```

## 7. Limitações e pendências

1. **ADR-071 não existe** no repositório (ver §0). Sem ela, esta fatia tem especificação de tarefa, não decisão registrada.
2. **Sem transporte**: nada de QUIC/mTLS, handshake de par, nem conexão entre nós. `trust_state` ainda não governa nenhum canal (não há canal).
3. **Servidor não tem identidade de nó**: `NodeIdentity`/`GODMESH_NODE_KEY` existem no crate e são testados, mas o processo servidor não carrega a chave nesta fatia. `GET /v1/federation/manifests/current` e `POST /v1/federation/leave` (previstos em `descoberta-ingresso.md`) **não** foram implementados.
4. **Revogação de par não tem rota**: o estado `revoked` é suportado pelo modelo e pelo banco, e a transição é testada, mas nenhuma rota o produz (o teste o escreve direto no banco para exercitar a máquina de estados).
5. **Aprovação não revalida frescor do manifesto**: aprovar um pedido pendente vale mesmo que o manifesto já esteja velho. É decisão administrativa sobre pedido armazenado, não sobre manifesto fresco — declarado, não acidental.
6. `peer_id` é igual a `node_id` nesta fatia; a separação existe para M2 (endpoint/relay).
7. `cargo fmt --check` no workspace **já falhava antes** desta mudança (arquivos antigos fora do formato). Os arquivos novos foram formatados individualmente; o restante não foi reformatado para não poluir o diff.

## 8. Arquivos alterados

Novos:
- `crates/godmesh-federation/{Cargo.toml,src/lib.rs,src/identity.rs,src/manifest.rs,src/peer.rs}`
- `crates/godmesh-server/src/federation.rs`
- `crates/godmesh-server/src/http/federation.rs`
- `crates/godmesh-server/tests/federation.rs`
- `migrations/0008_federation_peers.sql`
- `migrations/tests/0008_invariants.sql`
- `docs/09-equipe/evidencias/2026-11-18-federacao-identidade.md` (este arquivo)

Alterados:
- `Cargo.toml` (membro do workspace), `Cargo.lock`
- `crates/godmesh-server/Cargo.toml` (dependência `godmesh-federation`)
- `crates/godmesh-server/src/lib.rs` (módulo `federation`)
- `crates/godmesh-server/src/auth.rs` (`federation:read`, `federation:admin`, `Scope::ALL` com 10)
- `crates/godmesh-server/src/api.rs` (variantes de erro de federação + `From<sqlx::Error>`)
- `crates/godmesh-server/src/http/mod.rs` (`AppState.join_window`, rotas públicas/protegidas, códigos HTTP)
- `crates/godmesh-server/tests/common/mod.rs` (cliente HTTP de teste reutilizável)
- `migrations/tests/run_all.sh` (0008 aplicada e verificada)