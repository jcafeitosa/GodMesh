# Evidência — Integração da federação: M1 (ingresso) + M2 (transporte) (18/11/2026)

**Branch:** `room/m-int-retry-reconcilia-m1-e-m2-7u6n`
**Ambiente:** macOS arm64; Rust 1.99.1 (`cargo 1.99.1 (5f94df478 2026-08-27)`); Postgres 18.7 em
`postgres://godmesh:godmesh_dev_only@localhost:55432`. Bancos usados foram **descartáveis**
(`godmesh_smoke_int_*`, criado e removido pela própria fumaça) ou o banco de
desenvolvimento `godmesh` **apenas** em `--migrate-only` (idempotente). Nenhum contêiner
(Docker/OrbStack) foi usado. Nenhum push.

**Regras de referência:** ADR-071 + emenda de 18/11/2026 (identidade Ed25519, `node_id` derivado
da chave, QUIC mTLS validado por chave registrada); ADR-072 capa
(`docs/09-equipe/rede-alvo-cto.md`) §2 invariantes 6, 8 e 9, §5 transporte, emenda com ordem
vinculante M1 → M2; ADR-026/ADR-031 (proibido cifra própria); ADR-029 (API/transporte
versionado).

**Estado medido, não presumido:** o que esta execução prova está nas §5 e §6. O que **não**
prova está na §7. Nada aqui autoriza dizer "malha pronta".

---

## 1. O problema que esta fatia resolve

M1 e M2 correram em paralelo, com bases diferentes: M2 saiu de `a71b70d` (antes de M1 existir no
`main`) e, para não travar, **criou o crate `godmesh-federation` inteiro do zero** — identidade,
manifesto e registro próprios. Resultado: dois `NodeIdentity`, dois `SignedManifest`, dois
`PeerRecord` incompatíveis, com formatos de assinatura e de `node_id` diferentes:

| Conceito | M1 (main, canônico) | M2 (paralelo, descartado) |
|---|---|---|
| `node_id` | `gmn_` + hex de `sha256(pubkey)` | hex puro de 64 caracteres da pubkey |
| Tipo do `node_id` | `String` + `is_node_id`/`node_id_for` | newtype `NodeId` |
| Forma assinada do manifesto | TLV explícito, campo com nome + `u32 BE` len | frame binário `GMF1` posicional |
| Envelope de fio | JSON do contrato (`serde`, `deny_unknown_fields`) | frame binário próprio |
| Estado de confiança | `PeerRecord::trust_state` vindo do banco, 4 valores (inclui `unknown`) | declarado **pelo próprio nó** no manifesto, 3 valores |
| Registro de pares | tabela `federation_peer` + rotas HTTP | `RwLock<HashMap<NodeId, PeerRecord>>` em memória |
| `PeerRecord` | 11 campos (inclui `peer_id`, `region`, `capabilities`, `endpoints`) | 6 campos |

Manter os dois lados como estavam produziria duas malhas que não se reconhecem — e a pior falha
possível: um nó "autorizado" numa tabela e invisível no transporte.

## 2. Mapa conceito → arquivo canônico

| Conceito | Arquivo canônico (uma única definição) | Origem |
|---|---|---|
| `NodeIdentity`, `node_id`, assinatura/verificação | `crates/godmesh-federation/src/identity.rs` | **M1**, estendido (§3.1) |
| `SignedManifest` + bytes canônicos + janela de `issued_at` | `crates/godmesh-federation/src/manifest.rs` | **M1**, estendido (§3.2) |
| `PeerRecord`, `TrustState`, máquina de transições, `PeerRegistry` | `crates/godmesh-federation/src/peer.rs` | **M1** (`PeerRecord`/`TrustState`) + **M2** (`PeerRegistry`, movido para cá) |
| Transporte QUIC/mTLS, ALPN, allowlist por chave do SPKI | `crates/godmesh-federation/src/transport.rs` | **M2**, adaptado |
| Handshake de par, códigos `0x474D00xx`, motivo sanitizado | `crates/godmesh-federation/src/handshake.rs` | **M2**, adaptado |
| Estado de enlace (`ready`/`unknown`/`rejected`) | `crates/godmesh-federation/src/link.rs` | **M2**, adaptado |
| Endpoint, aceitação, discagem, heartbeat 5 s × 3 | `crates/godmesh-federation/src/node.rs` | **M2**, adaptado |
| Ingresso por API (join-requests), pares, aprovação | `crates/godmesh-server/src/federation.rs` + `src/http/federation.rs` | **M1**, preservado |
| Diagnóstico de enlace `GET /v1/federation/peers/{node_id}/link` | `crates/godmesh-server/src/http/federation.rs` | **M2**, adaptado |
| Boot: identidade, listener QUIC, allowlist, discagem | `crates/godmesh-server/src/main.rs` | **novo** (§3.4) |
| Esquema `federation_peer` / `federation_join_request` | `migrations/0008_federation_peers.sql` | **M1**, preservado |

