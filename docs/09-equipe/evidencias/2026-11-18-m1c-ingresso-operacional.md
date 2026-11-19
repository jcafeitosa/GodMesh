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

## Aceite local da v1.2

Commit de código `e01e999ce4056bf0cd8caffe41fa6714a60e46c1`, com parecer independente favorável. As migrações 0001–0011 e `Cargo.lock` foram conferidos byte a byte contra `57c9eff`.

| Verificação executada | Resultado observado |
|---|---|
| `cargo test --locked --offline --workspace` | 500 aprovados, 0 falhas, 1 ignorado pela suíte. PostgreSQL 16.15 descartável, com as duas URLs apontando ao mesmo banco. |
| `cargo test --locked --offline -p godmesh-server --test retention_policy_missing -- --ignored` | 1 aprovado em outro banco descartável. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 0012 aplicada depois da 0011 e 57 invariantes SQL verificados. A atualização e a reexecução da 0012 também têm teste de migração no workspace. |
| `git diff --check 57c9eff e01e999` | Exit 0. |

O ensaio no fio iniciou três processos em loopback no macOS, com portas TCP e UDP, bancos PostgreSQL e chaves distintas. Só A foi pinado como genesis. B e C importaram o manifesto de A pela API. A confirmou B e a oferta de C assinada por B. Cada processo importou as provas próprias e remotas aplicáveis. Os quatro sentidos de A↔B e B↔C atingiram `ready` com ALPN `godmesh/1` e observação de heartbeat. Nenhum par ou allowlist foi inserido por SQL.

- **Ingest.** B recebeu `422 event_scope_denied` antes do escopo. Com escopo e sem jurisdição do tenant, recebeu `409 retention_scope_unresolved`. Após a provisão de retenção, uma lacuna retornou `409 sequence_gap`. Uma posição contígua retornou `201 persisted_local` e seu replay retornou `200` sem nova linha.
- **Ordem e revogação.** No banco de A, o insert do ingest aguardou um bloqueio SQL controlado, enquanto a revogação aguardava a proteção transacional do cluster. Após liberar o insert, o ingest confirmou e a revogação bloqueou novos eventos de B e novas vigências de B e C. Com o recibo importado em B e C, B↔C saiu de `ready`. Reimportações retornaram `200`. Sem política portátil, B recusou boot antes dos listeners. Com a política restaurada, preservou a revogação. Clusters de listener e política divergentes fizeram A sair com código 78.
- **Resposta perdida e cadeia alternativa.** A confirmou nova geração de manifesto de C numa cadeia direta. Um intermediário de teste leu o `201` da raiz e encerrou a conexão com o cliente sem entregar a resposta. `GET /proof` e replay `200` devolveram a mesma prova. A versão das linhas da decisão e do piso permaneceu igual após a recuperação. A↔C atingiu `ready` e retomou o ingest.
- **Autoridade indisponível e par revogado.** Com a leitura de provas bloqueada no banco de C, A observou o enlace sair de `ready` antes de vencer a vigência. O novo handshake recuperou o enlace após liberar a consulta. A revogação explícita do par A no banco de C, usada como condição de teste, fez o enlace ser recusado e o ingest de A retornar `422 event_node_unauthorized`, mesmo com pin e escopo presentes.
- **Prazo sem recibo.** Em outra execução com três bancos novos, B↔C permaneceu em `ready` após A revogar A→B, enquanto B e C ainda tinham vigências válidas e aguardavam o recibo. Após o vencimento, o enlace saiu de `ready` e C recusou novo ingest. A contagem durável de posições em C permaneceu igual.

As duas rodadas encerraram os processos e os clusters PostgreSQL descartáveis. O ensaio verificou ingresso, vigência, revogação, QUIC e ingest no recorte M1C-B. A distribuição de configuração, a replicação de eventos e a validação em hosts Linux independentes continuam nas fatias seguintes.

A rodada com entrega do recibo também foi repetida no mesmo binário com os
listeners HTTP e QUIC dos três processos em loopback IPv6 (`::1`). Ingresso,
heartbeats, ingest, ordenação da revogação, recuperação após perda da resposta
e troca para a cadeia alternativa passaram. O PostgreSQL descartável continuou
em loopback IPv4 nessa rodada.
