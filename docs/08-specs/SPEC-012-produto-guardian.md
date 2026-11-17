# SPEC-012 — Produto, contexto e Guardian

**Versão:** DRAFT-0.8 · **Data:** 2026-11-17 · **Estado:** DRAFT/OPEN · **Autora documental:** Margaret Hamilton · **Revisão independente:** Ada Lovelace, revisão do delta desta versão solicitada; pendente. Não aprovada, implementada nem validada. Casos abaixo são planejados e `NOT_EXECUTED`; identificadores de teste formais `NOT_ASSIGNED`.

## 1. Fontes e cobertura

`00-produto/visao.md:3–20,34–45`; `00-produto/requisitos.md:3,26,56–57`; `02-dominio/contexto-operacional.md:3–22,37–38`; `02-dominio/guardian-acompanhamento.md:3–34,47–51`; `07-contratos/guardian-crowdfind.md:1–15,40–46` (contrato documental, não API comprovada); `06-planejamento/decisoes.md` ADR-004, 010, 023, 030, 039, 042, 047 e 055; `06-planejamento/matriz-rastreabilidade.md` linhas individuais e conciliações; `06-planejamento/pendencias.md` §Direções de produto transmitidas. A prevalência normativa segue `README.md` e arquitetura de referência. Direções recentes foram comunicadas por Ada; a coordenação registra não ter acessado a resposta original.

