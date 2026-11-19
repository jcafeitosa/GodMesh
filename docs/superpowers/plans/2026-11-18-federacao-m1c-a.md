# Plano de implementação da prova portátil M1C-A

**Meta.** Implementar a [SPEC M1C-A v1.1](../../08-specs/federacao/m1c-a-prova-portatil.md) no crate `godmesh-federation`, com comparação contra vetores de um codificador independente. A cadeia é avaliada pelo cluster, chaves, limites e instante fornecidos pelo chamador.

1. Em `crates/godmesh-federation/tests/join_chain.rs`, preparar os testes contra o vetor A → B → C e os casos da §5. Conferir bytes, digests, assinatura, precedência de versão e de falha por elo. Executar antes da implementação e registrar o motivo das falhas.
2. Em `crates/godmesh-federation/src/join_chain.rs` e `src/lib.rs`, implementar a checagem de tamanho e versão, codec e verificador de cadeia. Reutilizar Ed25519, SHA-256, `SignedManifest` e o TLV existente. Preservar os módulos de v1. Rodar os casos dirigidos até passarem e verificar as regressões v1.
3. Rodar os testes do crate, o workspace e o Clippy. Obter revisão independente do código e registrar resultados, arquivos e limites. A revisão e os testes do ingresso e da revogação entre processos continuam na M1C operacional.
