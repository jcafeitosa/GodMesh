# Evidência — Federação: transporte QUIC/mTLS e handshake de par (18/11/2026)

**Branch:** `room/m2-federacao-transporte-quic-mtls-7ry3`
**Ambiente:** macOS arm64; Rust 1.99.1 (`cargo 1.99.1 (5f94df478 2026-08-27)`); Postgres 18.7 em
`postgres://godmesh:godmesh_dev_only@localhost:55432` — banco **próprio desta execução**
(`godmesh_m2_10759`), migrado pelo runner oficial (`godmesh_server::migrate::migrate`), **sem
contêiner**. Nenhum `DROP`/`TRUNCATE` no banco compartilhado `godmesh`.

**Regras de referência:** ADR-071 + emenda de 18/11/2026 (identidade Ed25519, QUIC mTLS,
certificado validado por chave registrada, aprovação assinada por nó autorizado); ADR-072 capa
(`docs/09-equipe/rede-alvo-cto.md`) §2 invariantes 6 e 8, §5 transporte; ADR-026/ADR-031
(proibido cifra própria); `docs/01-arquitetura/descoberta-ingresso.md`.

**Estado medido, não presumido:** o que este documento prova está na §4; o que **não** prova
está na §7. Nada aqui autoriza dizer "malha pronta".

> **Correção posterior (18/11/2026):** a §0 abaixo descreve a base **histórica** desta execução
> (fatia M1 ainda fora do `main`). M1 entrou no `main` e a reconciliação foi feita: este crate
> passou a ter **uma** definição de `NodeIdentity`, `SignedManifest` e `PeerRecord`, a de M1, com o
> transporte adaptado a ela. `identity.rs`/`manifest.rs`/`registry.rs` descritos aqui como "mínimo
> da fatia M1" **não existem mais** nessa forma (`registry.rs` foi absorvido por `peer.rs`), e o
> `trust_state` deixou de ser declarado no manifesto. Os números de teste e as versões deste
> documento são os daquela execução; a prova atualizada está em
> [`2026-11-18-integracao-federacao.md`](2026-11-18-integracao-federacao.md). O que este documento
> continua provando — handshake com códigos `0x474D00xx`, allowlist por chave, heartbeat que
> degrada para `unknown` — foi reexecutado na integração.

---

## 0. Divergência de fatia declarada (M1 ausente no `main`)

O pré-requisito era o crate `godmesh-federation` da fatia M1 (identidade, manifesto assinado,
registro de pares). **Ele não existe no `main`** no commit base desta branch:

```
$ git log --oneline -5
a71b70d docs(adr): ADR-072 adota a rede-alvo de cinco planos como direcao aprovada
1c4e454 docs(arquitetura): proposta de rede-alvo do CTO
2c6de4e docs(adr): emenda a ADR-071 - nos autorizados assinam ingresso de outros nos
1b53d06 feat(deploy): empacota servidor 1.0.0 e corrige healthcheck para o /healthz que consulta o pool
67e2a2c docs(evidencia): registro da integracao dos grants e da migracao 0007

$ ls crates/
godmesh-core  godmesh-emergency  godmesh-identity  godmesh-policy
godmesh-protocol  godmesh-server  godmesh-sync
```

Como a fatia M2 exige o mínimo de M1 para existir, o mínimo foi implementado **dentro deste
crate e deste branch**: `identity.rs` (par Ed25519, `node_id` = hex da chave),
`manifest.rs` (`SignedManifest` canônico + verificação) e `registry.rs` (allowlist de pares,
`trust_state`, `epoch`). São deliberadamente pequenos e sem `join_certificate`,
`join_request` nem aprovação assinada por nó — isso continua sendo M1/M1b. **Se M1 chegar com
outro formato de `SignedManifest`, este branch precisa ser reconciliado antes do merge**; os
formatos de assinatura estão documentados na §3.1 exatamente para permitir essa reconciliação.

---