Baseline de autoria: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-margaret-hamilton.md`, SHA-256 `be5222e988d2bb220c9065c17b752e371d924aa12bd2660f90842bed863fcc21`. Confronto de interface: `SPECS-EM-ANDAMENTO/contribuicao-nancy-leveson.md` §Interfaces RF-028 e RNF-016, que preserva atraso distinto de SOS; revisão independente de DRAFT-0.1 e rechecagem de DRAFT-0.2 recebidas em `SPECS-EM-ANDAMENTO/REVISAO-SPEC-012-ADA.md`; rechecagem focal DRAFT-0.3 recebida em `SPECS-EM-ANDAMENTO/REVISAO-SPEC-012-ADA.md`; DRAFT-0.5 tinha revisão pendente à época; esta versão DRAFT-0.6 também aguarda revisão independente.

| Requisito | Subescopo nesta versão | Limite |
|---|---|---|
| RF-020 | Jornadas de ofertas Personal, Teams e Node e fronteira operacional | Cobertura parcial: jornadas e modalidades iniciais nesta SPEC; matriz de capacidades/serviços e critérios por oferta permanecem OPEN. Platform/SDK está na visão, não é parte literal de RF-020; detalhes de operação e estimativa estão em SPEC-019 |
| RF-027 | Contexto selecionado por atividade, desconhecido e sugestão | Taxonomia final, conflitos e expiração OPEN |
| RF-028 | Sessão Guardian, audiência, check-in, expiração e revogação | Política de cópias, atraso e sincronização OPEN |

Interfaces: RF-009/021/026/029/030/034 e RNF-003/012/017 pertencem também a outros contratos/SPECs; referências não comprovam sua cobertura integral. Identidade e autorização pertencem à SPEC-002, sync à SPEC-004, SOS à SPEC-006, localização à SPEC-011, UX à SPEC-013, offline à SPEC-014 e política de segurança à SPEC-020; vínculos de integração são planejados e dependem de conciliação dos respectivos autores.

## 2. Atores, autoridade e escopo

Pessoa titular cria e encerra a própria sessão; destinatário convidado aceita escopo e período delimitados; serviços validam identidade, autorização e política antes de permitir ações. Administrador de organização atua dentro de sua autoridade e não elimina autorização ou transparência exigidas pelo Guardian. Provedor opera a camada hospedada; cliente/operador a instalação autogerida conforme direção relatada em `pendencias.md`; limites por oferta, terceiros e suporte continuam OPEN. Identificadores apresentados pelo cliente não são prova de autoridade. A ausência de evidência de permissão não concede acesso.

## 3. Ofertas e contexto

**Pré-condição:** identidade e oferta selecionadas; capacidades, conectividade e permissões estão conhecidas ou explicitamente desconhecidas. **Estímulo:** selecionar atividade ou receber sugestão por sensores/perfil. **Resultado:** seleção manual prevalece; sugestão é identificada como tal e não comprova papel, local, licença ou direito de transmissão RF. Sem contexto, `unknown` mantém funções fundamentais elegíveis. Contexto não transmite posição nem inicia SOS por si. Ao negar permissão, apresentar função afetada, razão disponível e alternativa segura; outras funções independentes mantêm sua própria elegibilidade. Uma bridge indisponível não determina por si o estado do núcleo nativo. Offline, indicar dados autorizados, idade e pendências sem afirmar entrega; na recuperação revalidar política e executar reconciliação sob contrato de sync.

A atividade é temporária, não atributo permanente da pessoa. O contrato de contexto documenta `environment`, `activity`, `source`, tempos e `policy_ref` (`guardian-crowdfind.md:5–6`); a interpretação de origem, vencimento, conflitos entre dispositivos e taxonomia final depende de OPEN-02. Não introduzir wire/schema adicional nesta SPEC.

## 4. Guardian: estados, transições e dados

Estados documentados: `draft`, `consent_pending`, `scheduled`, `active`, `paused`, `completed`, `expired`, `revoked`; conectividade separada: `online`, `offline_buffering`, `stale`, `unknown`. O contrato de eventos registra identificador, estado, conectividade e tempo (`guardian-crowdfind.md:8–15`). As setas resumidas da origem não definem todas as transições: não inventar transição válida para estados terminais até conciliação.

| Etapa | Pré-condição e ação | Resultado e limites |
|---|---|---|
| Preparar | Titular escolhe finalidade, prazo, audiência e precisão; permissões são independentes | Rascunho não concede leitura; convite autenticado contém finalidade, prazo e escopo |
| Autorizar | Destinatário aceita e grant está vigente; horário e permissões permitem operação | Somente destinatário e escopo autorizados podem consultar; ausência de prova ou prazo vencido nega acesso |
| Acompanhar | Sessão ativa e fonte permitida | Exibir origem, idade e incerteza disponíveis; sem prova de posição atual, exibir desconhecido/desatualizado, não criar posição |
| Check-in | Titular realiza ação ou prazo vence | Registrar confirmação observada; `overdue` indica ausência de confirmação, não SOS, entrega garantida ou atendimento humano |
| Pausar/terminar | Ação autorizada, expiração ou revogação | Impedir novas leituras onde a revogação é conhecida; sinalizar propagação não comprovada e tratar cópias por política aprovada |
| Partição/retorno | Rede indisponível e depois recuperada | Fila local somente dentro das autorizações vigentes; não afirmar entrega. Revalidar autorização, expiração e retenção antes de divulgar ou reconciliar |

Dados mínimos conceituais: referência pseudônima do titular e sessão, destinatários e grants, finalidade, janela, precisão, check-ins, observações com origem/tempo/incerteza e referências de política; não expor coordenadas precisas no canal público. Beacon tem consentimento separado e não anuncia identidade ou coordenadas. Não presumir purga de cópia externa nem revogação instantânea global. Detalhes de armazenamento, formato, segurança, cópias, garantias de tempo e erros de wire são interfaces pendentes dos autores respectivos.

## 5. Segurança, falhas e acessibilidade

Precisão e audiência seguem finalidade e menor privilégio; uso de localização, histórico, notificação e Beacon requer permissões próprias. Revogação offline pode não ter chegado à outra parte: apresentar incerteza e negar operações sensíveis com autoridade `stale/unknown`, segundo política conciliada. A direção relatada admite grant offline assinado e limitado para ações locais autorizadas; duração, frescor e operações são OPEN, não autorização irrestrita. Falha de autorização, destinatário sem aceite, grant expirado, erro de fonte ou cópia desatualizada não podem aparecer como sucesso. Repetição, perda e recuperação exigem idempotência e evidência sob SPEC-004, ainda não homologada.

Revogar, encerrar, verificar atraso e compreender permissões devem ser acessíveis por alternativas equivalentes; UX e critérios por leitor de tela, teclado e outras necessidades dependem da SPEC-013. O app não controla serviços de emergência ou configurações de terceiros sem base autorizada; não promete socorro ou entrega.

## 6. Aplicabilidade

Servidor em contêiner Linux aplica contratos e autorização para as modalidades aprovadas; não apresenta UI de pessoa como obrigação de servidor. macOS universal arm64/x86_64 e iOS SwiftUI, Windows, Linux desktop e Android mantêm jornadas no escopo; disponibilidade de sensores, execução em segundo plano, notificação e acessibilidade deve ser documentada por plataforma, com fallback e sem alegação de compatibilidade testada. Web: aplicabilidade de cada jornada e sensor a decidir com Lea e a visão vigente; não se exclui globalmente nem se afirma plataforma aprovada pela ADR-055. Dispositivo sem sensor pode continuar funções independentes sem inventar evidência.

## 7. Critérios e casos planejados

Cada caso depende de contrato de identidade, política e plataforma apropriados; preparação, ambiente e IDs formais serão atribuídos no plano de testes. Nenhum caso foi executado.

1. **RF-020:** oferta e autorização válidas, com bridge externa indisponível: operação nativa independente mantém seu estado conforme dependências próprias; interface mostra falha da bridge sem declarar estado de núcleo apenas por inferência.
2. **RF-027:** contexto manual entra em conflito com sugestão de sensor: manual prevalece; sugestão permanece identificada; nenhuma permissão RF ou SOS é gerada. Sem sensor, `unknown` não bloqueia funções fundamentais.
3. **RF-028 positivo:** convite aceito, escopo/tempo/permissão vigentes e sessão ativa: apenas destinatário autorizado consulta precisão concedida; posição informa origem, idade e incerteza quando disponíveis.
4. **RF-028 negativo:** convite pendente, grant vencido ou revogação conhecida: leitura é negada; não revelar posição. Revogação não confirmada remotamente é informada como pendente/desconhecida.
5. **RF-028 atraso:** prazo decorrido sem check-in: aviso de ausência de confirmação conforme política e destinatários autorizados; não há incidente SOS automático. Confirmação técnica não representa atendimento humano.
6. **RF-028 recuperação:** partição e retorno: fila local não é mostrada entregue; antes de divulgar, revalidar autorização, expiração e retenção; informação antiga continua identificada como tal.
7. **Permissões e acessibilidade:** localização negada ou sensor ausente: explicar capacidade afetada, preservar funções independentes e oferecer encerramento acessível; ativar Guardian não implica consentimento Beacon.

## 8. Decisões OPEN e condição de fechamento

| ID | Dono documental / interface | Alternativas e consequência | Fechamento |
|---|---|---|---|
| OPEN-01 | Margaret + Grady, produto | Catálogo comum com permissões por perfil versus catálogo configurado por oferta; afeta suporte, custo e jornadas. Recomenda-se catálogo comum com diferenças explícitas, sem impor quotas. | Mapa por oferta e modalidade, limites e responsabilidades aprovado e rastreado a RF-020. |
| OPEN-02 | Margaret + Lea e domínio | Taxonomia fechada versus núcleo versionado extensível; recomendação provisória extensível sem conferir privilégio. | Definir precedência, conflitos, expiração e transições por sessão e plataforma, revisados com aceite. |
| OPEN-03 | Margaret + Nancy | Tolerância, destinatários, repetição e canal de alerta configuráveis por sessão ou política organizacional limitada pelo consentimento. | Política aprovada e estados/negativas reconciliados com SOS e sync; sem tratar atraso como SOS. |
| OPEN-04 | Bruce + Kim + Alex + Nancy | Grant limitado e retenção por finalidade/cópia: duração, frescor, operações e efeitos sob partição; alternativas devem respeitar autoridade e negativas vigentes. | Matriz operação × autorização × cópia aprovada; risco e tratamento de revogação revisados. |
| OPEN-05 | Lea + Chet + Donny + Alex | Capacidades e fallback por plataforma; sem pressupor suporte de APIs. | Matriz de elegibilidade, acessibilidade e fontes oficiais revisada, com casos planejados por plataforma. |
| OPEN-06 | Margaret + Ada, com autores transversais | Cobertura nominal versus cobertura semântica requisito→contrato→aceite→teste. | Registrar versões e revisão independente das interfaces e fechar lacunas do índice; não afirmar que esta SPEC cobre os demais requisitos. |
| OPEN-07 | `NOT_ASSIGNED` — dono a atribuir pelo usuário; relacionado a produto e privacidade do Beacon | Política do Beacon durante pausa: se permanece ativo, se emite, e como o cliente distingue pausa solicitada de pausa confirmada. Alternativas não escolhidas nesta SPEC. | Política aprovada para o estado `paused`, com casos de pausa e retomada rastreados em §7. |
| OPEN-08 | Kim Cameron + Martin Kleppmann (conforme §9 e S012-A3) | Ordem, assinatura, tempo e transições da sessão, incluindo término antes do início. Não escolhe se cancelamento vira estado novo, qual relógio é autoritativo, nem como vence o grant. | Contrato de autoridade e eventos aprovado, definindo transições assinadas, ordenação e relógio autoritativo. |

**Dependências de fechamento:** conciliar atualizações das SPECs 002/004/006/011/013/014/020, decisões de produto, limites operacionais e revisão independente. O rascunho não aprova nenhum candidato, prazo ou gasto. Revisão documental não substitui teste futuro ou prova de implementação.


## 9. Resposta à revisão independente de DRAFT-0.1

Fonte: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/REVISAO-SPEC-012-ADA.md`, alvo SHA-256 `ad8ebd982f2ba9c4ff1f63f3b948631992ac919f35eccd3a11f5a3de210fff17`. Abaixo são propostas sujeitas a revisão, não fechamento automático.

