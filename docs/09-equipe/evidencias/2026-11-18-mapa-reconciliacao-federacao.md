# Mapa de reconciliação — M1 (ingresso) × M2 (transporte)

> Levantamento feito por worker antes de qualquer merge, em 18/11/2026. Registro de insumo, **não** de implementação: a reconciliação em si ainda não foi commitada.

## Canônico no `main` (`bda4108`, fatia M1)

| Arquivo | Conteúdo |
|---|---|
| `crates/godmesh-federation/src/identity.rs` | `NodeIdentity` com chave em arquivo (`GODMESH_NODE_KEY`, 0600, diretório 0700); `node_id = "gmn_" + sha256(pubkey)` |
| `src/manifest.rs` | `SignedManifest` com `canonical_bytes()` (domínio `godmesh.federation.manifest.v1`, campos com tag+len, `capabilities`/`endpoints` normalizados), `verify`, `verify_fresh` |
| `src/peer.rs` | `PeerRecord` + `TrustState` (pending/authorized/revoked/unknown) com transições |
| `migrations/0008_federation_peers.sql` | `federation_peer`, `federation_join_request` |
| rotas | `/v1/federation/join-requests` (pública), `.../approve`, `.../reject`, `/v1/federation/peers` |

## Fora do `main` (`96e1d40`, `97feb18`, fatia M2)

`identity.rs` próprio (`NodeId` = hex cru da pubkey, `NodeIdentity` em memória, `pkcs8_der()`), `manifest.rs` próprio (frame binário `GMF1` com `alpn`/`trust_state`/`issued_at`/`epoch`), `registry.rs` próprio (`PeerRegistry` em memória), `transport.rs` (quinn + rustls, ALPN `godmesh/1`, validação por chave do SPKI), `handshake.rs` (códigos `0x474D00xx`), `link.rs` (`PeerLinks`, `unknown` como piso), `node.rs` (bind/dial, heartbeat 5 s × 3), `tests/quic.rs` (8 testes de fio).

## Colisões e decisão de autoridade

| Conceito | M1 (canônico) | M2 | Decisão |
|---|---|---|---|
| `NodeIdentity` | arquivo 0600 + `gmn_…` | memória + `pkcs8_der` | M1, acrescentando `NodeId` tipado e `pkcs8_der()` |
| `node_id` | `gmn_` + sha256(pubkey) | hex(pubkey) | M1 vence; `NodeId` valida o formato `gmn_` |
| `SignedManifest` | struct serde + `canonical_bytes` | frame binário `GMF1` + `alpn`/`trust_state` | M1 estendido com `alpn`/`trust_state`; frame binário vira método do mesmo tipo |
| `PeerRecord` | serde, espelha o banco | struct em memória | M1; `PeerRegistry` guarda o `PeerRecord` de M1 |
| `TrustState` | 4 estados (inclui `unknown`) | 3 estados | M1 (o `CHECK` do banco tem os 4) |
| Estado de enlace | — | `PeerLinks`/`LinkState` | mantido de M2: conceito distinto de `TrustState` |

## Por que isso importa

Dois tipos com o mesmo nome e formatos diferentes no mesmo crate produzem bug silencioso: o banco grava um formato, o transporte valida outro, e a falha aparece só em produção. A regra aplicada foi "um conceito, um tipo canônico, extensível por teste" — nunca dois structs para a mesma coisa.