## 1. Dependências (versão resolvida, fonte e data)

Consulta ao índice do crates.io em **18/11/2026** (`cargo info <crate>`); todas são a versão
**latest estável** no momento da consulta:

| Crate | Resolvida | Última no registry (18/11/2026) | Papel | ADR |
|---|---|---|---|---|
| `quinn` | 0.11.12 | 0.11.12 | QUIC (RFC 9000) | ADR-071 |
| `rustls` | 0.23.46 | 0.23.46 (o `0.24.0-dev.3` é pré-release, não usado) | TLS 1.3 | ADR-071 |
| `ed25519-dalek` | 3.0.0 | 3.0.0 | assinatura de identidade/manifesto | ADR-071 |
| `rcgen` | 0.14.10 | 0.14.10 | certificado autoemitido por nó | ADR-071 |
| `x509-parser` | 0.18.1 | 0.18.1 | **só leitura** do `SubjectPublicKeyInfo` | — (ver nota) |

Transitivas relevantes resolvidas no `Cargo.lock`: `quinn-proto` 0.11.19, `quinn-udp` 0.5.16,
`ring` 0.17.14, `asn1-rs` 0.7.2, `der-parser` 10.0.0, `oid-registry` 0.8.1.

**Nota sobre `x509-parser` (dependência nova fora da ADR-071):** o verificador precisa extrair a
chave pública do certificado que o par apresenta. `rustls`/`webpki` verificam cadeia, não expõem
`SPKI`; reimplementar parsing DER à mão seria pior. `x509-parser` **não faz criptografia** —
lê a estrutura e devolve os 32 bytes de chave, que então são comparados com a allowlist, e a
prova de posse continua sendo o `CertificateVerify` verificado por
`rustls::crypto::verify_tls13_signature` (provider `ring`). Nenhuma primitiva foi escrita neste
repositório (ADR-026/ADR-031).

`rustls` foi configurado **sem** a feature `tls12` e sem `aws-lc-rs`: só TLS 1.3 com `ring`.
`quinn` foi configurado com `default-features = false` (sem `platform-verifier`, que traria
validação por CA do sistema — exatamente o que este desenho **não** usa).

---

## 2. Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `Cargo.toml` | `crates/godmesh-federation` entra no workspace |
| `crates/godmesh-federation/Cargo.toml` | **novo** — deps da §1 |
| `crates/godmesh-federation/src/lib.rs` | **novo** — escopo da fatia e `now_unix()` |
| `crates/godmesh-federation/src/identity.rs` | **novo** — `NodeId`/`NodeIdentity` Ed25519, `pkcs8_der`, segredo redigido em `Debug` |
| `crates/godmesh-federation/src/manifest.rs` | **novo** — `Manifest`/`SignedManifest`, codificação canônica binária, `verify` (assinatura, vínculo node_id↔chave, cluster, ALPN, janela de `issued_at`) |
| `crates/godmesh-federation/src/registry.rs` | **novo** — `PeerRegistry`/`PeerRecord`, `knows_key`, revogação com `epoch` que não regride |
| `crates/godmesh-federation/src/transport.rs` | **novo** — certificado autoemitido, `server_config`/`client_config` (mTLS obrigatório, ALPN `godmesh/1`, TLS 1.3), verificador de certificado por allowlist de chaves, `peer_public_key`, `negotiated_alpn` |
| `crates/godmesh-federation/src/handshake.rs` | **novo** — `SignedManifest` em stream dedicado nos dois sentidos, verificação de 9 passos, `Rejection` com códigos estáveis e motivo sanitizado |
| `crates/godmesh-federation/src/link.rs` | **novo** — `LinkState` (`ready`/`unknown`/`rejected`), `LinkSnapshot`, expiração para `unknown` |
| `crates/godmesh-federation/src/node.rs` | **novo** — `FederationConfig` (heartbeat padrão 5 s × 3), `FederationNode::bind/dial`, laço de aceitação, heartbeat envio/recepção, vigia de fechamento |
| `crates/godmesh-federation/tests/quic.rs` | **novo** — 8 testes de fio entre dois nós reais |
| `crates/godmesh-server/Cargo.toml` | dependência `godmesh-federation` |
| `crates/godmesh-server/src/auth.rs` | escopo novo `federation:read` (`ALL` passa de 8 para 9) |
| `crates/godmesh-server/src/http/mod.rs` | `AppState.federation: Option<Arc<PeerLinks>>`, `router_with_federation`, rota de federação no grupo protegido |
| `crates/godmesh-server/src/http/federation.rs` | **novo** — `GET /v1/federation/peers/{node_id}/link` |
| `crates/godmesh-server/tests/federation_link.rs` | **novo** — 5 testes HTTP (sem banco) |