### S012-A1 — término e Beacon

Em `completed`, `expired` ou `revoked`, a sessão deve cessar novas consultas e desativar o Beacon associado, conforme `02-dominio/guardian-acompanhamento.md:22–28,36–37`. O cliente deve distinguir solicitação de desativação, confirmação local observada e estado desconhecido; não afirmar desativação remota instantânea. Para cada terminal, planejar caso com Beacon ativo: aplicar término autorizado/expiração/revogação, verificar comando e confirmação local conforme capacidade; se indisponível ou confirmação ausente, apresentar desconhecido e não alegar emissão encerrada remotamente. Em `paused`, comportamento do Beacon permanece OPEN-07, pois a regra de terminal não define pausa. OPEN-07: Margaret e especialistas de plataforma propõem pausa com suspensão ou manutenção sob consentimento explícito; fechar com política, feedback e revisão de privacidade.

### S012-A2 — matriz inicial de jornadas por oferta

Esta matriz deriva apenas das capacidades citadas em `00-produto/visao.md:10–16,34–39`; não atribui exclusividade, quotas ou disponibilidade operacional. Modalidades hospedada e autogerida seguem fronteiras da decisão relatada em `06-planejamento/pendencias.md` §Direções de produto; aplicabilidade de cada modalidade por oferta permanece OPEN-01.

