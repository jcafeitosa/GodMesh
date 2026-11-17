# Registro consolidado de gaps — GodMesh

> **Documento de trabalho do CTO.** Data: 17/11/2026. Consolida os pareceres dos pods G1–G5, dos críticos C1–C4 e dos poderes P1–P8. **Nada aqui aprova SPEC, implementação ou resultado de teste.** Cada gap tem dono, ação e critério de fechamento; "aberto" significa sem ação concluída.

## 1. Método

Fonte: 8 threads de descoberta (`G1` produto/expansão, `G2` integrações, `G3` defeitos documentais, `G4` escala/resiliência, `G5` segurança/conformidade) e 3 debates (`D1` compatibilidade, `D2` autoridade/recuperação, `D3` E2EE/offline/abuso), mais os pareceres anteriores de P1–P8 e C1–C4. Cada parecer exigiu evidência citada ou marcação explícita de "não encontrado".

Classificação usada: **falha** (contradição ou insuficiência frente a regra existente) · **lacuna** (contrato necessário ausente) · **melhoria** (reforço do que existe) · **expansão** (capacidade fora do escopo descrito) · **integração** (sistema externo novo).
Severidade: **crítica** bloqueia produto ou segurança · **alta** bloqueia escopo declarado · **média** degrada qualidade · **cosmética**.

## 2. Resumo por bloco

| Bloco | Gaps registrados | Críticos | Alta | Ação do CTO |
|---|---|---|---|---|
| G1 Produto e expansão | 13 | 4 | 6 | Decidir oferta mínima viável de produto (ADR pendente) |
| G2 Integrações e API | 33 | 4 | 14 | Priorizar 6 integrações de v1; resto backlog |
| G3 Defeitos documentais | 15 | 0 (2 bloqueiam aprovação) | 4 | 8 corrigidos nesta rodada; 7 atribuídos a autores |
| G4 Escala e resiliência | 27 | 9 | 12 | Perfis de capacidade + 6 experimentos obrigatórios |
| G5 Segurança/conformidade | 25 | 9 | 16 | 9 decisões de política para o usuário; 6 baselines técnicos |

## 3. Decisões tomadas nesta rodada (ADR-056…063)

Registradas em [Decisões](../06-planejamento/decisoes.md) com evidência e critério de verificação: compatibilidade N/N-1 por perfil (056), posição canônica de evento/cursor (057), taxonomia de ACK por fronteira (058), escopo do grant offline (059), confidencialidade E2EE e proibição de alegação (060), fronteira transacional e recuperação (061), retenção fail-closed (062), cadeia de suprimento e atualização assinada (063).

## 4. Gaps que eu fecho agora (correção aplicada)

| ID | Gap | Correção aplicada |
|---|---|---|
| G3-01 | Snapshot de hashes divergente em 15/21 | Tabela de `SPECS.md` regenerada em 17/11/2026, com aviso de que hash não é revisão |
| G3-10 | Links `/Users/...` em `SPECS.md` | Substituídos por referência textual a fonte externa não versionada |
| G3-09 (parte) | Caminhos absolutos em `aceites-comportamentais.md` | Normalizados para `docs/...` (portável) |
| G3-08 | `credenciais-crowdfind.md` órfão do índice | Indexado no README como contrato ativo condicionado a SPEC de CrowdFind |
| G3-15 | `aceites-comportamentais.md` sem link nominal | Indexado no README |
| G3-04 | Catálogo diz "A conciliar" e matriz nova diz cobertura | Registrado como divergência explícita nos dois documentos; conciliação linha a linha atribuída a W1/C1 |
| G1/G2/G4/G5 | Listas completas | Consolidadas aqui e nos pareceres das threads |

## 5. Decisões do usuário — resolvidas em 17/11/2026