Nenhum teste existente foi alterado ou enfraquecido. Nenhuma migração nova.

---

## 3. Desenho entregue

### 3.1 Identidade e manifesto (mínimo de M1)

- **Identidade:** par **Ed25519**. `node_id` = **hex minúsculo da chave pública** (64 chars). O
  identificador *é* a chave: não existe tabela de nomes para envenenar.
- **Manifesto** (conteúdo assinado): `node_id`, `public_key` (32 B), `cluster`,
  `software_version`, `alpn`, `trust_state` (`pending|authorized|revoked`), `issued_at` (Unix
  seg.), `epoch`.
- **Codificação canônica binária, não JSON:** `GMF1` + `trust_state` (1 B) + `issued_at`
  (u64 BE) + `epoch` (u64 BE) + 4 campos de texto prefixados por comprimento (u32 BE) + 32 B de
  chave; a assinatura de 64 B vai ao final do frame. Motivo: assinatura de JSON depende de ordem
  de campo e escape de serializador — cada um deles é uma chance de emissor e verificador
  divergirem.
- Teto de frame: 8 KiB (recusa **antes** de alocar). Campo de texto: ≤ 128 B, ASCII imprimível.
  Byte sobrando no frame é recusa, não tolerância.

### 3.2 Transporte

- QUIC (`quinn`) sobre **TLS 1.3 apenas**; `rustls` com provider `ring`.
- **ALPN fixo `godmesh/1`**. Versão diferente é recusada, e o ALPN declarado no manifesto
  assinado precisa ser o mesmo do handshake.
- **mTLS obrigatório**: `ClientCertVerifier::client_auth_mandatory` = `true` e o cliente também
  verifica o servidor.