| Oferta / ator | Jornada planejada e resultado positivo | Negativo / degradação |
|---|---|---|
| Personal / titular e pessoa autorizada | Pessoa cadastra dispositivo e compartilha posição consentida com destinatário autorizado; consulta mostra qualidade/origem quando disponível. | Sem consentimento, grant ou fonte válida, negar divulgação ou mostrar desconhecido; offline não implica entrega. |
| Teams / participante, administrador autorizado | Organização configura equipe, ativos e ACL; participante autorizado usa comunicação e consulta escopo permitido; auditoria segue política. | Pessoa fora do tenant ou escopo não acessa dados; administração não substitui consentimento Guardian. |
| Node / operador autorizado | Operador habilita nó regional/edge com capacidades elegíveis e ingresso autorizado; estado mostra serviço disponível somente com evidência. | Ingresso sem autorização é recusado; perda de conectividade mostra limite de operação e reconciliação pendente. |

Casos planejados: executar cada jornada no cenário autorizado e repetir com permissão negada, capacidade ausente e partição; verificar resultado descrito e isolamento de escopo. Atribuição de plataforma, modalidade, contratos, IDs de caso, operador e aceite específico aguardam revisão de Grady/Ada e OPEN-01/05. Platform/SDK tem oferta própria na visão e interface RF-019, sem ampliar RF-020 por presunção. Cobertura RF-020 continua parcial até matriz integral e revisão.

### S012-A3 — transições e eventos

A tabela abaixo é proposta de comportamento de produto, não enumeração normativa exaustiva. Autoridade e ordem verificáveis pertencem às SPECs 002/004; `07-contratos/guardian-crowdfind.md:40–44` exige revisão de sessão e grant assinada e idempotente por `event_id`. Fonte dos estados: `02-dominio/guardian-acompanhamento.md:8–10,22–34`. O tratamento específico de conflitos, tempo e replay permanece OPEN-08, sem afrouxar revogação e privacidade.

| Origem | Estímulo e autoridade | Destino proposto / negativa |
|---|---|---|
| draft | Titular autorizado envia convite válido | consent_pending; convite sem escopo ou autoridade não concede acesso. |
| consent_pending | Consentimentos e agendamento necessários são válidos | scheduled; aceite ausente não concede leitura. |
| scheduled | Início elegível e política vigente | active; sem condição necessária permanece pendente ou falha explícita, conforme contrato. |
| active | Titular autorizado pausa | paused; política do Beacon em pausa OPEN-07. |
| paused | Titular autorizado retoma dentro de prazo e com autorização vigente | active; com grant vencido ou revogado não retoma. |
| active / paused | Titular autorizado conclui, prazo expira ou autoridade válida revoga | completed / expired / revoked conforme evento; bloquear novas leituras sob estado conhecido e desativar Beacon da sessão. Sem prova ou em partição, comunicar estado desconhecido. |
| terminal | Evento antigo, duplicado ou pedido de retomada | Não reabrir por evento atrasado; duplicação não repete efeitos. Não inferir recuperação do histórico nem nova sessão sem contrato autorizado. |

Casos planejados adicionais: agendamento até início elegível; pausa e retomada autorizada; expiração automática sem conexão, com confirmação local e estado remoto desconhecido; negativa de retomada após revogação; entrega duplicada de mesmo `event_id` e evento antigo após estado terminal sem reativar acesso/Beacon. Resultado desconhecido permanece desconhecido onde autoridade/ordem não é demonstrada. OPEN-08: Margaret, Kim e Kleppmann conciliam ordenação, precedência, transições omitidas e tratamento de relógio, incluindo revogação, expiração e cancelamento antes do início em `consent_pending` e `scheduled`; até decisão não presumir destino, acesso ou ativação de Beacon; fechar com contrato assinado/idempotente e casos negativos revisados, sem estabelecer mecanismo ou prazo por esta tabela.

**Estado após resposta:** S012-A1/A2/A3 tratados no texto para revisão de delta, sem alegar fechamento ou aprovação. Todos os casos seguem planejados `NOT_EXECUTED`; revisão por Ada e conciliação dos donos continuam pendentes.

