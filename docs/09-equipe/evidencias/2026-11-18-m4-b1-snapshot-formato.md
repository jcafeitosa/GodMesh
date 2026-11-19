# M4-B1 — formato e verificação do snapshot de metadados

A coordenação aprova a [SPEC M4-B1 v1.1](../../08-specs/federacao/m4-b1-snapshot-formato.md), SHA-256 `279be6c80e9e616988e29dd9fcc59392dd854b0e26547e2cd22093dee4b5ed51`, conforme a ADR-075. A revisão independente confirmou a precedência do parser e a validação estrita das provas dessa versão. O [aceite da M4-A2](2026-11-18-m4-a2-replica-quic.md#aceite-operacional-em-três-instâncias) foi registrado em `11ebc1f`.

1. **Vetor independente.** Gerar pacote com as chaves sintéticas da SPEC e registrar bytes canônicos, digest, assinaturas e frames exatos.
2. **Codec.** Implementar em `godmesh-federation` o manifesto assinado, o framing e a leitura estrita de escopo, limites e lexemas. Reusar os codecs TLV e de evento e as dependências existentes. [Serde JSON RawValue 1.0.154](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html) · [SHA-2 0.11.1](https://docs.rs/sha2/0.11.1/sha2/) · [Ed25519 estrito 3.0.0](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict)
3. **Verificação.** Cobrir correlação com a política, assinatura do manifesto, digest ordenado, assinatura de cada evento e sequência contígua. Comparar literalmente o vetor independente e conferir as falhas tipadas e seus índices.
4. **Integração.** Rodar suíte do crate e do workspace, Clippy estrito e revisão independente do código antes de integrar.

A B1 autentica os bytes e a janela assinada. A B2 decide elegibilidade corrente, aplica o snapshot e reconcilia o restore. O código e as verificações da B1 seguem pendentes.
