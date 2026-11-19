# M4-B1 — formato e verificação do snapshot de metadados

A coordenação aprova a [SPEC M4-B1 v1.1](../../08-specs/federacao/m4-b1-snapshot-formato.md), SHA-256 `279be6c80e9e616988e29dd9fcc59392dd854b0e26547e2cd22093dee4b5ed51`, conforme a ADR-075. A revisão independente confirmou a precedência do parser e a validação estrita das provas dessa versão. O [aceite da M4-A2](2026-11-18-m4-a2-replica-quic.md#aceite-operacional-em-três-instâncias) foi registrado em `11ebc1f`.

1. **Vetor independente.** Gerar pacote com as chaves sintéticas da SPEC e registrar bytes canônicos, digest, assinaturas e frames exatos.
2. **Codec.** Implementar em `godmesh-federation` o manifesto assinado, o framing e a leitura estrita de escopo, limites e lexemas. Reusar os codecs TLV e de evento e as dependências existentes. [Serde JSON RawValue 1.0.154](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html) · [SHA-2 0.11.1](https://docs.rs/sha2/0.11.1/sha2/) · [Ed25519 estrito 3.0.0](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict)
3. **Verificação.** Cobrir correlação com a política, assinatura do manifesto, digest ordenado, assinatura de cada evento e sequência contígua. Comparar literalmente o vetor independente e conferir as falhas tipadas e seus índices.
4. **Integração.** Rodar suíte do crate e do workspace, Clippy estrito e revisão independente do código antes de integrar.

## Aceite do codec

O commit `912d185d766e3e634d4175a7eb5b9b3ae5634597` foi aprovado em revisão independente e integrado localmente. O módulo `snapshot` autentica o manifesto e cada prova segundo política explícita, antes de devolver as fatias originais. O encoder usa a mesma validação. A B2 decide elegibilidade corrente, aplica o snapshot e reconcilia o restore.

O vetor independente usou Python e Cryptography 50.0.2 com as sementes sintéticas da SPEC. O pacote tem 1.596 bytes e SHA-256 `8ea8e94ba5ebfd5987219ae1f1ccbcf9e32db49bb3afb9d57309712e039c4676`. O teste Rust reproduziu literalmente o pacote, o TLV canônico, a preimagem e o digest ordenado, as assinaturas e as duas fatias originais. O SHA-256 do [vetor versionado](../../../crates/godmesh-federation/tests/fixtures/m4-b1-vetor.json) é `b30058cc2e428f5293fccdc5fb5b2d54c2189179c86857abe63e8819c12d14fd`. [Cryptography 50.0.2 Ed25519](https://cryptography.io/en/50.0.2/hazmat/primitives/asymmetric/ed25519/)

O primeiro teste falhou pela ausência do módulo. A revisão identificou que o primeiro encoder podia emitir prova malformada com digest correto. O teste dirigido reproduziu a aceitação indevida antes da correção. Os seis testes finais cobriram a recusa em emissão e recepção, a precedência de erros, o índice da primeira prova inválida, os tetos e a preservação da janela assinada.

| Verificação do commit | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` em PostgreSQL 16.15 descartável | 601 aprovados, zero falhas e um ignorado por desenho. Inclui os seis cenários B1. |
| Teste ignorado de política ausente, em banco próprio | Um aprovado. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 64 invariantes em banco criado do zero. |

O delta está restrito a `godmesh-federation`, com as dependências, o lockfile e as migrações preservados. O aceite abrange o codec puro. Aplicação, transporte do snapshot e restore seguem na B2.