Rechecagem Ada de DRAFT-0.2: S012-A1 atendido textualmente; S012-A2 permanece parcial; S012-A3 mantém OPEN-08 para ordem, assinatura, tempo e transições, incluindo término antes do início. Fonte `SPECS-EM-ANDAMENTO/REVISAO-SPEC-012-ADA.md` §Rechecagem DRAFT-0.2, alvo SHA-256 `bb9f4067c12f8a27ad176ef7a359ce5fbe05c038296eccf25650de7dec0714c3`. Esta atualização não constitui aprovação nem teste.


## 10. Integração da revisão Ada e delimitação da cobertura — DRAFT-0.4

A rechecagem focal de Ada sobre DRAFT-0.3 (SHA-256 `da9ff208c928d998f8f7baf6cf03fb71abdc9333c4459959e8c556b0c06199b2`) confirmou as correções de histórico e dos estados pré-início, mantendo S012-A2/A3 parciais. Fonte: `SPECS-EM-ANDAMENTO/REVISAO-SPEC-012-ADA.md`, §Rechecagem focal DRAFT-0.3. Esta seção responde aos resíduos sem declarar aprovação.

**RF-020 e fronteira com SPEC-019.** SPEC-019 §Oferta/modalidades (linhas 27–29), quadro de capacidade (70–75) e casos S019-09/10 (97–98) propõem modalidade hospedada/autogerida nas quatro ofertas e explicam fronteiras por camada. Isto melhora o insumo da jornada, mas não fecha uma matriz de funções/capacidades exclusivas/inclusivas, contratos, limites ou critérios de completude por oferta; custos, suporte, terceiros e demanda permanecem desconhecidos/OPEN naquela SPEC. Integração de produto nesta SPEC: Personal focaliza titular e pessoas autorizadas em dispositivo/localização consentida; Teams focaliza equipe, ACL, ativos e auditoria; Node focaliza operador, capacidade e rede/região autorizadas. Critérios negativos por tenant, permissão e evidência continuam nos casos §9. As combinações por modalidade e atores de operação devem seguir SPEC-019 depois de revisão de seu conteúdo. Assim RF-020 é parcial, sem contagem como cobertura integral.

**Matriz residual e condição de fechamento RF-020.** Para cada oferta, falta inventário que relacione jornada/ator → função e capacidade → contrato/serviço → autorização/dado → modo offline/falha → plataforma elegível → critério positivo e negativo. Responsáveis propostos: Margaret por jornada e semântica do produto; Grady por combinação modalidade/operação/capacidade; donos das SPECs de serviço por contrato e limite. Fechar após a matriz ser incorporada, revisada por Ada, reconciliada ao índice sem sobreposição falsa e sem inventar quotas. A existência de quatro nomes de oferta ou casos exemplares não prova integralidade.

**S012-A3, transições antes de início e efeitos idempotentes.** A SPEC registra que os casos antes do início abrangem revogação, expiração e cancelamento em `consent_pending`/`scheduled`, sem destino presumido. Acrescenta-se a matriz observável de produto:

| Estado | Estímulo autorizado/observado | Resultado esperado proposto | Negativo |
|---|---|---|---|
| consent_pending | titular cancela ou grant de convite expira/revoga | novas leituras não são concedidas; informar cancelamento/expiração conforme evidência disponível | evento sem autoridade ou duplicado não concede leitura |
| scheduled | titular cancela antes de `effective_at` ou autorização vence | sessão não se torna ativa por relógio isolado; refletir estado apenas quando transição válida for comprovada | evento de início atrasado, duplicado ou fora de ordem não pode ressuscitar sessão cancelada/revogada |
| completed/expired/revoked | pedido atrasado de início/retomada ou replay anterior | terminal continua terminal; nenhum acesso/Beacon é reativado implicitamente | estado remoto/ordem desconhecidos continuam `unknown`, sem alegar confirmação |

Estas são expectativas de produto, sujeitas à resolução de conflito pelo contrato de autoridade/eventos. Não escolhem se cancelamento vira novo estado, como vence grant, qual relógio é autoritativo ou quando a confirmação remota é alcançada. OPEN-08 permanece com Kim/Kleppmann para transições assinadas, ordenação, idempotência, efeito em voo e prova do terminal; Margaret mantém estados e observáveis da jornada. Fechamento requer tabela canônica de transições e negativos reconciliada com SPEC-002/004 e aceite Guardian; falta de consenso mantém este recorte parcial.

**Casos planejados incrementais:** (a) revogar ou cancelar enquanto `consent_pending`, com tentativa subsequente de leitura; (b) cancelar/expirar enquanto `scheduled`, seguido de evento `active` atrasado; (c) reenviar o mesmo `event_id` e verificar que nenhum efeito adicional concede acesso nem reativa Beacon; (d) replay antigo após terminal preserva terminalidade; (e) para cada oferta, percorrer jornada aprovada e contrastar autorização válida com ausência de autorização/capacidade. Predicados específicos, autoridade, fixtures e IDs centrais continuam NOT_ASSIGNED; resultados NOT_EXECUTED.