| # | Decisão | Resolução | ADR / documento |
|---|---|---|---|
| U1 | Mercados e jurisdições | Operação mundial; região primária Brasil (São Paulo), secundária Colômbia (Bogotá); contrato declara a região que atende cada país | ADR-066 |
| U2 | Prazos de retenção por classe | Tabela aprovada pelo CTO com critério de minimização (mensagem 24 m, posição 30 d, áudio não retido, SOS 12 m, auditoria 12/24 m, backup 35 d) | ADR-065 + `04-seguranca/retencao-revogacao.md` |
| U3 | ASR no servidor vs E2EE | ASR opt-in por canal, com E2EE retirado e visível | ADR-060 |
| U4 | Monetização, planos e limites | segue aberto; não bloqueia técnica, bloqueia cadastro público | G1-04 |
| U5 | Menores de idade | público 18+ no v1; menor exige desenho próprio | ADR-066 + G5-10 |
| U6 | Janela N/N-1 | 12 meses para clientes, 24 para firmware; revisar com custo medido | ADR-056 |
| U7 | Escopo de emergência | nunca alegar serviço público; integração só por acordo formal | ADR-039/064 |
| U8 | SOS offline sem raiz de confiança | negar e informar na UI; oferecer discador do sistema e caminho online | ADR-064 |

## 5b. Bloqueios que exigem decisão do usuário (histórico)

| # | Decisão | Recomendação do CTO | Evidência |
|---|---|---|---|
| U1 | Mercados e jurisdições iniciais | Começar por **um** mercado com operação própria, sem prometer emergência pública | G5-06, G5-09, G5-13 |
| U2 | Prazos de retenção por classe | Aprovar tabela mínima por finalidade (mensagem, posição, auditoria, mídia, ASR) | ADR-062, SPEC-005 §10, G5-24 |
| U3 | ASR no servidor vs E2EE | ASR **opt-in por canal**, com E2EE retirado e visível | ADR-060, SPEC-008, G5-20 |
| U4 | Monetização, planos e limites | Não é bloqueio técnico; decidir antes de abrir cadastro público | G1-04 |
| U5 | Menores de idade e contas familiares | Declarar público 18+ no v1; tratar menor só com desenho próprio | G5-10 |
| U6 | Janela de suporte N/N-1 | 12 meses para clientes, 24 para firmware; revisar com custo medido | ADR-056 |
| U7 | Escopo de emergência | Nunca alegar serviço público; integração só por acordo formal | G5-13, ADR-039 |

## 6. Produto — G1 (13)

| ID | Gap | Classe | Sev. | Ação | Dono |
|---|---|---|---|---|---|
| G1-01 | Cadastro, primeiro acesso e recuperação de conta sem jornada | lacuna | crítica | Escrever jornada + SPEC-022 | P5/Margaret |
| G1-02 | Ciclo de vida de organização e convites (Teams) | lacuna | alta | SPEC-022 §organizações | P5 |
| G1-03 | Papéis, administração e governança de Teams | lacuna | crítica | Catálogo de papéis + autorização | P2/P5 |
| G1-04 | Planos, faturamento e limites comerciais | expansão | alta | Decisão de produto (U4) | usuário |
| G1-05 | Portal de organizações e autoatendimento | expansão | alta | Backlog pós-v1 | P7 |
| G1-06 | Suporte, contestação e disputa | lacuna | alta | Fluxo + SLA mínimo | P5 |
| G1-07 | Catálogo de dispositivos e compatibilidade visível | lacuna | média | Derivar de capacidades (RF-003) | P6 |
| G1-08 | Preferências globais de notificação | lacuna | média | SPEC-013 delta | P7 |
| G1-09 | Importação/exportação e portabilidade | lacuna | alta | Depende de U2 | P3 |
| G1-10 | Exclusão e encerramento de conta/organização | lacuna | alta | Depende de U2 | P3/P5 |
| G1-11 | Uso compartilhado de veículo/ativo | expansão | média | Modelo de posse múltipla | P3 |
| G1-12 | Busca e navegação em histórico | lacuna | média | Depende de E2EE (U3) | P3/P7 |
| G1-13 | Idiomas e localização da experiência | lacuna | média | i18n + RTL desde o início | P7 |