`crates/godmesh-federation/src/registry.rs` (M2) **deixou de existir**: `PeerRegistry` foi para
`peer.rs`, ao lado do `PeerRecord` que ele guarda. Um tipo de registro de par, um módulo.

## 3. Como a reconciliação foi feita (decisão a decisão)

Nenhum `--ours`/`--theirs` cego em código: o `--ours` foi usado **só** em `Cargo.lock`, que o
`cargo` resolve depois (não é código). Nas fontes, cada conflito foi resolvido tipo a tipo.

### 3.1 `identity.rs` — M1 vence; M2 acrescenta o que o TLS exige

O `node_id` de M1 (`gmn_` + `sha256`) é o que o banco já grava (`0008`, `CHECK` de formato), é o
que as rotas devolvem e o que o `SignedManifest` assina. Trocar para hex puro de M2 exigiria
migração de dado persistido e quebraria `/v1/federation/peers` — mudança incompatível sem
versionamento, proibida pelo contrato do repositório. **M1 é o canônico.**

O que M2 precisava e M1 não tinha:

| Adição | Por quê |
|---|---|
| `public_key_bytes() -> [u8; 32]` | comparar a chave do certificado TLS (SPKI) com a chave do manifesto sem passar por hex |
| `pkcs8_der()` | `rcgen` emite o certificado autoemitido com a **mesma** chave da identidade |
| `PUBLIC_KEY_LEN`, `SIGNATURE_LEN` | dimensões usadas pelo parsing do SPKI e dos códigos de fio |
| `IdentityError::KeyExport` | falha de export do PKCS#8 não pode virar identidade duvidosa |
| `from_env_with(lookup)` | o boot do servidor precisa ser testável sem tocar `std::env` (global e concorrente entre testes) |

**O que M2 tinha e não entrou:** `NodeIdentity::from_seed([u8;32])` público. M1 deliberadamente
não expõe escolha da semente (o chamador não escolhe o byte secreto). Os testes de M2 que usavam
semente fixa foram reescritos com `generate_in_memory()` — a asserção testada é a mesma
(identidade distinta por nó), sem abrir uma API que permite forjar chave a partir de constante.

### 3.2 `manifest.rs` — M1 canônico, estendido com `alpn`

O `Manifest`/`SignedManifest` do M2 (frame `GMF1` posicional) foi **descartado**: o mesmo
conceito já tinha forma canônica definida e testada em M1, e o ingresso por API depende dela. O
que era **campo a mais** em M2 foi avaliado um por um:

| Campo de M2 | Decisão | Fundamento |
|---|---|---|
| `epoch` | **já existia em M1** | `SignedManifest::epoch` e `PeerRecord::epoch`; a checagem `StaleEpoch` de M2 foi preservada |
| `alpn` | **estendido em M1** (`#[serde(default)]`) | versão de protocolo no conteúdo **assinado**, não só no handshake; `serde(default)` mantém manifesto de ingresso antigo legível |
| `trust_state` | **removido** | ver §3.3 |
| `node_id: NodeId`, `public_key: [u8;32]` | **não entram** | `String` (`gmn_…`, hex) é o formato do banco e do contrato JSON |
| `to_bytes`/`from_bytes` (frame binário) | **substituído** por `to_frame`/`from_frame` (JSON do contrato, teto `MAX_MANIFEST_FRAME_BYTES` = 8 KiB) | um formato para o mesmo dado; o frame não é a forma assinada e não precisa de um segundo codec com as mesmas regras duplicadas |
| `ManifestError::TooLarge/Truncated/BadMagic/BadField/BadTrustState` | **substituídos** | `FrameTooLarge`, `FrameMalformed` e as variantes de M1 cobrem os mesmos desfechos no `map_manifest_error` |