**Estado de revisão e cobertura.** Ada rechecagem DRAFT-0.3 confirma textualmente duas correções e preserva A2/A3 parciais; detalhes recebidos foram incorporados como propostas rastreáveis. A revisão citada de DRAFT-0.5 é de versão anterior; revisão independente do DRAFT-0.6 solicitada por hash ao revisor designado permanece pendente. RF-027 tem direção e casos propostos, com taxonomia/precedência multi-device ainda OPEN. RF-028 cobre parcialmente sessão, convites, atraso, estados terminais e Beacon; retenção, sync, política offline, estado de pausa e autoridade/ordem exigem integração. RF-020 parcial conforme limitações acima. Nenhum requisito declara implementação ou validação.

## Mudanças desta versão — ADR-059

A política offline segue ADR-059. As únicas operações permitidas por grant offline são: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) enviar mensagem a canal já autorizado; (4) registrar observação local de localização; (5) PTT somente em rede local com grant de floor válido. São sempre negados: divulgação de localização precisa, mutação federada, criação/alteração de grant, mudança de identidade, transmissão RF e transcrição remota.

Cada operação exige epoch comparável, frescor do perfil dentro do limite aplicável, ausência de revogação conhecida e estado anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. O limite numérico de frescor permanece desconhecido até medição; esta SPEC não cria número. A UI distingue ACK por classe e nunca promove classe inferior; `unknown` é mostrado como desconhecido, conforme ADR-058.

- **casos por requisito e casos P desbloqueados (C3-1):** nova subseção final `### Casos por requisito (P/N/B/F)`, com uma linha por requisito declarado (RF-020, RF-027, RF-028) e as quatro colunas preenchidas ou `n/a` justificado, mais linha própria para requisitos apenas de interface; casos P cujo esperado dependia de valor não decidido — janela N/N-1 do perfil, limite de frescor do grant offline e prazo/gatilho da política de retenção — reescritos para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID. Caso de borda `S012-061-B1` (ADR-061) acrescentado. Nenhum número ou prazo é criado.
- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.

As revisões citadas nas seções anteriores pertencem a versões anteriores. A revisão independente do DRAFT-0.6 está pendente; referências anteriores não são parecer desta versão. Revisão desta versão pendente conforme G3-03; SPEC-018 permanece fora do escopo desta edição autorizada.


## Casos por decisão (ADR-056…063)

