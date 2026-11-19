# M3-A — codec e verificação de bundle assinado

A coordenação liberou a implementação do recorte puro após o [aceite local da M1C-B](2026-11-18-m1c-ingresso-operacional.md#aceite-local-da-v12) e a revisão independente favorável da [SPEC M3-A v1.1](../../08-specs/federacao/m3-a-bundle-assinado.md). O SHA-256 do texto revisado é `dc8944952eaf6f0369f6e157d361c17698327f84d3fd169e6ff76cfeccacfd83`.

O recorte cobre somente `federation.minimum_protocol_version`, autenticação criptográfica de snapshots e classificação pura de histórico, tempo e compatibilidade. A integração operacional de pull e aplicação do piso exige SPEC e aceite próprios, conforme a ADR-075.

1. Congelar bytes canônicos, hash, assinaturas e política dos vetores de três e cinco votantes. A geração em Python por um codec independente produziu fixture com SHA-256 `a9c08cfd0bf54ae3ff2c03ee3b82f35f98d82c7e77cda0752872893ef975e04c`. O hash do corpo com três votantes coincidiu com o da SPEC.
2. Implementar contrato e verificador após testes de parser, precedência, quórum, histórico e janela. Conferir regressões do crate.
3. Executar suíte do workspace e Clippy, revisar o código de forma independente e registrar os resultados. O aceite de publicação, pull e aplicação em três servidores pertence à integração operacional.
