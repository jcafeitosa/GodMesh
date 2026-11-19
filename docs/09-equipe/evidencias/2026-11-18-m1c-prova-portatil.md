# M1C-A — prova portátil de ingresso

A coordenação aprovou a implementação da [SPEC M1C-A v1.1](../../08-specs/federacao/m1c-a-prova-portatil.md) após parecer independente favorável em 18/11/2026. O parecer conferiu o SHA-256 `c958c6aecbcd5518e1514e57f15517c1db01adc1fddbbd8ff0b869d62bb61231` e resolveu as ressalvas sobre discriminação de versão, estrutura JSON e ordem dos erros. A liberação segue a ADR-075 e a direção da ADR-076.

O codificador de referência em Python com Ed25519 gerou as entradas, assinaturas, bytes canônicos e digests de A → B → C. O JSON de referência teve SHA-256 `ec89d2e2a13684afb624e8ab883756e967b587d0bb219d6610ae45f8de769433`. O teste Rust comparou literalmente bytes, digests e assinaturas e verificou a cadeia de um e dois elos.

O aceite desta fatia é criptográfico. A comprovação de emissão e consumo entre três servidores depende da integração operacional da M1C.

## Aceite M1C-A

O código recebeu parecer independente favorável e entrou na `main` em `a8028a05eb83ce1c735ed5ae2a7b29a8868d2825`. A execução inicial dirigida falhou pela ausência da API `join_chain`. Após implementar, passaram os 7 testes dirigidos, os 57 unitários e os 10 de QUIC. A coordenação executou `cargo test --locked --offline --workspace` em PostgreSQL 16 descartável, com 477 aprovados, 0 falhas e 1 ignorado. `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` e `git diff --check` passaram. O código v1 e os arquivos de dependência permaneceram idênticos.

Arquivos de implementação: `crates/godmesh-federation/src/{join_chain,lib}.rs`, `crates/godmesh-federation/tests/join_chain.rs` e `crates/godmesh-federation/tests/fixtures/m1c-a-vetor.json`.

## Liberação M1C-B1

A revisão independente deu parecer favorável à [SPEC M1C-B1 v1.1](../../08-specs/federacao/m1c-b1-vigencia-revogacao.md), SHA-256 `8b230e331c0bc333ad60e2e20f0bcf179fedf89309c1a2fec265bb61ebd54d15`. A coordenação aprovou a implementação do contrato criptográfico. O vetor independente dos três artefatos foi gerado em Python, com SHA-256 `0939faa35f2cc3a7eaeab10d0c716923b3cb2bc4d8e24a461299c98d6f85db0b`.

Plano da subfatia:

1. Em `crates/godmesh-federation/tests/join_authority.rs`, comparar o vetor e os vínculos de vigência, revogação e recibo. Executar antes do código e registrar a falha pelo comportamento ausente.
2. Em `crates/godmesh-federation/src/join_authority.rs` e `src/lib.rs`, implementar os três artefatos e a evidência de revogação vinculada à cadeia. Reutilizar o codec, os tipos e os domínios aprovados. Reexecutar os casos de borda e regressão.
3. Rodar suíte e Clippy, revisar o código por SHA e registrar evidência de implementação. A persistência e a ativação pelo servidor seguem a SPEC operacional separada.

### Aceite M1C-B1

O teste dirigido falhou inicialmente pela ausência da API e passou com seis casos após a implementação. A suíte do crate passou com 80 testes. A coordenação verificou o candidato `7ba34b39d9b7693cd4dda5cb3048f57fb0db0adf` com PostgreSQL 16 descartável e diretório de compilação isolado. `cargo test --locked --offline --workspace` teve 483 aprovados, zero falhas e um ignorado. Clippy do workspace com `-D warnings` e `git diff --check` passaram.

O parecer independente verificou a construção restrita de `VerifiedJoinRevocation`, os vínculos exigidos pelo recibo, os domínios, o vetor independente e a preservação da M1C-A e da v1. A integração local está em `6fd91a7`. Arquivos em `crates/godmesh-federation`: `src/{join_authority,lib}.rs`, `tests/join_authority.rs` e `tests/fixtures/m1c-b1-vetor.json`.