Casos documentais planejados. ID formal: `NOT_ASSIGNED`. `Estado` observado em todos: `NOT_EXECUTED`. Parâmetro OPEN implica esperado de negar/informar desconhecido, nunca valor inventado. `ack_status` de check-in continua sem enum definido (divergência D3 de `docs/07-contratos/autoridade-contratos.md`), e os casos abaixo não o definem.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S012-056-P1 | ADR-056 | Cliente com perfil versionado; **janela N/N-1 não declarada** (`OPEN`); identidade válida | Criar sessão Guardian por cliente N e por cliente N-1 | Esperado reformulado com parâmetro ausente: criação recusada com `param_not_configured` e estado preservado em `draft`; nenhum convite emitido e compatibilidade exibida como desconhecida. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED |
| S012-056-N1 | ADR-056 | Cliente sem feature crítica comum (ex.: check-in assinado) | Tentativa de ativar sessão | Sessão permanece em `draft`; nenhum convite emitido; resposta com `required_profile` preenchido; sem downgrade silencioso para `active` | N | NOT_EXECUTED |
| S012-056-B1 | ADR-056 | Sessão criada por binário N com campo desconhecido no registro | Leitura por binário N-1 | Campo desconhecido preservado na leitura/regravação; estado da sessão não alterado pela leitura | B | NOT_EXECUTED |
| S012-056-F1 | ADR-056 | Janela de suporte OPEN; consulta de compatibilidade sem resposta conclusiva | Tentativa de ativar sessão durante falha de consulta | Sessão mantida em `draft`/`consent_pending` com estado `unknown`; nenhuma ativação presumida; nenhuma mutação repetida | F | NOT_EXECUTED |
| S012-057-P1 | ADR-057 | Sessão ativa; check-ins gravados em duas partições com mesma `seq` | Leitura de check-ins por cursor com `partition` | Check-ins ordenados pela tupla canônica; posições distintas por partição; nenhum check-in colapsado | P | NOT_EXECUTED |
| S012-057-N1 | ADR-057 | Cursor de check-in emitido sem `partition` | Reutilização do cursor na consulta de sessão | Rejeição por cursor sem partição; zero check-ins retornados; nenhum check-in exibido como novo | N | NOT_EXECUTED |
| S012-057-F1 | ADR-057 | `authority_epoch` mudou; cursor antigo de sessão em mãos | Replay do cursor após troca de epoch | Falha fechada; nenhum check-in reentregue; estado da sessão não regride nem avança por dado de epoch incomparável | F | NOT_EXECUTED |
| S012-057-B1 | ADR-057 | Evento de sessão traz `event_sequence` no campo de autoridade | Validação do evento | Evento rejeitado por tipo incompatível; `event_sequence` não é gravado como `authority_epoch` | B | NOT_EXECUTED |
| S012-058-P1 | ADR-058 | Check-in recebido e commitado localmente; depois replicado à autoridade | Consulta de estado do check-in | `ack_class=persisted_local` e depois `replicated_to_authority`; UI exibe a classe recebida e o escopo atestado | P | NOT_EXECUTED |
| S012-058-N1 | ADR-058 | Check-in com `ack_class=persisted_local` e `ack_status` indefinido | Tentativa de exibir "pessoa confirmou" ou atendimento | Escalada para `acknowledged_human` rejeitada; UI mantém "não confirmado por pessoa"; nenhum `principal_ref` fabricado | N | NOT_EXECUTED |
| S012-058-F1 | ADR-058 | Check-in enviado; confirmação perdida por timeout | Consulta após prazo de check-in | Resultado `unknown`; `overdue` indica ausência de confirmação e não cria SOS, entrega ou atendimento humano | F | NOT_EXECUTED |
| S012-058-B1 | ADR-058 | Evento de sessão sem `acknowledged_human` | Renderização do detalhe da sessão | Campo exibido como `not_requested`/`unknown`; nunca `acknowledged` por passagem de tempo | B | NOT_EXECUTED |
| S012-059-P1 | ADR-059 | Offline; sessão Guardian existente com grant válido, epoch comparável, sem revogação; **limite de frescor não decidido** (OPEN) | Realização de check-in | Esperado reformulado com parâmetro ausente: check-in registrado como pendente com conectividade `offline_buffering`; frescor reportado como `param_not_configured`; nenhuma entrega afirmada e nenhum prazo inventado. `S012-059-B1` cobre a negativa quando a política exigir frescor decidido | P | NOT_EXECUTED |
| S012-059-N1 | ADR-059 | Offline; sessão ativa | Pedido de divulgar localização precisa ou de alterar escopo/audiência da sessão | Negado com estado `denied`; zero entradas na fila; nenhuma exceção por urgência presumida | N | NOT_EXECUTED |
| S012-059-N2 | ADR-059 | Offline | Tentativa de criar ou alterar grant, ou de mudar identidade | Negado sempre; nenhuma operação enfileirada; UI mostra motivo e alternativa | N | NOT_EXECUTED |
| S012-059-F1 | ADR-059 | Grant `stale`/`unknown`, relógio incerto, revogação conhecida ou snapshot restaurado com grant ainda válido | Retomada da sessão e novo check-in | Check-in negado com `authorization=unknown`; anti-rollback bloqueia o grant restaurado; sessão não retoma de `paused` | F | NOT_EXECUTED |
| S012-059-B1 | ADR-059 | Pausa com grant vencido | Pedido de retomada pelo titular | Sessão permanece `paused`; nenhuma leitura liberada; nenhum prazo de frescor inventado | B | NOT_EXECUTED |
| S012-060-P1 | ADR-060 | Sessão cujo resumo passa por bridge, ASR ou gravação | Consulta do resumo da sessão na API e na UI | `confidentiality=terminated` presente nos dois lugares junto do resumo; canal nativo permanece sem alegação indevida | P | NOT_EXECUTED |
| S012-060-N1 | ADR-060 | Artefato de sessão com perna terminadora | Tentativa de rotular o resumo como E2EE ou de exibir posição precisa em superfície não autorizada | Rótulo E2EE rejeitado; posição precisa ausente da tela bloqueada e da notificação | N | NOT_EXECUTED |
| S012-060-F1 | ADR-060 | Cópia do resumo sem metadado `confidentiality` | Exibição ao destinatário | Tratada como `confidentiality=terminated`; ausência de metadado não vira E2EE | F | NOT_EXECUTED |
| S012-060-B1 | ADR-060 | Bridge recebe resumo sem redução de conteúdo | Publicação da projeção | `confidentiality=terminated` com declaração de que não houve perda; nenhuma promessa de anonimato | B | NOT_EXECUTED |
| S012-061-P1 | ADR-061 | Transição `active` → `completed` autorizada, com outbox na mesma transação | Conclusão da sessão | Estado terminal e evento de outbox commitados juntos; publicação após commit; leitura subsequente retorna `completed`; Beacon da sessão desativado | P | NOT_EXECUTED |
| S012-061-N1 | ADR-061 | Sessão já em `revoked`; mesmo `event_id` reentregue | Reprocessamento do evento | Nenhum efeito externo reemitido (notificação, Beacon, bridge); terminal continua terminal; nenhum acesso reaberto | N | NOT_EXECUTED |
| S012-061-B1 | ADR-061 | Duplicata do mesmo `event_id` de transição no outbox (retry do publicador) | Replay do outbox | Uma única transição gravada e uma única notificação; duplicata descartada pela chave idempotente e registrada; Beacon não é reativado nem desativado duas vezes | B | NOT_EXECUTED |
| S012-061-F1 | ADR-061 | Crash entre commit da transição e publicação | Reinício do serviço | Outbox reprocessada; sessão permanece no estado terminal commitado; nenhum `active` ressuscitado | F | NOT_EXECUTED |
| S012-061-F2 | ADR-061 | Autoridade de sessão `stale`/`unknown` | Tentativa de ativar ou alterar escopo da sessão | Mutação forte bloqueada com erro explícito; leitura permitida com estado de autoridade desconhecido declarado | F | NOT_EXECUTED |
| S012-062-P1 | ADR-062 | Política com finalidade, classe e cópia declaradas para sessão, check-ins e observações, mas **prazo e gatilho não decididos** (OPEN) | Criação de sessão e registro de check-in | Esperado reformulado com parâmetro ausente: gravação aceita com `retention_policy_ref` presente; prazo e gatilho reportados como `param_not_configured`, por classe e cópia, sem expiração automática anunciada | P | NOT_EXECUTED |
| S012-062-N1 | ADR-062 | Classe de dado da sessão sem política aprovada | Tentativa de persistir sessão ou histórico | Falha fechada com erro explícito; nenhuma linha persistida; UI declara degradação e não promete histórico | N | NOT_EXECUTED |
| S012-062-F1 | ADR-062 | Sessão em terminal com cópias remotas não confirmadas | Revogação e pedido de descarte | Bloqueio de novas leituras registrado; cada cópia com estado próprio; nenhuma afirmação de purga externa ou revogação global | F | NOT_EXECUTED |
| S012-062-B1 | ADR-062 | Hold de auditoria com autoridade, escopo e revisão | Consulta comum ao dado retido | Dado mantido além do prazo sob hold; acesso comum não reativado; revisão do hold registrada | B | NOT_EXECUTED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §7, §9, §10 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-020 | S012-RF020-P1 (novo) | S012-056-N1, S012-RF020-N1 (novo) | S012-056-B1 | S012-056-F1 | Jornadas Personal/Teams/Node parciais: matriz de funções e limites por oferta segue `OPEN-01`; Platform/SDK não é parte literal de RF-020 |
| RF-027 | S012-RF027-P1 (novo) | S012-RF027-N1 (novo) | S012-RF027-B1 (novo) | S012-RF027-F1 (novo) | Seleção manual prevalece; sugestão identificada como tal; taxonomia, conflito multi-dispositivo e expiração seguem `OPEN-02` |
| RF-028 | S012-059-P1 (reformulado) | S012-059-N1, S012-059-N2 | S012-059-B1, S012-061-B1 (novo) | S012-058-F1, S012-059-F1 | Atraso nunca cria SOS; retenção, sync e ordem/relógio de transições seguem `OPEN-03/04/08` |
| RF-009, RF-021, RF-026, RF-029, RF-030, RF-034, RNF-003, RNF-012, RNF-017 | n/a — pertencem a SPEC-006/011/013/014/020 | n/a | n/a | n/a | Apenas interfaces de integração; esta SPEC não declara casos desses requisitos nem afirma cobertura |

