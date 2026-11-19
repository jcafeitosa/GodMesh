# M1C-B — ingresso portátil em operação

A coordenação aprovou a implementação da [SPEC M1C-B v1.2](../../08-specs/federacao/m1c-b-ingresso-operacional.md) após parecer independente favorável. O SHA-256 revisado é `f433e600738b877d2ea91218f35d7c9dd6834fe0d4bd582d83d51449d51efa83`. A versão fixa a raiz como autoridade das decisões por cadeia, o bloqueio comum por cluster, a precedência de revogação, a separação dos limites de corpo e frame e o bootstrap pela API oficial.

## Plano de implementação

1. **Contratos e persistência.** Integrar M1C-B1, aplicar a migração aditiva `0012_federation_portable_join.sql` e testar parâmetros de boot, estado inicial e compatibilidade com o histórico da M1B.
2. **Ofertas e decisões.** Implementar serviço no módulo `crates/godmesh-server/src/portable_join.rs` e rotas no módulo `http/portable_join.rs`. Provar autoria, encadeamento, piso, atomicidade, recuperação e repetição histórica.
3. **Bootstrap e vigência.** Exercitar manifesto publicado, importação de genesis por pin, instalação de prova ativa do próprio processo e de pares, renovação e recusa após perda da política local.
4. **Revogação.** Verificar recibos pela evidência criptográfica da M1C-B1 e provar persistência, reimportação tardia, ordem no cluster e efeito em descendentes. Preservar decisões legadas.
5. **Autorização em uso.** Integrar a checagem PostgreSQL em QUIC e ingest. Os callbacks no crate de transporte recebem uma proteção mantida durante a aceitação local e um sinal de alteração da autoridade. O caminho legado continua funcionando fora do modo portátil.
6. **Aceite.** Rodar TDD dirigido, suíte do workspace, Clippy, invariantes SQL e revisão independente dos diffs. No fio, instalar A, B e C com bancos e identidades próprios, obter os enlaces pela prova v2 e verificar revogação e expiração. Registrar a topologia e os efeitos observados.

Até a implementação e a execução, estes casos são planejados. M3–M5 e a validação em hosts Linux independentes têm aceites próprios.

## Controle inicial do ensaio

No binário de `c71be47`, o ensaio HTTP iniciou os três processos em loopback com bases PostgreSQL e identidades distintas. O primeiro pedido, `GET /v1/federation/local-manifest` em A, retornou `404` em vez de `200`. A execução terminou nessa fronteira, antes de qualquer importação, e desligou os processos descartáveis.
