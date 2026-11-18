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

## Aceite local da v1.3

Em 18/11/2026, o crítico independente aprovou o código em `e060b4e04bd331abb706d36cc8d9d49f2bcc593a`. O parecer conferiu a associação entre a SPEC v1.3 e o candidato, o piso por aprovador, rollback, replay, expiração UTC exata e compatibilidade das migrações. A `main` local foi avançada até esse commit após os testes.

| Verificação em PostgreSQL 16.15 descartável | Resultado observado |
|---|---|
| `cargo test --workspace --locked`, paralelismo padrão com as duas variáveis de banco no mesmo ambiente | 470 aprovados, 0 falhas, 1 ignorado pela suíte |
| `cargo test --locked -p godmesh-server --test retention_policy_missing -- --ignored`, banco isolado | 1 aprovado |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Código de saída 0 |
| `bash migrations/tests/run_all.sh` | 57 invariantes. 0009, 0010 e 0011 aplicadas em ordem. |
| `git diff --check` e comparação dos arquivos SQL e Cargo | Diff limpo. 0009 e 0010 idênticas às origens. Cargo.toml e Cargo.lock idênticos à base. |
| Ingresso e ingest assinados com cliente Python independente | Nó pendente recusado. A aprova B, o servidor persiste a assinatura, B publica após receber escopo, o replay mantém uma posição e B aprova C. Contratos de aprovação e evento conferidos no OpenAPI. |
| Transporte QUIC/mTLS com três instâncias e aprovações assinadas obtidas pela API de cada uma | C permaneceu `unknown` até autorização. Todos os enlaces foram a `ready`. Os enlaces de A e C com B passaram a `unknown` após a parada de B, AC e CA seguiram prontos e os seis voltaram a `ready` após o reinício de B. |

O teste do piso confirmou que a rejeição também eleva o epoch aceito para decisões futuras. O teste de validade comparou sem arredondamento a expiração armazenada no banco e a retornada na API. Ambos mantiveram o último segundo persistível, e o segundo seguinte retornou `422 invalid_request` sem efeito. A correção da fixture compartilhada passou em dez rodadas paralelas de `events` e na suíte integrada padrão.

Arquivos de código e migração integrados:

- `crates/godmesh-federation/src/{approval,genesis,lib,manifest}.rs`
- `crates/godmesh-server/src/{api,events,federation,inbox,main}.rs` e `crates/godmesh-server/src/http/{contract,federation,mod,openapi}.rs`
- `crates/godmesh-server/tests/{api_contract,event_signature,federation,runtime}.rs` e `crates/godmesh-server/tests/common/mod.rs`
- `migrations/0009_federation_approval.sql`, `migrations/0011_federation_decision_epoch.sql`, `migrations/tests/0009_invariants.sql`, `migrations/tests/run_all.sh` e `deploy/README.md`

O aceite é local, com aprovação em cada servidor. Para o aceite de ponta a ponta ainda faltam a [cadeia portátil de ingresso e revogação](../../08-specs/federacao/m1c-cadeia-confianca.md), a configuração assinada, a replicação escopada e o ensaio em hosts independentes.
