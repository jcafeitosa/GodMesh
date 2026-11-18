# Integração da M1B com a ingestão assinada

## Liberação da fatia

A [ADR-075](../../06-planejamento/decisoes.md#adr-075--aprovação-por-fatia-da-federação) registra a autorização do usuário para avançar por fatia.

O crítico técnico independente levantou o veto em 18/11/2026 e emitiu parecer favorável à [SPEC M1B/I v1.2](../../08-specs/federacao/m1b-integracao.md), SHA-256 `44822e58667e758a469644b71731375413699d150e15b025e844636d2d861fe3`. Os vetos das versões anteriores trataram do legado da migração, da governança, da concorrência e do contrato HTTP. Esta versão explicita que a implantação depende de zero aprovações legadas, exige declaração do cluster sem votantes, define as invariantes transacionais e separa o aceite local do aceite de ponta a ponta.

A coordenação aprovou a implementação da v1.2 sob SDD e TDD em 18/11/2026. O aceite da implementação depende dos resultados observados no ambiente descartável e de revisão independente do código. O aceite de ponta a ponta aguarda a execução após M1C, com pelo menos três servidores.

A revisão de código do candidato `65136e6` bloqueou a integração até que o piso de epoch abrangesse rejeições e a expiração fosse validada e persistida com precisão de um segundo. A revisão documental emitiu parecer favorável à v1.3, SHA-256 `61a25f7a5ac886c7da6a46b9d75bff5c1d3f5f2124b2d51a4be62dcc520e20de`. A coordenação aprovou a correção na v1.3. Mantém-se a exigência de revisão do código e execução dos casos novos.

## Base verificada

Em `51e192c`, com um PostgreSQL 16 descartável e `DATABASE_URL` apontando para ele, `cargo test --locked -p godmesh-server --test federation --test event_signature --test federation_link` completou com 25 testes aprovados, zero falhas e zero ignorados. A base contém S-sec-2. A execução antecedeu a integração da M1B.

O cliente HTTP intermediário, com codificação canônica independente em Python, registrou o candidato B como pendente com `201` e confirmou `422 event_node_unauthorized` ao publicar antes de aprovar. A aprovação na base respondeu `200` e o teste falhou pela ausência de `approved_by_node`. Este é o resultado esperado antes da integração.

A falha intermitente registrada na integração S-sec-2 foi reproduzida na base. Na primeira rodada de `cargo test --locked -p godmesh-server --test events` com o paralelismo padrão, cinco testes passaram e três falharam ao cadastrar o nó de teste compartilhado, com SQLSTATE `23505` em `federation_peer_peer_id_key`. A falha ocorreu na preparação das fixtures, antes da operação HTTP.

## Transporte de referência com três servidores locais

Teste executado sobre o binário da base `51e192c`, compilado com `cargo build --locked -p godmesh-server --bin godmesh-server`. A, B e C usaram três processos, três identidades, três bancos descartáveis e portas TCP e UDP distintas no loopback. O cliente Python submeteu manifestos assinados pela API e acionou as aprovações administrativas locais em cada servidor. As tabelas de confiança foram preenchidas pelo fluxo HTTP.

| Etapa | Resultado observado em `GET /v1/federation/peers/{node_id}/link` |
|---|---|
| Inicialização | `/healthz` retornou `200` nos três servidores. Seis enlaces direcionais em `unknown`. |
| Autorização local entre A e B | AB e BA em `ready`, usando `godmesh/1` e com heartbeat observado. Quatro direções de C em `unknown`. |
| Autorização local de todos os pares | Seis direções em `ready`. |
| Parada de B | AB e CB em `unknown`. AC e CA em `ready`. |
| Reinício de B com o próprio banco e chave | Seis direções em `ready` novamente. |

O cliente terminou com código zero. Esta execução verifica transporte local, detecção de queda e reconexão com aprovação por servidor. O aceite M1C ainda depende de provar distribuição e consumo da cadeia pelo protocolo. Replicação, rede entre hosts Linux distintos e recuperação de partição com replay não foram exercitadas.

## Candidato anterior à correção da v1.3

Em `65136e6`, o cliente HTTP registrou a recusa do evento de B antes da aprovação e a recusa por falta de escopo após a aprovação. Com escopo concedido, observou `201` com `persisted_local`, `200` no replay, uma única posição durável e `409 sequence_gap` para salto de sequência. B aprovou C e os esquemas de aprovação e evento apareceram no OpenAPI. A repetição do teste QUIC com três servidores verificou assinatura de aprovação no retorno de cada servidor e os mesmos estados de enlace da tabela anterior.

A suíte padrão do workspace passou com 467 testes, zero falhas e 1 ignorado. O caso ignorado passou quando executado isoladamente em outro banco descartável. O Clippy saiu com código zero e o script de migrações passou com 57 invariantes. O candidato ficou no worktree para correção dos dois pontos de revisão da v1.3.
