# M3-B1 — serviço durável do piso de configuração

A coordenação aprovou a implementação isolada após o [aceite da M3-A](2026-11-18-m3-a-bundle-assinado.md#aceite-do-codec) e o parecer independente favorável à [SPEC M3-B1 v1.2](../../08-specs/federacao/m3-b1-servico-duravel.md). O SHA-256 da SPEC revisada é `7b98b1e76bb6b58718974f00948c33a6cadef1be9b8cad18c886cc182d77d7ea`.

1. Adicionar migração incremental e preflight para validar e reconstruir o estado com a política pinada.
2. Autenticar e ordenar bundles com o codec M3-A, manter replays, conflito e incompatibilidade duráveis e aplicar seleção e resolução sob o lock M1C.
3. Compartilhar um segundo explícito de autorização. Testar ativação, janela e classificação de operações sob o lock até commit.
4. Verificar os resultados P/N/B/F em PostgreSQL descartável, preservar as migrações aplicadas, rodar a suíte e solicitar revisão independente do código.

A [proposta operacional M3-B](../../08-specs/federacao/m3-b-bundle-operacional.md) documenta a etapa seguinte. O listener, o pull e o uso do piso nos efeitos de QUIC e ingest continuam sujeitos ao aceite da B2. Nenhum teste da B1 foi executado até a aprovação da SPEC.

## Aceite do serviço durável

O parecer independente aprovou `fe4202de096c4112711ef9a96a6fa8e487fcd014`. A integração local `619223e` contém o mesmo código e migração. O squash introduz somente a versão final da 0013 no histórico da `main`. As migrações 0001–0012 e `Cargo.lock` foram conferidos byte a byte.

`ClusterGuard` vincula a autorização explicitamente temporizada e as operações M3-B1 à transação que adquiriu o advisory lock M1C em `READ COMMITTED`. O teste distingue o lock de sessão do lock transacional ainda detido até o commit. A trilha de recuperação impede que o preflight aceite um marcador que contradiz uma resolução pendente ou uma exigência já devida substituída. O worker verificou essas regressões antes e depois da correção.

| Verificação executada no candidato revisado | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` com PostgreSQL 16.15 descartável | 522 aprovados, 0 falhas e 1 ignorado pela suíte. Inclui 13 testes M3-B1 e 2 testes de compilação negativa. |
| `cargo test --locked --offline -p godmesh-server --test retention_policy_missing -- --ignored` | 1 aprovado em banco descartável separado. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` com a 0013 incluída | Aplicação em sequência 0001–0013 e 57 invariantes SQL aprovados. |
| Ensaio M1C-B com binário do candidato | Três processos em `::1`. Ingresso, heartbeats, ingest, revogação, recuperação após perda de resposta e cadeia alternativa passaram. |

O serviço durável está disponível para integração interna. Seu uso por HTTPS, pelo QUIC e pelo ingest depende da [SPEC M3-B2](../../08-specs/federacao/m3-b2-integracao-operacional.md) e do aceite operacional em três instâncias. A revisão do texto da B2 foi favorável como proposta. A integração exige autorização para o adaptador TLS e para classificar o ingest HTTP como nível 1.