## 7. Integrações — G2 (33, agrupados)

| Grupo | IDs | Achado central | Ação |
|---|---|---|---|
| Mensageria/IoT externo | G2-01…07 | MQTT existe como menção; Home Assistant, Traccar, Matrix, XMPP, SIP e WebRTC gateway não têm contrato; APRS só citado | v1: MQTT + APRS-RX + WebRTC. Resto backlog |
| API pública | G2-08, G2-27…33 | Sem sandbox, webhooks, rate limit, deprecação, chaves, SDK gerado; `202` não prova entrega | Escrever contrato público antes de abrir API |
| Identidade empresarial | G2-09, G2-10 | OIDC parcial; SAML e SCIM ausentes → contas órfãs após desligamento | SCIM é P1 para Teams |
| Operação e auditoria | G2-11, G2-15 | Sem exportação SIEM nem contrato de object storage (chave, ACL, ciclo de vida) | P1 |
| Cliente e distribuição | G2-12, G2-13, G2-16 | Mapas offline, APNs/FCM e e-mail sem contrato; push não pode virar prova de entrega | P1 |
| Monetização | G2-14 | Pagamentos ausentes (depende de U4) | backlog |
| UDNL/Bridge SDK | G2-17…26 | Proveniência de áudio, cadeia de duas bridges, versionamento de SDK, quotas e observabilidade por bridge sem contrato | P0 para v1 |

## 8. Defeitos documentais — G3 (15)

Corrigidos nesta rodada: G3-01, G3-08, G3-09 (parcial), G3-10, G3-15.
Atribuídos aos autores com prazo de próximo delta: G3-02 (cabeçalhos sem data/versão/revisor: SPEC-002, 006, 010, 014, 015, 021), G3-03 (pareceres vinculados a versão anterior: SPEC-007, 012, 018, 019), G3-05 (matriz atribui cobertura que a SPEC não reivindica — RF-034), G3-06 (ACK duplicado sem autoridade única), G3-07 (enums repetidos sem tabela de autoridade), G3-14 (IDs de caso em cinco famílias).
Sem defeito reproduzido: G3-11, G3-12, G3-13.

## 9. Escala e resiliência — G4 (27)

| Faixa | IDs | Conteúdo |
|---|---|---|
| Pontos de ruptura | G4-01…08 | Fan-out de PTT, custo de replicação por tenant, cardinalidade QUIC, PostGIS, hot partitions, filas/backpressure, snapshot/replay, cardinalidade de métricas |
| Falhas de modo | G4-09…15 | Split brain com fencing parcial, nó saudável saturado, perda regional com mídia ativa, upgrade misto em pico, restore com Inbox em voo, relógio divergente, DDoS de SOS |
| Capacidade | G4-16…18 | Faltam perfis Personal/Teams/Node/Platform, método de medição e explicitação do que não pode ser prometido |
| Operação | G4-19…22 | Runbooks, plantão, ciclo de chaves/certificados, upgrade/downgrade, quotas, DR não testado |
| Melhorias | G4-23…27 | Cache de presença, coalescência de telemetria, sharding por tenant, regionalização de evento, backpressure por classe |

**Experimentos obrigatórios antes de qualquer promessa de capacidade:** (1) carga de PTT com salas representativas; (2) replicação com tenant dominante; (3) churn de conexões QUIC IPv6/IPv4; (4) PostGIS com dataset denso; (5) failover regional com sessão de mídia ativa; (6) restore com Inbox em voo. Nenhum executa antes de SPEC aprovada e ambiente autorizado.

## 10. Segurança e conformidade — G5 (25)