Casos novos, descritos em uma linha: **S012-RF020-P1** — jornada da oferta com autorização válida e bridge externa indisponível: operação nativa mantém seu estado e a falha da bridge é exibida sem inferir estado do núcleo. **S012-RF020-N1** — ator fora do tenant, do escopo ou do papel exigido: acesso negado sem revelar existência, e administração não substitui consentimento Guardian. **S012-RF027-P1** — seleção manual de atividade com sugestão de sensor divergente: manual prevalece, sugestão permanece rotulada e nenhuma permissão RF ou SOS é gerada. **S012-RF027-N1** — contexto `unknown` sem sensor: funções fundamentais permanecem elegíveis e nenhum papel, local ou direito de transmissão é presumido. **S012-RF027-B1** — dois dispositivos do mesmo titular com atividades conflitantes: conflito declarado e nenhuma escolha silenciosa de um dos contextos. **S012-RF027-F1** — perda da fonte de contexto durante sessão ativa: contexto passa a `unknown`/expirado sem encerrar a sessão nem iniciar SOS. **S012-061-B1** — duplicata do mesmo `event_id` no outbox: uma única transição, uma única notificação, Beacon não reativado nem desativado duas vezes.

**Sem caso por falta de decisão ou de escopo:** ADR-063 não tem caso local nesta SPEC — assinatura, SBOM e piso de versão de artefatos distribuídos pertencem às SPECs de cliente/firmware/operação (015/016/017/019), e esta SPEC de produto não distribui artefato. `ack_status` de check-in permanece sem enum (OPEN/D3): por isso não existe caso positivo de `acknowledged_human` com valor concreto. Beacon em `paused` (OPEN-07) e ordem/relógio de transições (OPEN-08) impedem casos com esperado categórico de pausa.
