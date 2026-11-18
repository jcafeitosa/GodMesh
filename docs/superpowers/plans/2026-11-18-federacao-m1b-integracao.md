# Plano de integração do ingresso assinado M1B

**Meta.** Unir a aprovação assinada da M1B e a ingestão de eventos assinados da S-sec-2 na linha principal, com prova de migração e de que os dois controles funcionam juntos.

**Dependências.** ADR-071, ADR-072, ADR-075 e [SPEC da integração](../../08-specs/federacao/m1b-integracao.md). Base de produto `51e192c` e M1B `0550be4`. A implementação começa após revisão e aprovação da versão da SPEC. Em desenvolvimento, usar PostgreSQL descartável e as dependências fixadas no `Cargo.lock`.

## 1. Fixar o comportamento com testes

- [x] Com um cliente Python independente, usar o mesmo servidor HTTP real para apresentar o manifesto de um candidato, tentar publicar seu evento enquanto pendente e aprovar o ingresso com um genesis. Conferir que o evento ainda é negado até a concessão explícita do escopo, que a primeira ingestão retorna `201` e que o replay retorna `200` com uma posição durável.
- [x] Em `crates/godmesh-server/tests/runtime.rs`, exercitar a migração de uma cópia de esquema com `0010` aplicada e `0009` ausente. Conferir que só `0009` é aplicada e que nova execução mantém os verbetes anteriores. Cobrir também a falha transacional diante de aprovação legada.
- [x] Rodar os casos novos contra a base e observar falha pelos comportamentos ausentes da M1B antes de integrá-la.

## 2. Integrar a M1B

- [x] Resolver os conflitos em `godmesh-federation/src/{lib,manifest}.rs`, `godmesh-server/src/http/{mod,openapi}.rs` e `migrations/tests/run_all.sh`.
- [x] Manter o codec canônico compartilhado, os tipos e erros da aprovação e dos eventos, as janelas de frescor independentes e as duas migrações com seus checksums.
- [x] Rodar novamente os casos novos e os testes de federação, ingestão, runtime e contrato. Os dois testes no fio também passaram no SHA integrado.

## 3. Aplicar o contrato de decisão

- [x] Em `tests/federation.rs` e `src/main.rs`, fixar a configuração explícita do cluster sem votantes, desabilitada por padrão, e os erros de configuração e HTTP. Manter o corpo e domínio de aprovação e vincular a rejeição ao domínio próprio em `godmesh-federation/src/approval.rs`.
- [x] Fixar em teste que replay de aprovação mantém o registro e a configuração do par e que uma falha transacional permite retomar. Serializar por identidade do aprovador até o commit em `src/federation.rs`.
- [x] Provar o piso de epoch após aprovação e rejeição e o limite exato de `timestamptz` antes de implementar a v1.3. Criar `0011_federation_decision_epoch.sql` para inicializar e proteger o piso durável. Conferir transição e dados de prova sob o mesmo commit.

## 4. Serializar ingest e Inbox

- [x] Em `tests/event_signature.rs`, fixar a proteção da autoridade e do escopo dentro da transação de ingest, a monotonicidade por namespace e a convivência com o checkpoint legado do Inbox nos dois sentidos.
- [x] Em `src/events.rs` e `src/inbox.rs`, compartilhar o bloqueio transacional por origem, tenant, stream e partição. Manter as consultas de par e escopo protegidas até o commit da ingestão.

## 5. Verificar e registrar

- [x] Rodar `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `bash migrations/tests/run_all.sh` e `git diff --check`.
- [x] Solicitar revisão independente do conjunto integrado. Registrar os resultados literais, revisar o diff e só integrar o commit após resolver achados materiais.
- [x] Documentar a pré-verificação de aprovações legadas e o aceite HTTP local como etapa intermediária. A próxima fatia entrega a prova de ingresso e sincroniza a confiança entre servidores. A validação ponta a ponta exige pelo menos três servidores.

## Limites

A fatia integra comportamento existente e preserva as duas fronteiras de autorização. Entrega e propagação de certificados, revogação assinada distribuída, quórum, configuração remota, replicação e relay seguem nas fatias dependentes.