| Faixa | IDs | Conteúdo | Depende do usuário |
|---|---|---|---|
| Abuso por ator autorizado | G5-01…05 | Coerção em Guardian, SOS falso, assédio/moderação, inferência por consulta repetida, vigilância por empregador/Estado | Sim |
| Conformidade | G5-06…14 | Jurisdição e base legal, DSR/portabilidade, hold, transferência internacional, menores, licenças RF, eCall/112, acessibilidade | Sim (com revisão jurídica) |
| Plataforma | G5-15…21 | Gestão de chaves/KMS, segredos de bridge, SBOM/provenance, release assinado, isolamento por tenant (RLS), perfil SRTP, ciclo de vida de dispositivo | Parcial |
| Falha de política | G5-22…25 | "Piso global", "revogação imediata", retenção de auditoria, consentimento genérico | Sim |

Fechados tecnicamente nesta rodada: G5-17 e G5-18 (ADR-063), G5-20 (ADR-060), G5-19 (ADR-061 + RLS como defesa em profundidade, nunca isolamento único), G5-22/G5-23/G5-24 (ADR-060/062).

## 11. Ordem de execução aprovada pelo CTO

1. **Fechar os 8 defeitos atribuídos** (G3-02, 03, 05, 06, 07, 14) nos próximos deltas de SPEC.
2. **Três SPECs novas**: SPEC-022 produto/organizações, SPEC-023 capacidades e conectividade, SPEC-024 CrowdFind (resolve RF-003, RF-030…033 sem SPEC canônica).
3. **Aplicar ADR-056…063** nos textos de SPEC-001, 002, 003, 004, 005, 006, 007, 008, 018, 019, 020, 021.
4. **Re-revisão por hash** de cada SPEC alterada por crítico diferente do autor.
5. **Aprovação e implementação** do Programa A só depois de 1–4; nenhum código antes disso.
6. **Integrações de v1**: MQTT, APRS-RX, WebRTC, SCIM, object storage, SIEM. Demais no backlog documentado.

## 12. Não verificado

Nenhuma SPEC aprovada; nenhum código; nenhum teste executado; nenhum número de capacidade medido; nenhuma fonte legal consultada; nenhum acesso a hardware, operadora, satélite ou serviço externo. Gaps marcados como "não encontrado" significam ausência nos documentos lidos, não afirmação sobre sistemas externos.

## 13. Atualização 17/11/2026 — o que foi fechado depois da varredura

| Item | Estado | Evidência |
|---|---|---|
| ADR-056…063 aplicadas nos textos das 24 SPECs | aplicado | seções "Mudanças desta versão" em cada arquivo; `SPECS.md` com hash por arquivo |
| SPEC-022, SPEC-023, SPEC-024 criadas | aplicado | fecham a lacuna de SPEC canônica de RF-020, RF-003, RF-030, RF-031, RF-032, RF-033 |
| Veto C4 — cursor sem partição | fechado | `07-contratos/eventos-sincronizacao.md` v0.2 com tupla canônica e rejeição explícita |
| Veto C4 — confidencialidade herdada | fechado | `07-contratos/autoridade-contratos.md` §3, com falha fechada na ausência do metadado |
| Veto C4 — SOS descartado por fail-closed | fechado | SPEC-006 DRAFT-0.5, classe mínima de emergência e falha observável |
| Veto C4 — grant offline repetível (clone/restore) | fechado | SPEC-002 DRAFT-0.5 §6.3, alta marca na autoridade, negação sem fonte confiável |
| Veto C4 — rollback para release vulnerável | fechado | ADR-063 emendada (`generation` + piso assinado) e SPEC-017/019 |
| Veto C3 — casos de teste por decisão | aplicado nas 24 SPECs | seção "Casos por decisão (ADR-056…063)" em cada arquivo |
| Divergências de vocabulário D1/D4 | fechadas; D2, D3, D5–D9 abertas | `07-contratos/autoridade-contratos.md` §4 |
| Aprovação de SPEC | **não** | exige revisão independente por hash das versões atuais |
| Implementação | **não iniciada** | gate de planejamento e aprovação integral segue em vigor |