- **Certificado autoemitido** (`rcgen`), assinado com a **mesma** chave Ed25519 da identidade
  (PKCS#8 exportado da identidade → `rcgen`). CN vazio, SAN constante e cosmético.
- **Validação por chave registrada:** o verificador extrai o `SubjectPublicKeyInfo`, exige
  algoritmo **Ed25519** (OID 1.3.101.112) e exige que a chave esteja na allowlist. **Nome/CN,
  SAN, cadeia, CA pública, data de validade e SNI/DNS são ignorados de propósito** (invariante 8:
  endereço IP não é permissão, chave é). `supported_verify_schemes` devolve só `ED25519`.
- Nome de conexão usado pelo cliente é a constante `godmesh-peer.invalid` e não carrega
  autoridade — o verificador não o lê.
- TLS 1.2 não é oferecido; o método `verify_tls12_signature` existe no trait e **nega**.

### 3.3 Handshake de par (stream dedicado)

Um stream bidirecional por conexão, aberto pelo iniciador:

```
iniciador                                  aceitador
  open_bi()                                 accept_bi()
  write(manifest_assinado) + finish         read(manifest)          → verifica (1..9)
                                            write(manifest_assinado) + finish
  read(manifest) → verifica (1..9)
```

Verificação, na ordem (falha = `close(código, motivo sanitizado)`):

| # | Verificação | Recusa |
|---|---|---|
| 1 | frame bem formado, ≤ 8 KiB | `BadFrame` |
| 2 | assinatura Ed25519 confere com a chave declarada | `SignatureInvalid` |
| 3 | `node_id` == hex(chave declarada) | `NodeIdMismatch` |
| 4 | chave do manifesto == chave do certificado TLS | `KeyMismatch` |
| 5 | chave registrada no registro local | `UnknownKey` |
| 6 | `cluster` do registro == `cluster` do manifesto | `ClusterMismatch` |
| 7 | `trust_state` (registro **e** manifesto) == `authorized` | `NotAuthorized` / `Revoked` |
| 8 | `epoch` do manifesto ≥ `epoch` registrado | `StaleEpoch` |
| 9 | `issued_at` dentro da janela (±60 s padrão) | `StaleManifest` |

O passo 4 fecha a falha clássica de identidade: sem ele, um nó **autorizado** A poderia abrir TLS
com a própria chave e apresentar um manifesto assinado por B, aparecendo como B no registro do
outro lado. Está coberto por teste (`chave_do_manifesto_difere_do_certificado_tls`).

### 3.4 Códigos de erro (faixa `0x474D00xx`, estável)

| Constante | Código | `code()` | Motivo (reason no fio) |
|---|---|---|---|
| `BadFrame` | `0x474D0001` | 1196228609 | `manifesto ilegivel` |
| `SignatureInvalid` | `0x474D0002` | 1196228610 | `assinatura invalida` |
| `NodeIdMismatch` | `0x474D0003` | 1196228611 | `node_id nao corresponde a chave` |
| `KeyMismatch` | `0x474D0004` | 1196228612 | `chave do manifesto difere da chave TLS` |
| `UnknownKey` | `0x474D0005` | 1196228613 | `chave nao registrada` |
| `NotAuthorized` | `0x474D0006` | 1196228614 | `par pendente de autorizacao` |
| `Revoked` | `0x474D0007` | 1196228615 | `par revogado` |
| `ClusterMismatch` | `0x474D0008` | 1196228616 | `cluster diferente` |
| `StaleManifest` | `0x474D0009` | 1196228617 | `manifesto fora da janela` |
| `StaleEpoch` | `0x474D000A` | 1196228618 | `epoch do manifesto obsoleto` |
| `AlpnMismatch` | `0x474D000B` | 1196228619 | `alpn diferente` |
| `Transport` | `0x474D000C` | 1196228620 | `handshake incompleto` |

Motivo é **string fixa do crate**, nunca dado do par: log e alerta não carregam conteúdo
controlado pelo outro lado (regra de "nenhum segredo em log" vale também para conteúdo hostil).
Quem recebe a recusa reconstrói o motivo pelo código (`Rejection::from_code`), em vez de reportar
"frame ilegível" — que seria o erro de quem *recebe* a recusa, não o motivo dela.

Observação de desenho: TLS aceita a conexão de uma chave **registrada** mesmo `pending`/`revoked`
e é o handshake que a fecha com código explícito. Assim o par aprende *por quê*, e nenhum stream
de dados é aberto antes da decisão. Chave **não registrada** para no TLS (nenhum handshake de
aplicação acontece).

### 3.5 Heartbeat

- Frame de 12 B em stream unidirecional: `GMH1` + contador u64 BE.
- Intervalo **padrão 5 s** (`DEFAULT_HEARTBEAT_INTERVAL`); silêncio tolerado **3 intervalos
  (15 s)** (`DEFAULT_MISSED_HEARTBEATS`) — configurável em `FederationConfig`.
- Cada heartbeat aceito atualiza `last_seen_at` no `PeerLinks` e no `PeerRegistry`.
- Ausência > 3 × intervalo → estado `unknown` e `rtt_ms` zerado. **Nunca** promovido a `ready`
  por passagem de tempo (invariante 6): só handshake verificado marca `ready`, e `beat()` não
  promove nada.
- Se o registro revogar o par depois do handshake, o próximo heartbeat fecha a conexão com
  `Revoked` (`0x474D0007`) — revogação vale no enlace vivo, não só na reconexão.
- Frame fora do formato é ignorado: não vira evidência de vida.

### 3.6 Diagnóstico

`GET /v1/federation/peers/{node_id}/link` — escopo **`federation:read`**.

```json
{ "state": "ready", "last_seen_at": 1794931200, "rtt_ms": 1,
  "negotiated_alpn": "godmesh/1", "peer_version": "0.1.0" }
```

- `state`: `ready` | `unknown` | `rejected`.
- Sem evidência (inclusive com o runtime de federação **não** anexado ao processo):
  `200` com `state: "unknown"` e os demais campos `null` — o piso honesto, não um erro que
  esconderia "não sei".
- `node_id` fora da forma canônica: `422 invalid_request` (não normaliza id "quase igual").
- `last_seen_at` é segundos Unix; `rtt_ms` é o RTT do QUIC no momento do handshake e é zerado
  quando o enlace expira.

---

## 4. Verificações executadas (saídas literais)

### 4.1 `cargo test -p godmesh-federation`

```
running 21 tests
test manifest::tests::oversized_frame_is_refused_before_allocation ... ok
test link::tests::never_seen_is_unknown_without_evidence ... ok
test link::tests::rejected_is_terminal_for_the_attempt ... ok
test link::tests::silent_link_expires_to_unknown_and_never_returns_to_ready ... ok
test identity::tests::debug_never_prints_the_secret ... ok
test identity::tests::node_id_is_the_public_key_and_round_trips ... ok
test manifest::tests::signing_with_a_foreign_key_is_refused ... ok
test registry::tests::older_revocation_epoch_does_not_undo_revocation ... ok
test registry::tests::touch_does_not_promote_trust_state ... ok
test registry::tests::authorize_and_revoke_keep_the_key_registered ... ok
test identity::tests::node_id_parse_rejects_non_canonical_forms ... ok
test registry::tests::unknown_key_is_not_known ... ok
test transport::tests::only_ed25519_is_advertised ... ok
test manifest::tests::any_flipped_byte_invalidates_the_signature ... ok
test transport::tests::non_ed25519_certificate_is_refused ... ok
test manifest::tests::signed_manifest_round_trips_and_verifies ... ok
test manifest::tests::window_rejects_past_and_future ... ok
test manifest::tests::verifying_with_another_cluster_or_alpn_is_refused ... ok
test identity::tests::signature_binds_message_and_key ... ok
test transport::tests::certificate_key_is_the_identity_key ... ok
test transport::tests::allowlist_refuses_unregistered_key_and_accepts_registered ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 4.2 Dois nós reais no mesmo host (portas distintas) — `tests/quic.rs`

```
running 8 tests
test chave_nao_registrada_nao_abre_conexao ... ok
test chave_do_manifesto_difere_do_certificado_tls ... ok
test peer_pending_e_recusado ... ok
test handshake_feliz_entre_dois_nos ... ok
test issued_at_fora_da_janela_e_recusado ... ok
test assinatura_invalida_rejeita_com_codigo ... ok
test no_revogado_nao_reconecta ... ok
test heartbeat_que_para_marca_unknown ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.12s
```

Cobertura por caso (o que cada um realmente exercita):

| Teste | Prova | Como |
|---|---|---|
| `handshake_feliz_entre_dois_nos` | mTLS nos **dois** sentidos, ALPN `godmesh/1`, manifesto assinado aceito dos dois lados, `rtt_ms`/`last_seen_at`/`peer_version` presentes | `FederationNode::dial` entre dois endpoints UDP loopback |
| `chave_nao_registrada_nao_abre_conexao` | allowlist no TLS: par não registrado não chega ao handshake | servidor com registro vazio |
| `assinatura_invalida_rejeita_com_codigo` | assinatura corrompida → `0x474D0002`, e o servidor registra `rejected` | conector cru com frame manipulado |
| `chave_do_manifesto_difere_do_certificado_tls` | nó autorizado **não** se passa por outro → `0x474D0004` | cert do atacante + manifesto assinado pela vítima, ambos registrados |
| `peer_pending_e_recusado` | `pending` → `0x474D0006` e o lado que tenta não promove para `ready` | registro do servidor com `Pending` |
| `issued_at_fora_da_janela_e_recusado` | replay de manifesto de 1 h atrás → `0x474D0009` | frame assinado com `issued_at` antigo |
| `heartbeat_que_para_marca_unknown` | silêncio (100 ms × 2) → `unknown`, `rtt_ms: null`, `last_seen_at` preservado; mais 400 ms não promove | par cru faz handshake e para de mandar heartbeat |
| `no_revogado_nao_reconecta` | revogação com `epoch` maior → `0x474D0007` em duas tentativas seguidas | `PeerRegistry::revoke(&a, 2)` |

### 4.3 Diagnóstico HTTP — `cargo test -p godmesh-server --test federation_link`

Sem banco (pool `connect_lazy` de URL inalcançável: se o handler tocasse o banco, o teste
falharia):

```
running 5 tests
test par_nunca_observado_e_unknown_com_campos_nulos ... ok
test grant_sem_escopo_de_federacao_e_403 ... ok
test node_id_nao_canonico_e_recusado ... ok
test sem_credencial_e_401 ... ok
test enlace_real_aparece_como_ready ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`enlace_real_aparece_como_ready` sobe **dois nós QUIC reais**, faz o handshake e então consulta o
endpoint HTTP com token de escopo `federation:read`, exigindo `state: "ready"`,
`negotiated_alpn: "godmesh/1"` e `peer_version: "0.1.0"` no corpo — o estado vem do transporte,
não de um default.

### 4.4 `cargo test --workspace`

Sem banco (testes que exigem `DATABASE_URL` são pulados):

```
$ cargo test --workspace 2>&1 | grep -cE "^test result: ok"
43
$ cargo test --workspace 2>&1 | grep -E "FAILED|failures:|error\[" | head
(vazio)
```

Com Postgres real (`DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_m2_10759`,
migrado pelo runner oficial) — **todas** as suítes, inclusive as que dependem de esquema:

```
$ DATABASE_URL=... cargo test --workspace 2>&1 | grep -E "^test result" \
  | sed 's/test result: //' | awk -F'[ ;]' '{s+=$2; f+=$5} END {print "passed:",s," failed:",f}'
passed: 269  failed: 0
```

### 4.5 `cargo clippy --workspace --all-targets`

```
$ cargo clippy --workspace --all-targets 2>&1 | tail -3
    Checking godmesh-federation v0.1.0 (.../crates/godmesh-federation)
    Checking godmesh-server v0.1.0 (.../crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Zero warnings (`workspace.lints` já trata `clippy::all` como `deny` e `unsafe_code` como
`forbid`; o crate repete `#![forbid(unsafe_code)]`).

### 4.6 Invariantes do ADR-072 §2 tocados nesta fatia

| # | Invariante | Evidência |
|---|---|---|
| 6 | Estado `unknown` nunca promovido a sucesso por passagem de tempo | `link::tests::silent_link_expires_to_unknown_and_never_returns_to_ready`, `heartbeat_que_para_marca_unknown`, `par_nunca_observado_e_unknown_com_campos_nulos` |
| 8 | Endereço/nome não é permissão; chave é | `transport::tests::allowlist_refuses_unregistered_key_and_accepts_registered`, `chave_nao_registrada_nao_abre_conexao` |

Os demais oito invariantes **não** são cobertos por esta fatia (§7).

---

## 5. O que esta fatia prova

1. Dois nós reais no mesmo host completam QUIC + mTLS + ALPN `godmesh/1`, com verificação de
   manifesto assinado nos **dois** sentidos.
2. A confiança é por **chave registrada**: chave fora da allowlist para no TLS; chave registrada
   mas `pending`/`revoked` é fechada pelo handshake com código próprio.
3. Um nó autorizado **não** consegue se passar por outro (chave do manifesto ≠ chave do
   certificado ⇒ recusa).
4. Janela de `issued_at` recusa replay de manifesto antigo.
5. Silêncio de heartbeat degrada para `unknown` e ali permanece; `ready` só com handshake
   verificado.
6. Revogação declarada no registro impede reconexão e encerra enlace vivo no próximo heartbeat.
7. O diagnóstico devolve o estado observado — e `unknown` quando não há evidência.

## 6. Limitações de implementação declaradas

- **Revogação não é propagada entre nós.** Cada nó tem o próprio `PeerRegistry` em memória; a
  revogação assinada e sua propagação são M1b/M3. Hoje, um nó só sabe o que foi registrado nele.
- **Registro não é persistido.** Reinício do processo perde `last_seen_at` e a allowlist em
  memória (o registro virá de armazenamento persistente em outra fatia). Nada aqui afirma
  durabilidade.
- **Epoch do manifesto local é fixo em 1.** O campo existe, é assinado e é comparado; a gestão de
  epoch (incremento, revogação assinada) é de outra fatia.
- **Sem descoberta.** Só se disca para endereço conhecido; mDNS/DNS/anycast não entram.
- **Sem 0-RTT e sem migração assistida.** Nenhum caminho de 0-RTT é habilitado (ADR-072 §5 pede
  handshake completo para o que é sensível, e a fatia não distingue classes de tráfego).
- **Não existe roteamento de dados por este enlace.** O transporte hoje carrega manifesto e
  heartbeat; stream de dados (eventos/sync escopado) é a fatia seguinte.

## 7. O que esta fatia NÃO prova (não declarar como pronto)

| Não provado | Por quê importa |
|---|---|
| **WAN / latência real / perda de pacote** | todo teste é loopback no mesmo host; RTT de 0–1 ms não diz nada sobre BR↔CO |
| **NAT traversal e relay** | nada de hole punching nem relay autorizado com quota; nó atrás de NAT simplesmente não conecta |
| **Migração de conexão QUIC** | não exercitada (troca de rede no meio do PTT é o motivo principal do QUIC, ADR-072 §5) |
| **IPv6 / Happy Eyeballs** | só IPv4 `127.0.0.1` |
| **Queda de processo/rede com `unknown` observado ponta a ponta** | o caminho `mark_unknown` por fechamento de conexão existe, mas o teste de degradação usa silêncio de heartbeat, não queda de host |
| **Split brain / partição de rede** | nenhum teste de partição; fencing não é desta fatia |
| **Revogação propagada e verificável entre nós** | só local (ver §6) |
| **Rotação de chave** | não implementada; trocar chave muda o `node_id` e exige novo registro |
| **Certificado expirado** | data de validade é ignorada **de propósito**; não há política de expiração de certificado (a identidade é a chave) |
| **Robustez a fuzzing do parser de manifesto/certificado** | sem fuzz; o parser de manifesto é caseiro e testado por casos, não por fuzzing |
| **Carga, concorrência alta, muitos pares** | nenhum teste de estresse; verificação de cert por allowlist faz varredura linear do registro |

## 8. Bloqueios e próximos passos

1. **Reconciliar com M1** quando `godmesh-federation` da fatia M1 chegar ao `main` (identidade,
   manifesto, registro). Este branch implementa o mínimo e deve ser ajustado ao formato de M1.
2. Próxima fatia de transporte: **stream de dados** (eventos escopados) sobre este enlace, com
   classes de tráfego (SOS completo, sync não sensível).
3. **Revogação assinada + propagação** (M1b) antes de qualquer alegação de malha pronta.
4. Relay autorizado com quota e NAT traversal — exige ADR própria (ADR-072 §3/§4).