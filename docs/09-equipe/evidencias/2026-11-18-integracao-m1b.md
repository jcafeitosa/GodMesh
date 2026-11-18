# Integração da M1B com a ingestão assinada

## Liberação da fatia

A [ADR-075](../../06-planejamento/decisoes.md#adr-075--aprovação-por-fatia-da-federação) registra a autorização do usuário para avançar por fatia.

O crítico técnico independente levantou o veto em 18/11/2026 e emitiu parecer favorável à [SPEC M1B/I v1.2](../../08-specs/federacao/m1b-integracao.md), SHA-256 `44822e58667e758a469644b71731375413699d150e15b025e844636d2d861fe3`. Os vetos das versões anteriores trataram do legado da migração, da governança, da concorrência e do contrato HTTP. Esta versão explicita que a implantação depende de zero aprovações legadas, exige declaração do cluster sem votantes, define as invariantes transacionais e separa o aceite local do aceite de ponta a ponta.

A coordenação aprova a implementação da v1.2 sob SDD e TDD em 18/11/2026. O aceite da implementação depende dos resultados observados no ambiente descartável e de revisão independente do código. O aceite de ponta a ponta aguarda a execução após M1C, com pelo menos três servidores.

## Base verificada

Em `51e192c`, com um PostgreSQL 16 descartável e `DATABASE_URL` apontando para ele, `cargo test --locked -p godmesh-server --test federation --test event_signature --test federation_link` completou com 25 testes aprovados, zero falhas e zero ignorados. A base contém S-sec-2. A execução antecedeu a integração da M1B.