Testes de M2 portados para o tipo canônico (`manifest.rs` e `tests/quic.rs`): round-trip de frame,
frame acima do teto recusado **antes** do parse, campo desconhecido recusado, `alpn` coberto pela
assinatura, manifesto sem `alpn` ainda legível.

### 3.3 `trust_state` saiu do manifesto: contrato decide

M2 exigia `trust_state` no manifesto e comparava com o registro (`Authorized/Authorized` → ok;
`Revoked` → `0x474D0007`; `Pending` → `0x474D0006`). A comparação de **estado** permanece; o que
saiu foi a **declaração do próprio nó**.

Motivo, citando o contrato:

- ADR-071 emenda §1: "A autoridade de ingresso é de **nó**, não de token solto" e §5 "Bootstrap
  não é confiança automática… nada de *trust on first use*". Um nó que declara `authorized` no
  próprio manifesto não é evidência de nada: o manifesto prova **posse da chave** e **frescor**,
  não confiança. Confiança é decisão administrativa persistida (`federation_peer`), e o registro
  local é a autoridade — como o próprio comentário de M2 já dizia ("O registro é a autoridade; o
  manifesto é a declaração").
- Manter o campo daria uma superfície falsa: um nó `pending` no banco mas que escrevesse
  `authorized` no manifesto seria recusado de qualquer forma (registro vence), então o campo só
  servia para produzir um código de recusa diferente. A recusa honesta é uma: o registro.

Consequência preservada: os dois códigos continuam existindo e testados —
`0x474D0006` (`NotAuthorized`) para `pending`/`unknown` e `0x474D0007` (`Revoked`) para revogado,
agora decididos **só** pelo registro. `TrustState::Unknown` (que M2 não tinha) entra no `match`
como `NotAuthorized`, nunca como autorizado.

### 3.4 `registry.rs` → `peer.rs`: registro em memória sobre o `PeerRecord` canônico

`PeerRegistry` é conceito diferente de `PeerRecord` (allowlist de runtime vs. registro de par),
então não foi duplicação — mas ele **guardava um segundo `PeerRecord`**. Agora guarda o de M1,
`HashMap<String, PeerRecord>`, e:

- `insert`/`replace` preservam `last_seen_at` (evidência de vida não se perde em recarga);
- `replace` **substitui** o conjunto pelo snapshot do banco (par que deixou de estar autorizado
  sai da allowlist) — é o que a recarga periódica usa;
- `set_trust_state`/`revoke` passam pela **máquina canônica** `PeerRecord::transition_to`:
  `revoked → authorized` direto devolve `false` em vez de contornar M1 (o caminho de volta é
  `request_rejoin` → `pending` → nova decisão);
- `knows_key(&[u8;32])` decide por **chave**, nunca por nome/SAN/validade do certificado
  (ADR-072 §2, invariante 8: "Endereço IP não é permissão; chave é");
- `touch` não muda `trust_state` (ADR-072 §2, invariante 6).

`PeerRecord::from_manifest` (M1) continua sendo o caminho do ingresso por API;
`authorized_from_key` é o caminho do runtime/teste, para um registro cuja autorização **já** foi
decidida em outro lugar (banco ou bootstrap). Construir não é promover: toda transição posterior
passa pela máquina.

### 3.5 `handshake.rs`: mesma ordem de verificação, tipos canônicos

Ordem preservada de M2, adaptada ao verificador de M1:

1. frame bem formado e ≤ 8 KiB (`from_frame`);
2. campos, ligação `node_id`↔chave (`sha256` recomputado), assinatura e janela de `issued_at`
   (`verify_fresh` — **um** caminho de verificação para o ingresso e para o transporte);
3. `cluster` do manifesto == local e == registrado;
4. ALPN negociado é `godmesh/1` **e** é o declarado no manifesto assinado;
5. chave do manifesto == chave do certificado TLS (SPKI), por `ec25519` apenas;
6. chave registrada, `cluster` do registro igual, `trust_state` do **registro** `authorized`;
7. `epoch` do manifesto ≥ `epoch` do registro.

Códigos de fechamento `0x474D0001..0x474D000C` inalterados; motivo sempre string fixa do crate.
`SignedManifest::verify` falhando em `node_id` de outra chave é `0x474D0003`
(`NodeIdMismatch`) — novo teste `node_id_que_nao_deriva_da_chave_e_recusado` prova que a ligação
é **recomputada** na recepção, não aceita.

### 3.6 Boot do servidor (novo, exigido pela tarefa)

`crates/godmesh-server/src/main.rs`, em `federation_runtime_from(lookup)`:

| Condição | Comportamento |
|---|---|
| `GODMESH_NODE_KEY` ausente/vazia | **exit 78**, mensagem citando a variável |
| arquivo de chave com permissão ≠ `0600` | **exit 78** (`IdentityError::InsecureMode`), falha fechada |
| arquivo de chave ausente | criado `0600` (primeiro boot de um nó novo; `O_EXCL`, sem sobrescrever) |
| `GODMESH_FEDERATION_BIND` ausente | transporte **desligado** (default); `/link` responde `unknown` — piso honesto |
| `GODMESH_FEDERATION_BIND` presente | abre listener QUIC; exige `GODMESH_FEDERATION_CLUSTER` e `_REGION` (sem default silencioso) |
| allowlist de pares | carregada de `federation_peer` no boot e **recarregada** a cada `GODMESH_FEDERATION_PEERS_REFRESH_MS` (default 5000) |

Além do listener, esta fatia fecha um buraco que o M2 deixou: **nada chamava `dial` em produção**
— o nó só escutava, e a malha nunca fecharia handshake. `spawn_peers_maintenance` faz, a cada
ciclo, (1) recarga da allowlist e (2) discagem para cada par `authorized` **sem** enlace `ready`,
usando os `endpoints` `quic://IP:porta` que o par declarou. Falha de discagem não promove nada: só
handshake verificado marca `ready`.

`/healthz` e o shutdown gracioso não mudaram de contrato: o mesmo `watch` de término alimenta HTTP,
publicador e manutenção de pares; `FederationNode::shutdown()` fecha o endpoint e aborta as tarefas
depois de o HTTP drenar. `deploy/Dockerfile` e `deploy/README.md` foram atualizados (nova variável
obrigatória e volume para a identidade) para não quebrar deploy em silêncio.

## 4. Dependências — versões resolvidas

Resolvidas em 18/11/2026 (crates.io) e fixadas no `Cargo.lock`; nenhuma atualização foi feita
nesta fatia, apenas a **união** das listas de M1 e M2.

| Crate | Versão | Papel | Fonte |
|---|---|---|---|
| `quinn` | 0.11.12 | QUIC (RFC 9000) | ADR-071 / M2 |
| `quinn-proto` | 0.11.19 | transitiva | `Cargo.lock` |
| `quinn-udp` | 0.5.16 | transitiva | `Cargo.lock` |
| `rustls` | 0.23.46 | TLS 1.3 (provider `ring`) | ADR-071 / M2 |
| `ring` | 0.17.14 | primitivas TLS | transitiva |
| `ed25519-dalek` | 3.0.0 (`alloc`, `pkcs8`) | assinatura de nó | ADR-071 |
| `curve25519-dalek` | 5.0.0 | transitiva | `Cargo.lock` |
| `rcgen` | 0.14.10 | certificado autoemitido do nó | ADR-071 |
| `x509-parser` | 0.18.1 | **só** lê o SPKI do certificado | M2 |
| `sha2` | 0.11.1 | `node_id` = `sha256(pubkey)` | M1 |
| `serde_json` | 1.0.154 | frame do manifesto | M1 (JSON do contrato) |
| `tokio` | 1.54.1 | runtime das tarefas do nó | M2 |

`quinn`/`rustls` continuam com `default-features = false` (sem `platform-verifier`): a validação
é por chave registrada, e carregar um verificador de CA público sem uso seria superfície sem
função. `unsafe_code = "forbid"` no workspace vale para o crate.

## 5. Verificação — saídas literais

### 5.1 `cargo test --workspace`

```
$ cargo test --workspace 2>&1 | grep -E "^test result" | <soma>
total passed=306 failed=0 ignored=1
```

`1 ignored` é pré-existente e alheio a esta fatia
(`colombian_tenant_without_policy_is_refused_with_retention_policy_missing ... ignored, altera o
registro de política de CO; roda isolado com --ignored`). **306 ≥ 263** exigidos, **0 falha**.
Suítes de federação: `godmesh-federation` 39 unitários + `tests/quic.rs` 10 +
`godmesh-server tests/federation_link.rs` 5 + `--bin godmesh-server` 15 (identidade/boot) +
`godmesh-server tests/federation.rs` (M1, inalterado).

### 5.2 `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets 2>&1 | tail -3
    Checking godmesh-server v0.1.0 (.../crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s
```

Nenhum erro e nenhum warning (`all = deny`, priority -1, no workspace). O único achado no caminho
foi `clippy::large_enum_variant` no `Mode` do binário, resolvido com `Box<FederationRuntime>` —
não com `#[allow]`.

### 5.3 Invariantes de banco (inclui `0008`)

```
$ PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only bash migrations/tests/run_all.sh
...
===== 0008_invariants.sql =====
NOTICE:  OK 1: approved_at em pedido não aprovado recusado (CHECK)
NOTICE:  OK 2: approved_by em pedido não aprovado recusado (CHECK)
NOTICE:  OK 3: estado inventado em join request recusado (CHECK)
NOTICE:  OK 4: pedido duplicado por (node_id, epoch) recusado (UNIQUE)
NOTICE:  OK 5: trust_state inventado recusado (CHECK)
NOTICE:  OK 6: tabelas de federação guardam apenas chave pública
RESULTADO: OK — 44 invariantes verificados em banco criado do zero.
```

### 5.4 `--migrate-only` no ambiente de desenvolvimento

```
$ DATABASE_URL="postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh" GODMESH_ENV=dev \
    ./target/debug/godmesh-server --migrate-only; echo "EXIT=$?"
godmesh-server: migrações em migrations — 0 aplicada(s) [], 8 já aplicada(s) [0001_core.sql,
0002_incident_tenancy.sql, 0003_outbox_publisher.sql, 0004_grant_revocation.sql,
0005_sync_inbox.sql, 0006_incident_listing.sql, 0007_grant_issuance.sql,
0008_federation_peers.sql]
EXIT=0
```

### 5.5 Fumaça real: dois processos, handshake de par, `ready` → `unknown`

Script: `target/smoke-federation.sh` (artefato de execução, não versionado — o resultado está
aqui). Banco descartável criado e removido pelo próprio script; dois processos do **binário** em
portas distintas, cada um com seu `GODMESH_NODE_KEY`.

```
=== boot ===
a.log:godmesh-server: escutando em http://127.0.0.1:18091 (1 grant(s) configurado(s))
a.log:godmesh-server: identidade de nó carregada: node_id=gmn_db38e920306236ced4597d92b42cf5ff84e68a6d2806a1c998538e838c0057f4 public_key=90309eb8fed06ae44e2045417c3aea599671d91ae664414cae8ed0a13cf45b1d
a.log:godmesh-server: federação escutando em quic://127.0.0.1:17441 (0 par(es) no registro, ALPN godmesh/1)
b.log:godmesh-server: escutando em http://127.0.0.1:18092 (1 grant(s) configurado(s))
b.log:godmesh-server: identidade de nó carregada: node_id=gmn_4035a7f5434bd1b6c99406bf4d1173a3d8b8e9c31e2132b4a4808e2de88ede3c public_key=b3141a1e1ed5e4cf67f8481859aadb4152e8f875bd4c96df3d7318bee15c8b50
b.log:godmesh-server: federação escutando em quic://127.0.0.1:17442 (0 par(es) no registro, ALPN godmesh/1)
=== pares registrados no banco ===
                               node_id                                | trust_state |        endpoints
----------------------------------------------------------------------+-------------+--------------------------
 gmn_4035a7f5434bd1b6c99406bf4d1173a3d8b8e9c31e2132b4a4808e2de88ede3c | authorized  | {quic://127.0.0.1:17442}
 gmn_db38e920306236ced4597d92b42cf5ff84e68a6d2806a1c998538e838c0057f4 | authorized  | {quic://127.0.0.1:17441}
(2 rows)

=== enlace A -> B antes da discagem ===
{"state":"unknown","last_seen_at":null,"rtt_ms":null,"negotiated_alpn":null,"peer_version":null}
=== enlace A -> B (handshake de par concluído) ===
{"state":"ready","last_seen_at":1795023913,"rtt_ms":2,"negotiated_alpn":"godmesh/1","peer_version":"0.1.0"}
=== enlace B -> A (mTLS nos dois sentidos) ===
{"state":"ready","last_seen_at":1795023913,"rtt_ms":2,"negotiated_alpn":"godmesh/1","peer_version":"0.1.0"}
OK: enlace A->B ready
OK: enlace B->A ready
=== /healthz de A e B (não regride com a federação ligada) ===
A /healthz -> 200
B /healthz -> 200
=== matando B (pid 58138) ===
=== enlace A -> B depois da morte de B ===
{"state":"unknown","last_seen_at":1795023913,"rtt_ms":null,"negotiated_alpn":"godmesh/1","peer_version":"0.1.0"}
OK: enlace virou unknown (nunca ready por passagem de tempo)
=== shutdown gracioso de A (SIGTERM) ===
godmesh-server: shutdown solicitado; 0 requisição(ões) em voo drenada(s)
godmesh-server: encerrado
FUMACA OK
```

Leitura: `unknown` antes de qualquer evidência (nada de default verde); `ready` **só** depois de
handshake verificado nos dois sentidos, com `rtt_ms` medido e `peer_version` vindo do manifesto
assinado; morte do par leva a `unknown` mantendo `last_seen_at` como **histórico** e zerando
`rtt_ms` (medição de enlace morto não é medição); `/healthz` continua 200 com a federação ativa;
SIGTERM drena e encerra.

### 5.6 Falha alta do boot (verificado por teste, não por inspeção)

`--bin godmesh-server`, 15 testes; entre eles:

```
test tests::fed_sem_godmesh_node_key_falha_citando_a_variavel ... ok
test tests::fed_com_arquivo_de_chave_aberto_falha_fechado ... ok
test tests::fed_desligada_por_default_e_carrega_a_identidade ... ok
test tests::fed_com_bind_invalida_falha_citando_a_variavel ... ok
test tests::fed_com_bind_exige_cluster_e_regiao ... ok
test tests::fed_com_bind_completa_liga_o_listener ... ok
test tests::endpoint_do_manifesto_so_vira_socket_em_quic ... ok
```

Saída literal do binário sem `GODMESH_NODE_KEY`:

```
$ DATABASE_URL=... GODMESH_ENV=dev BIND_ADDR=127.0.0.1:18091 GODMESH_GRANTS='{"grants":[]}' \
    ./target/debug/godmesh-server
godmesh-server: configuração inválida: identidade de nó indisponível: GODMESH_NODE_KEY não
definida; o nó não tem onde persistir a identidade
```

## 6. Testes preservados e adaptados (nenhum apagado por conveniência)

| Origem | Arquivo | Destino |
|---|---|---|
| M1 | `federation/src/{identity,manifest,peer}.rs` (unit) | mesmos testes, +9 novos (§3.1, §3.2, §3.4) |
| M1 | `godmesh-server/tests/federation.rs` | mantido; único ajuste: literal de `SignedManifest` ganhou `alpn` |
| M2 | `federation/tests/quic.rs` (8 testes) | 8 mantidos + 1 novo (`node_id_que_nao_deriva_da_chave`) + 1 de formato; reescritos sem `from_seed` |
| M2 | `godmesh-server/tests/federation_link.rs` (5) | mantidos; `node_id` canônico `gmn_…`, 1 asserção ampliada (3 formas inválidas) |
| M2 | `federation/src/{manifest,registry,identity,transport,link}.rs` (unit) | portados para os tipos canônicos; nenhum comportamento perdido (tabela §3.2 e §3.4) |

Onde duas asserções se contradiziam (o `trust_state` declarado no manifesto de M2 vs. "o registro
é a autoridade" de M1), o contrato decidiu — ADR-071 emenda §1/§5 e ADR-072 §2 invariantes 6/8 —
e a decisão está justificada na §3.3.

## 7. O que **não** foi provado

Declarado, não maquiado:

- **NAT e buraco de NAT** — a fumaça é loopback (`127.0.0.1`). Nenhum nó atrás de NAT foi testado;
  QUIC sobre NAT real (rebind de porta, timeout de mapeamento) segue não medido.
- **Relay, hole punching e descoberta** — não implementados nesta fatia. Nó atrás de NAT simétrico
  **não** entra na malha hoje; ADR-071 deixa relay para fatia posterior (M5/borda).
- **WAN e RTT real** — o `rtt_ms: 2` é loopback. Nenhuma medição em rede de longa distância,
  nenhuma perda ou reordenação real.
- **Migração de conexão (connection migration / `preferred_address`)** — não exercitada. Troca de
  caminho (Wi-Fi → celular) não foi testada; o `keep_alive` de 2 s e `max_idle_timeout` de 15 s
  estão configurados, mas nenhum teste prova comportamento de migração.
- **Perda de pacote, jitter e MTU** — nada simulado. Comportamento sob perda e sob `netem` não
  verificado.
- **Handshake de aprovação assinada por nó** (ADR-071 emenda §2) — a aprovação de ingresso ainda é
  por grant humano (`federation:admin`) com escopo; assinatura de aprovação por nó autorizado é
  **M1b**, não esta fatia.
- **Propagação de revogação entre nós** — a revogação vale localmente (allowlist recarregada do
  banco). Nenhum teste prova um nó aprendendo revogação de outro nó.
- **Multi-cluster / multi-região** — só `cluster-br-sul`/`br-sul`; recusa de cluster divergente
  está testada, mas federação entre clusters distintos não.
- **Carga, concorrência e DoS** — nenhum teste de muitos pares simultâneos, de reconexão em
  rajada, nem de exaustão por handshake abortado. Bound conhecido: teto de 8 KiB por frame e
  `max_idle_timeout` 15 s; nada medido sob carga.
- **Fumaça com os **dois** processos registrando-se pela rota pública de ingresso** — nesta
  execução o par foi registrado por SQL direto no `federation_peer` do banco descartável (a rota
  exige manifesto assinado por um cliente com a chave do nó, que o script não fabrica). A rota de
  ingresso está coberta por `godmesh-server/tests/federation.rs` (M1), mas o caminho
  ponta-a-ponta "join-request assinado → aprovação → transporte" **não** foi exercitado num único
  teste.
- **Windows/Linux** — só macOS arm64.

## 8. Limitações conhecidas da implementação

1. **Discagem sem backoff exponencial**: o ciclo de manutenção tenta de novo a cada
   `GODMESH_FEDERATION_PEERS_REFRESH_MS` (default 5 s) enquanto o enlace não estiver `ready`, e
   registra cada falha em stderr. É simples e observável; não é o retry com jitter que uma rede
   grande pede.
2. **`endpoints` vem do manifesto do par** (declaração, não permissão). Um par pode declarar
   endereço de terceiro; a autorização continua sendo a **chave** — o endereço só decide *para
   onde* discar.
3. **Recarga da allowlist é por polling do banco** (mesmo intervalo). Revogação chega em até um
   intervalo; não há push nem propagação entre nós (§7).
4. **`node_id` público no log de boot** (junto da `public_key`): ambos são dado público e são o que
   o registro do outro nó precisa. A semente nunca é impressa — há teste que falha se aparecer.

## 9. Rastreabilidade

requisito (ADR-071 + emenda, ADR-072 §2/§5 + emenda M1→M5) → decisão de reconciliação (§3) →
arquivo canônico (§2) → teste (§5.1, §5.5, §5.6) → evidência (§5) → limites (§7, §8).

Nada nesta fatia altera o esquema persistido: `0008` e o formato de `node_id`/`public_key` são
exatamente os de M1, e por isso não há migração de dado.