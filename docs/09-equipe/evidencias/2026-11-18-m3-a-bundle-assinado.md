# M3-A — codec e verificação de bundle assinado

A coordenação liberou a implementação do recorte puro após o [aceite local da M1C-B](2026-11-18-m1c-ingresso-operacional.md#aceite-local-da-v12) e a revisão independente favorável da [SPEC M3-A v1.1](../../08-specs/federacao/m3-a-bundle-assinado.md). O SHA-256 do texto revisado é `dc8944952eaf6f0369f6e157d361c17698327f84d3fd169e6ff76cfeccacfd83`.

O recorte cobre somente `federation.minimum_protocol_version`, autenticação criptográfica de snapshots e classificação pura de histórico, tempo e compatibilidade. A integração operacional de pull e aplicação do piso exige SPEC e aceite próprios, conforme a ADR-075.

1. Congelar bytes canônicos, hash, assinaturas e política dos vetores de três e cinco votantes. A geração em Python por um codec independente produziu fixture com SHA-256 `a9c08cfd0bf54ae3ff2c03ee3b82f35f98d82c7e77cda0752872893ef975e04c`. O hash do corpo com três votantes coincidiu com o da SPEC.
2. Implementar contrato e verificador após testes de parser, precedência, quórum, histórico e janela. Conferir regressões do crate.
3. Executar suíte do workspace e Clippy, revisar o código de forma independente e registrar os resultados. O aceite de publicação, pull e aplicação em três servidores pertence à integração operacional.

## Aceite do codec

O parecer independente aprovou o commit `9622886ea754e2005abef6ee116f56ae9e765243`. O código, os testes e a fixture entraram na `main` local em `fcefe77` e foram comparados byte a byte com o candidato. O diff se restringe a `Cargo.toml`, `src/lib.rs`, `src/config_bundle.rs`, `tests/config_bundle.rs` e `tests/fixtures/m3-a-vetor.json` do crate `godmesh-federation`.

| Verificação executada | Resultado |
|---|---|
| Primeiro teste dirigido na base | Exit 101 pela ausência do módulo `config_bundle`. |
| Crate `godmesh-federation` | 97 aprovados, incluindo 7 testes da M3-A. |
| `cargo test --locked --offline --workspace` com PostgreSQL 16.15 descartável | 507 aprovados, 0 falhas e 1 ignorado pela suíte. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `git diff --check 3d107d0 9622886` | Exit 0. |

O vetor confirma literalmente os bytes TLV, o hash, a preimagem e as assinaturas para três e cinco votantes. Para preservar os lexemas de inteiros, o crate habilitou `raw_value` no `serde_json` já resolvido em 1.0.154. A [documentação oficial de RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html) foi conferida em 18/11/2026. `Cargo.lock` e as migrações permaneceram idênticos.

O aceite é do codec e das avaliações puras. O serviço durável da B1 e o pull com aplicação do piso da B2 têm aceites próprios.
