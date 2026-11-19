# M1C-A — prova portátil de ingresso

A coordenação aprovou a implementação da [SPEC M1C-A v1.1](../../08-specs/federacao/m1c-a-prova-portatil.md) após parecer independente favorável em 18/11/2026. O parecer conferiu o SHA-256 `c958c6aecbcd5518e1514e57f15517c1db01adc1fddbbd8ff0b869d62bb61231` e resolveu as ressalvas sobre discriminação de versão, estrutura JSON e ordem dos erros. A liberação segue a ADR-075 e a direção da ADR-076.

O codificador de referência em Python com Ed25519 gerou as entradas, assinaturas, bytes canônicos e digests de A → B → C. O JSON de referência teve SHA-256 `ec89d2e2a13684afb624e8ab883756e967b587d0bb219d6610ae45f8de769433`. A comparação com a implementação Rust será verificada após o ciclo TDD.

O aceite desta fatia é criptográfico. A comprovação de emissão e consumo entre três servidores depende da integração operacional da M1C.
