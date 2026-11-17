# SPEC-020 — Segurança, privacidade e retenção

**Versão:** DRAFT-0.4 · 17/11/2026  
**Estado:** DRAFT/OPEN; não aprovada, implementada ou validada.  
**Autor documental:** Bruce Schneier · **Revisores independentes designados:** Dan Kaminsky e Raphael Mudge; revisão desta versão pendente.  
**Data:** 2026-11-17 · **Revisor:** Dan Kaminsky e Raphael Mudge (independentes; parecer desta versão pendente).  
**Retenção por classe:** prazos definidos na ADR-065 (`../04-seguranca/retencao-revogacao.md`, seção "Política inicial de retenção por classe"); revisão jurídica por jurisdição pendente; tenant pode reduzir, nunca ampliar sem nova decisão.
**Escopo:** política transversal por finalidade, autorização, confidencialidade, revogação, retenção, cópias, riscos e evidência. As SPECs de identidade, dados, sync, localização, ASR, mídia e bridges mantêm seus contratos e autoridades próprios.

## 1. Origem e precedência

## Mudanças desta versão

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito coberto e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; caso de borda acrescentado em ADR-061. Nenhum caso foi executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com negativo e falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`. Nenhum caso foi executado.
- ADR-058: taxonomia de seis classes de ACK com `ack_class` obrigatório; proíbe inferir entrega ou atendimento.
- ADR-060: define perfis MLS para mensagens/grupos e SRTP para voz; explicita consentimento para ASR no servidor, terminação de confidencialidade e proibições de alegação.
- ADR-062: exige política de retenção fail-closed pela tupla `(finalidade, classe, cópia, jurisdição, prazo, gatilho)`; detalha hold e revogação por destino.
- Correção G3-02: cabeçalho explicita data, versão, estado e revisores; mantém DRAFT/OPEN.

A arquitetura de referência e ADRs prevalecem conforme `docs/README.md` §Regras de prevalência. Foram confrontados `docs/01-arquitetura/arquitetura-referencia.md` §§Modelo operacional e segurança, `docs/06-planejamento/decisoes.md`, `docs/06-planejamento/pendencias.md` §Direções de produto, `docs/06-planejamento/matriz-rastreabilidade.md` §Cobertura individual, `docs/04-seguranca/seguranca.md`, `docs/04-seguranca/retencao-revogacao.md` §§1–8, `docs/04-seguranca/politica-localizacao.md` §§1–3 e `docs/07-contratos/identidade-autorizacao.md` §§Chaves e falhas. RR-001 e os contratos são propostas documentais; não demonstram implantação. A origem da autorização específica de ASR foi relatada por coordenação e permanece a confirmar quanto à fonte e alcance; não é promovida aqui a decisão comprovada.

Insumo de autoria: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-bruce-schneier.md`, SHA-256 `8a18dac0c45c4ad365112619399cfdf1f14c4c28dfb2932e98b00ec971923488`, leitura em 16/11/2026, §§1–7 e 13. Pareceres Dan §4 e Raphael S-01/S-02 são considerados nos §§4–6; os hashes dos snapshots e sua mudança concorrente constam na contribuição §§13. Kim, Daniel e Paul foram revisados na contribuição §§8–12 e 14–17: integração e revisões de versões futuras seguem OPEN. Este documento não atribui decisão por copiar parecer.

## 2. Requisitos e fronteiras de cobertura

| ID | Subescopo coberto por esta SPEC | Interface / pendência |
|---|---|---|
| RF-021 | Autorização de rede, operação e localização por finalidade; não conceder transmissão RF por autenticação isolada. | Gregor/Kim e política de rádio; `matriz-rastreabilidade.md` linha RF-021. |
| RF-022 | Perfis de confidencialidade, destinatários, terminação e alegação E2EE. | Kim/Radia/Daniel/Gregor definem chaves, mídia e bridge; linha RF-022. |
| RF-030 | Proteção de avistamentos, audiência, revogação e cópias CrowdFind. | Kim/Gregor; token, validade e antirrastreamento permanecem OPEN. |
| RF-039 | Publicação e revogação de localização exata de AP por administrador autorizado. | Jouni/Graydon; prova de titularidade, estado e invalidadores de cache a conciliar. |
| RNF-003 | Isolamento multi-tenant, localização consentida, autorização e auditoria. | Kim/Richard; decisão por operação e retenção devem ser conciliadas. |
| RNF-015 | Dados administrativos de AP/visitantes: acesso por função/tenant e retenção por cópia. | Jouni/Kim/Michael; papéis, campos e prazo ainda OPEN. |
| RNF-017 | Subescopo transversal de privacidade em partição, revogação e recuperação. | Martin é integrador; disponibilidade de AP e comportamento cliente dependem de Jouni/Alex. |
| RF-025/RF-026 | Dependências de segurança de ASR/SOS, não cobertura funcional integral. | Daniel/Nancy/Radia; autorização específica ASR e ACK SOS por camada a conciliar. |

Casos formais e índice central são mantidos por Ada: identificadores ainda não associados são `NOT_ASSIGNED`. Os cenários abaixo são planejados, não testes executados. Não inferir vínculo a outros requisitos por semelhança nominal.

## 3. Aplicabilidade e atores

Abrange Personal, Teams, Node e Platform/SDK nas modalidades hospedada e autogerida. Servidor Linux aplica política em seus pontos de autorização, persistência, réplica e saída. macOS arm64/x86_64, iOS, Windows, Linux desktop, Android e web aplicam os mesmos invariantes nos pontos de captura, visualização e cópia **quando o recurso for suportado e autorizado**; capacidades, permissões do SO, armazenamento e operação em background precisam de especificação e evidência por plataforma. Hardware ou API não demonstrados permanecem OPEN; nenhum N/A global é presumido.

Ator autenticado solicita operação para recurso, tenant, audiência e finalidade; identidade e autoridade de grant/revogação pertencem à SPEC-002; autoridade de escrita/fence à SPEC-001 e contratos de autoridade; sync e ordem à SPEC-004; dados e cópias à SPEC-005. Operador hospedado e operador autogerido têm responsabilidades conforme modalidade e acordos, sem presumir que configuração de região determine residência de todas as cópias. Auditoria de acesso é restrita por papel/tenant/finalidade.

## 4. Contrato de política e autorização

**Pré-condições:** principal e vínculo autenticados; autoridade e estado conhecidos; tenant/recurso, finalidade, audiência e classe identificados; ACL/grant e consentimento quando exigido; política efetiva e regra por cópia; capacidade da plataforma e jurisdição resolvidas quando exigidas. Autenticação, identificador, permissão do SO ou capacidade técnica não substituem autorização para outro uso. O destino verifica sua própria autorização; autorização de saída não autoriza automaticamente entrada em bridge ou federação.

**Entrada lógica, não schema/wire:** referência de principal, recurso, tenant, operação, finalidade, classe, destinatário, grant/consentimento, versão de política e evidência de autoridade/frescor. Nunca incluir segredo, localização ou conteúdo no recibo de autorização. **Decisão:** permitir apenas interseção de todos os predicados; se ausente, revogado, expirado, incompatível, `stale` ou `unknown` no caso sensível, negar antes do efeito e retornar motivo sanitizado que não permita enumerar outros titulares. Política ou prazo de cópia ausentes bloqueiam persistência, nova divulgação, replicação e exportação; uso transitório local só com autorização, minimização e ciclo de memória próprios documentados.

**Estados:** `valid`, `expired`, `revoked`, `unknown`, `stale`, `key_unknown` referem-se ao resultado de validação da identidade, não a estados de sync. Assinatura válida demonstra integridade/origem sob confiança configurada, não ausência de revogação posterior. Epoch é identificado por autoridade, namespace e escopo; não confundir com sequência de evento ou dispatch. Validade e frescor por operação, tempo confiável e evidência de estado são OPEN. Em partição, não permitir nova ação sensível/federada quando atualidade não é demonstrada. Grants offline só cobrem as cinco operações locais enumeradas em SPEC-002 §6.1 e obedecem às negações e predicados da ADR-059; `stale`, `unknown` e relógio incerto negam.

### ACK por fronteira (ADR-058)

Toda resposta de API que represente confirmação inclui `ack_class`, obrigatoriamente uma de `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`. Cada classe requer emissor, evidência e escopo próprios. É proibido usar “ACK” sem classe ou inferir classe superior por decurso do tempo, sucesso de camada inferior ou ausência de erro. `delivered_to_device` e `acknowledged_human` exigem evidência própria do destino ou da pessoa; não se infere entrega nem atendimento.

**Erros lógicos propostos, não códigos de API:** autorização ausente/inválida, política ou escopo não resolvidos, autoridade indisponível, operação fora da finalidade e resultado de revogação desconhecido geram negação ou indisponibilidade sem efeito; nomes/envelope/códigos pertencem à SPEC-003. Nenhuma falha muda automaticamente audiência, precisão, criptografia ou destino. Repetição e replay só são aceitos com autorização atual e regras da SPEC-004.

## 5. Perfis de confidencialidade e tratamento de conteúdo

Cada operação deve declarar versão de perfil, origem, destinatários, proteção em trânsito e em repouso, componente que pode ver conteúdo em claro, metadados visíveis, autoridade de chaves, política de retenção e falha. Mensagens 1:1 e de grupo entre endpoints GodMesh nativos usam MLS (RFC 9420) como base de acordo de chaves; voz nativa usa chaves de sessão SRTP (RFC 3711) derivadas do mesmo contexto, conforme ADR-060. ASR no servidor só pode ocorrer com consentimento explícito por canal; esse consentimento retira E2EE daquele canal e deve ser mostrado ao usuário. Toda perna que termine proteção por bridge, ASR ou gravação expõe `confidentiality=terminated` na API e na UI. É proibido chamar TLS, DTLS ou SRTP de E2EE, prometer anonimato, alegar apagamento de cópia já entregue ou recuperação de conteúdo E2EE sem chave. Algoritmos complementares, gestão/recuperação de chave e processadores permanecem OPEN com Kim/Radia/Daniel/Gregor.

ASR: o documento `transcricao-multimodal.md` §§Política e pipeline descreve política de canal, ACL/consentimento quando aplicável e processadores; não identifica por si artefato, emissor, validade ou alcance de autorização específica para abrir áudio. A direção adicional relatada requer confirmação de origem e conciliação com Daniel/Kim. **Proposta conservadora não aprovada:** exigir autorização específica por finalidade e processador antes de abrir/processar, separada de participação no canal, gravação, retenção e exportação; se predicado exigido for desconhecido, não enviar áudio ao ASR, mantendo voz autorizada e timeline sem texto inventado. Não presumir que aviso equivalha a consentimento ou autorização.

### Revogação em voo — ASR e bridge

Quando a revogação é aplicada no ponto de decisão, bloquear novos segmentos cobertos e revalidar antes de cada nova divulgação/cópia. Cancelamento solicitado não prova interrupção do worker ou destinatário; por operação distinguir conclusão antes da aplicação, cancelamento pendente, interrupção confirmada e resultado desconhecido. Tais rótulos são conceituais, não enum GMP. Buffer de captura/PCM, fila, parcial/final/correção, índice, logs, backup e exportação são cópias distintas; cada destino reporta sua própria disposição conforme política. Destino desconectado continua desconhecido, conteúdo já recebido/lido e cópias fora do controle não são recolhíveis por promessa. Fronteira de cancelamento, tratamento de ação já iniciada, auditoria e capacidade da plataforma são OPEN; Daniel/Radia/Kim/Martin conciliam.

## 6. Localização, Wi-Fi e SOS

A `politica-localizacao.md` §§1–3 prevê que, se compartilhada em chat/PTT/timeline/bridge, localização pessoal seja projetada em região de raio nominal mínimo de 50 km, sem coordenada exata ou precisão real no payload público; esse limite não garante anonimato. Posição precisa SOS é separada para responsáveis autorizados. Frequência, estabilidade e conteúdo de consultas repetidas devem ser definidos para reduzir inferência de trajetória, sem alegar que a generalização elimina correlação. Localização do AP físico segue política Wi-Fi própria, não recebe automaticamente regra de localização pessoal.

Guardian depende de finalidade, sessão e contatos autorizados; CrowdFind de adesões e escopos aplicáveis; projeção exata de AP exige prova de administrador e consentimento/publicação específicos. Revogação aplicada impede novas leituras e divulgação cobertas, invalida caches controlados e indica destinos sem confirmação; credenciais Wi-Fi já entregues podem requerer rotação e não podem ser recolhidas unilateralmente. Em SOS, preparação, fila, persistência/ACK técnico e atendimento humano são estados diferentes: nenhum ACK técnico por si comprova atendimento. Tratamento de posição expirada, exceção offline, direitos de rádio e responsabilidade humana continuam OPEN para Nancy/Paul/Jouni/Kim.

## 7. Retenção, cópias e restore

Conforme ADR-062, nenhuma classe de dado pode ser persistida sem política aprovada identificada pela tupla `(finalidade, classe, cópia, jurisdição, prazo, gatilho)`. A falta de política bloqueia persistência e degrada explicitamente a operação, preservando somente o mínimo necessário ao efeito imediato autorizado. Uma política efetiva também identifica `policy_id/version`, tenant, operador/destino, disposição, autoridade e vigência. Representação é lógica e não cria SQL. A política trata separadamente primária, réplica/índice, fila/outbox/cache, dispositivo offline, backup, exportação, derivados e auditoria. Prazos e jurisdições permanecem `OPEN — decisão do usuário`; nenhum prazo jurídico é presumido.

| Cópia | Contrato proposto | Falha e evidência |
|---|---|---|
| Primária e réplica/índice | Acesso somente com autorização vigente; expiração e disposição por classe/cópia. Replay não renova prazo. | Bloqueio de acesso não é prova de eliminação; destino sem confirmação permanece pendente/desconhecido. |
| Fila/cache/dispositivo | Preservar apenas dado autorizado e ciclo próprio; revalidar antes da entrega, descartar conforme política. | Dispositivo offline ou relógio sem prova não demonstra revogação ou eliminação remota. |
| Backup/snapshot | Restaurar isolado e reconciliar autoridade, revogações, retenção e histórico posteriores ao snapshot antes de servir dado sensível. | Histórico incompleto bloqueia operação dependente; somente fluxos comprovadamente independentes podem prosseguir conforme política. |
| Exportação/terceiro | Exigir finalidade, operador, destino e política antes de exportar; registrar limite de controle. | `out_of_control` é limite de controle, não prova de que terceiro apagou ou reteve o dado. |
| Auditoria/derivado | Conteúdo mínimo e acesso por finalidade; agregado só é irreversível quando demonstrado. | Recibo sem payload, segredo ou coordenada; retenção própria e prova por destino. |

Cada resultado de disposição referencia objeto/classe, cópia e destino, operador, versão da política, instante, autoridade e evidência; alcance da consulta deve ser explícito. Acesso, disposição e controle são dimensões distintas: `access_blocked` não implica `delete_confirmed`; `crypto_inaccessible` só equivale a descarte se todas as cópias de chave e recuperação forem tratadas e verificadas. `pending/unknown` não são sucesso; exportação fora do controle é apresentada separadamente. Inventário incompleto impede declarar conclusão global; um resultado pode afirmar sucesso apenas para subconjunto identificado e comprovado. Estados são de RR-001, não novo evento de sync.

Hold específico exige autoridade, escopo e revisão identificados; suspende somente o descarte abrangido e nunca reativa grant nem acesso/uso comum. Revogação e seu resultado são declarados por destino/cópia, nunca globalmente. Autoridade de hold, prazos e jurisdições permanecem `OPEN — decisão do usuário`; seleção de região não determina base legal.

## 8. Ameaças e controles planejados

| Ameaça / pré-condição | Controle proposto e limite |
|---|---|
| Token válido usado em outro tenant/recurso | Validar vínculo, tenant, finalidade e ACL no destino; falha sanitizada sem revelar titular. |
| Revogação durante partição ou sessão longa | Revalidar por unidade de efeito, negar onde revogação é conhecida, manter destinos sem prova como desconhecidos; janela residual não é medição. |
| Replay ou restore anterior à revogação | Verificar autoridade atual/histórica, corte e política; isolar se histórico relevante incompleto. Snapshot ou tombstone não prova exclusão física. |
| Bridge/ASR recebendo conteúdo além do escopo | Verificar processador e audiência; declarar terminação, bloquear próximos envios e registrar estado por destino; não prometer recolhimento externo. |
| Inferência de posição por dados derivados | Projeção por superfície, minimização de campos e frequência; sem alegação de anonimato; regra para texto livre e dado não classificável está OPEN. |
| Vazamento por logs ou falha de redaction | Allowlist antes de buffer/sink; falha bloqueia exportação sensível e emite sinal sanitizado. Cobertura desconhecida não é ausência de incidente. |

## 9. Aceites e casos planejados

Os seguintes comportamentos são critérios propostos de revisão; cada caso formal depende de índice, contratos e política aprovados. **Todos NOT_EXECUTED.**

1. Principal com grant válido para tenant A tenta ler recurso B: negar antes do efeito, não revelar existência; principal autorizado a A obtém apenas classe/finalidade permitidas.
2. Grant assinado com epoch antigo, namespace incompatível ou relógio sem frescor solicita localização precisa/sync privado: negar conforme contrato de identidade; reconexão revalida antes de replay, sem ampliar audiência.
3. Revogação ocorre entre inferência e publicação ASR e em bridge externa: após confirmação no destino, impedir próxima divulgação; mostrar resultado da ação em voo e disposição por cópia; áudio já recebido por terceiro não é declarado removido.
4. AP exato sem prova/opt-in de administrador não é publicado; após revogação, cache controlado aplicado não retorna exato; dispositivo desconectado permanece estado desconhecido.
5. Acesso a registros de visitantes é limitado por papel, AP, tenant e finalidade; terceiro sem escopo não pode inferir existência; prazo e descarte seguem política específica.
6. Snapshot anterior à revogação é restaurado com histórico incompleto: cópia privada permanece isolada; replay não reativa grant nem reinicia prazo; confirmação de acesso bloqueado não equivale a expurgo.
7. SOS sem ACK humano não é apresentado como atendimento, ainda que haja ACK técnico; posição expirada ou autorização incerta gera estado honesto e avaliação de risco própria.
8. Inserir canários sintéticos de conteúdo, coordenada e segredo em eventos de erro/log/exportação em validação futura: política bloqueia vazamento conforme catálogo aprovado; falha do redactor resulta em sinal sanitizado e cobertura desconhecida.
9. Acesso/eliminação de duas réplicas, backup offline e exportação externa são apresentados por cópia; ausência de inventário ou prova impede alegação de eliminação global.

## 10. Decisões e dependências OPEN

| Tema / responsáveis propostos | Alternativas e consequência | Fechamento documental |
|---|---|---|
| Autorização ASR / Daniel, Kim, Radia, Alan/Ada | Referência de política/consentimento de canal versus autorização por sessão/processador; segundo pode melhorar escopo mas depende de autoridade e experiência. Direção relatada precisa fonte/alcance. | Registrar decisão rastreável, emissor, ações, validade, revogação e negativos; revisar sem presumir consentimento universal. |
| Revogação, grants e cancelamento / Kim, Martin, Daniel, Radia | Revalidação por unidade de efeito versus lease limitado; lease não permite envio após revogação conhecida. | Escolher fronteira, frescor e evidência por destino, com cenários em voo e partição; risco residual reconhecido. |
| Autoridade e restore / Leslie, Martin, Michael, Momjian | Fronteira atômica quando demonstrável ou protocolo recuperável com fence/ledger; segundo exige prova de completude. | Definir autoridade, corte, resultados desconhecidos e comportamento sem histórico/fence, com revisão independente. |
| Retenção e descarte / Bruce, Michael, Lawrence, Alan/Ada | Prazos por finalidade/classe/cópia, descarte físico versus criptográfico comprovável; maior retenção pode facilitar recuperação e ampliar exposição. | Aprovar gatilhos, duração, operador, método e evidência por cópia e jurisdição; sem número inventado. |
| Proteção de conteúdo/E2EE / Kim, Radia, Daniel, Gregor | Endpoint protegido versus processador terminador; disponibilidade e exposição diferem. | Declarar destinatários, chaves, metadados, terminação, recuperação e ausência de downgrade por operação. |
| Localização, SOS e Wi-Fi / Paul, Nancy, Jouni, Kim | Bloqueio em ausência de frescor e operações locais limitadas dentro das negativas; diferentes impactos de disponibilidade e privacidade. | Política por operação/destino, frescor e risco de atraso/perda por ACK, sem simular atendimento. |
| Auditoria e telemetria / Richard, Brendan, Bruce | Centralização ou escopos distribuídos, com limites de correlação, acesso e retenção. | Definir campos, papéis, fonte/cobertura, estados unknown, autoridade de contenção e evidência sanitizada. |
| Plataformas / Alex, Chet, Donny, Lea | Capacidade nativa comprovada ou degradação explícita; não pressupor hardware seguro uniforme. | Matriz por SO/versão, permissões, armazenamento, relógio, lifecycle e casos de falha. |

Revisões independentes de Dan e Raphael são exigidas para **esta versão/hash** após entrega; a revisão de rascunho anterior não substitui revisão desta SPEC. O gate de planejamento e aprovação das SPECs não é fechado por este arquivo. Nenhum código, SQL, teste de produto, infraestrutura, hardware, jurisdição ou proteção operacional foi executado ou validado.

## 11. Revisões independentes da versão DRAFT-0.1

**Escopo das revisões:** Dan Kaminsky e Raphael Mudge examinaram a versão SHA-256 `b30a1d004475e4f9d55c31f04b3e15015e0341d788673e90d8b240040baafd05`, não esta versão com adendo. Dan relata nenhum achado confirmado e nenhuma contradição material nos recortes, mantendo riscos e decisões OPEN. Raphael registra quatro achados de completude, sem contradição normativa definitiva. As avaliações não são conflitantes por si: Dan relata ausência de falha confirmada; Raphael identifica lacunas já abertas para prontidão. Ambas são revisões documentais; não validam implementação. Parecer Raphael: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-raphael-mudge.md` §Revisão independente — SPEC-020 DRAFT-0.1; Dan comunicou resultado e fontes por mensagem de 16/11/2026. Jev de Dan foi inconclusivo segundo seu relato e não é evidência desta SPEC.

| Achado Raphael | Resposta do autor / estado |
|---|---|
| R-020-01 — autoridade/origem ASR | **Aceito; OPEN.** §§1,5,10 já mantêm direção como relato indireto e proposta conservadora. Para fechar, coordenação e Kim/Daniel/Radia registram origem e alcance, autoridade, finalidade, processador, validade e revogação, sem converter aviso em autorização. |
| R-020-02 — revogação em voo por fronteira | **Aceito; OPEN.** §5 distingue pedido de cancelamento, confirmação e resultado desconhecido. Acrescentar na futura conciliação matriz captura → inferência → parcial/final/correção → entrega por destino, com ponto de decisão, prova e estados de mensagem perdida; §9 caso 3 é plano resumido, não cobre isoladamente todas as fronteiras. |
| R-020-03 — disposição por cópia | **Aceito; OPEN.** §7 separa acesso, disposição, controle e inventário incompleto. Michael/Martin/Kim/Bruce devem definir recibo verificável, completude, estado de chave e recuperação, distinguindo cópia externa e histórico faltante. |
| R-020-04 — catálogo e rastreabilidade | **Aceito; OPEN.** §9 descreve cenários sem IDs e caso 8 depende de catálogo de sanitização ainda não identificado por artefato/versão. Ada vincula requisitos/casos e responsáveis; Bruce/Richard/Brendan definem catálogo por sinal, fonte, cobertura, falha de redaction e evidência por destino; casos formais permanecem NOT_ASSIGNED e resultados NOT_EXECUTED. |

Esta resposta integra o estado das revisões, sem fechar riscos ou decisões de produto. A alteração muda o hash: Dan e Raphael devem verificar este adendo e a versão subsequente antes de considerar sua revisão aplicável ao arquivo completo. Nenhuma política, SPEC, teste ou resultado operacional é aprovado por este registro.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC. `Estado` sempre `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`; nada foi executado nem medido. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN produz negativa, bloqueio ou informação de desconhecido, nunca valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S020-056-P1 | ADR-056 | Perfil de confidencialidade e de política publicado declara N e N-1 | Operar com cliente N-1 e com cliente N | Com janela declarada, ambos aceitos dentro da interseção autorizada e versão de perfil registrada; sem janela declarada no perfil, `param_not_configured` com negativa sanitizada e nenhum aceite por padrão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-056-N1 | ADR-056 | Cliente em versão de perfil fora da janela declarada | Solicitar operação com conteúdo protegido | Negado antes do efeito, com motivo sanitizado; nenhuma degradação para perfil antigo | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-056-N2 | ADR-056 | Perna terminadora declara perfil de confidencialidade desconhecido pelo consumidor | Consumir o artefato | Tratado como `confidentiality=terminated` e recusado como E2EE; nenhuma cláusula do perfil ignorada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-056-B1 | ADR-056 | Janela numérica por perfil não decidida (produto OPEN) | Consultar suporte de uma versão de perfil | Informa janela não declarada; sem aceite por padrão | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-056-F1 | ADR-056 | Artefato derivado produzido por perfil novo; serviço revertido para perfil anterior | Consumir o derivado após rollback | Bloqueio com estado explícito de incompatibilidade; derivado mantido indisponível, sem reinterpretação de metadados | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-057-P1 | ADR-057 | Cursor opaco de exportação vinculado a principal, escopo, filtros, perfil e retenção; versão do cursor e janela de retenção podem não estar declaradas | Exportar a partir do cursor | Com versão e retenção declaradas, exportação a partir do prefixo contíguo com versão registrada; sem versão de cursor ou janela declarada, `param_not_configured` recusando a exportação e nenhum dado servido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-057-N1 | ADR-057 | Cursor de exportação sem `partition` | Apresentar o cursor | Rejeitado; nenhuma tradução e nenhum dado exportado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-057-N2 | ADR-057 | Cursor de epoch incomparável após troca de autoridade | Reapresentar o cursor | Rejeição fail-closed; recuperação declarada incompleta e rebootstrap autorizado exigido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-057-B1 | ADR-057 | Cursor válido cujo escopo teve retenção reduzida | Exportar | Exportação limitada à retenção vigente; nenhum evento acima do prazo é servido e a limitação é declarada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-057-F1 | ADR-057 | Cursor de outro principal reutilizado em sessão autorizada | Exportar | Rejeitado; nenhuma ampliação de acesso, nenhum dado de outro principal e registro da tentativa | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-058-P1 | ADR-058 | Divulgação persistida localmente, sem recibo do destino; nome wire de `ack_class` pode não estar aprovado | Ler a resposta e a UI | Com campo aprovado, `ack_class=persisted_local` e entrega/leitura exibidas como desconhecidas; sem nome wire aprovado (D8), `param_not_configured`/`unknown` e nenhuma classe presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-058-N1 | ADR-058 | Resposta com `ack_class=persisted_local` sobre conteúdo sensível | Fluxo tenta exibir "entregue" ou "atendido" | Exibição recusada; campos de entrega e atendimento permanecem desconhecidos | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-058-N2 | ADR-058 | Resposta de disposição/revogação sem `ack_class` | Consumir a resposta | Inválida; nenhuma confirmação registrada e estado local `unknown` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-058-F1 | ADR-058 | Revogação aplicada; recibo de destino perdido | Consultar estado por destino | `outcome_unknown` com reconciliação; nunca `confirmed` por ausência de erro | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-058-B1 | ADR-058 | Incidente SOS com confirmação do dispositivo e nenhuma pessoa autenticada | Exibir atendimento | `delivered_to_device` sem `acknowledged_human`; atendimento só após ação humana autenticada e auditada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-059-P1 | ADR-059 | Grant offline válido, operação enumerada; limite de frescor pode estar declarado ou não | Enviar mensagem a canal já autorizado | Com frescor declarado, aceito localmente com `ack_class=persisted_local` e nenhuma nova autorização de canal; sem limite configurado, `param_not_configured` com negativa e nenhum TTL presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-059-N1 | ADR-059 | Grant offline válido | Divulgar localização precisa, transmitir RF, pedir transcrição remota, criar grant ou mutar federação | Negado em todas; nenhum efeito e nenhuma cópia criada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-059-N2 | ADR-059 | Grant com revogação desconhecida no destino | Operação condicional | Negada com `revocation_state_unknown`; ausência de registros locais não é lida como ausência de revogação | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-059-F1 | ADR-059 | Dispositivo restaurado de snapshot com grant na validade | Operação condicional | `rollback_suspected`; negativa sem efeito e orientação de revalidação; repetição mantém a negativa | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-059-B1 | ADR-059 | Limite de frescor por classe não medido (OPEN) | Operação condicional offline | Negada e informada como frescor não definido; nenhum TTL presumido | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-P1 | ADR-060 | Canal 1:1 com MLS estabelecido entre endpoints nativos | Enviar mensagem e consultar metadados | Servidor sem acesso ao conteúdo e busca de conteúdo indisponível; resposta e UI expõem o perfil de confidencialidade somente com campo aprovado; sem campo aprovado (D9), `param_not_configured`/`unknown`, sem rótulo de E2EE presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-N1 | ADR-060 | Bridge ou ASR com consentimento de canal | Declarar E2EE na perna ou em derivado | Alegação rejeitada; `confidentiality=terminated` exibido de forma visível na API e na UI | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-N2 | ADR-060 | Chamada de TLS, DTLS ou SRTP como prova de E2EE em contrato ou texto de UI | Revisar a alegação | Alegação recusada; rótulo E2EE não aceito e conteúdo permanece `terminated` quando houver perna terminadora | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-N3 | ADR-060 | Cópia de derivado sem metadado de confidencialidade | Consumir a cópia | Falha fechada: tratada como `terminated`; nenhuma inferência por cache, versão anterior ou ausência de erro | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-B1 | ADR-060 | ASR no servidor sem consentimento específico por canal; áudio disponível | Enviar áudio ao ASR | Não enviado; voz autorizada mantida e timeline sem texto; motivo exibido ao usuário | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-F1 | ADR-060 | Consentimento de ASR revogado durante processamento; buffer e parcial já produzidos | Revogar e consultar estado | Novos segmentos bloqueados; parcial e final marcados `terminated`; interrupção do worker tratada como desconhecida até prova, sem promessa de apagar cópia já lida | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-060-F2 | ADR-060 | Remoção de dispositivo do grupo | Enviar mensagem posterior | Chaves atualizadas e removido sem acesso; tentativa de recuperar conteúdo antigo sem chave falha; cópias entregues não são declaradas apagadas | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-P1 | ADR-061 | Banco de domínio disponível; política e autorização vigentes | Mutação com outbox na mesma transação | Commit único; publicação após commit uma vez; ACK `persisted_local` | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-N1 | ADR-061 | Autoridade `stale` ou `unknown` no escopo | Mutação forte sobre dado sensível | Bloqueada sem commit e sem ACK de sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-N2 | ADR-061 | Inbox em ledger separado | Tentar tratar ACK de Inbox como materialização | Recusado; máximo `persisted_local` e projeção `pending` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-B1 | ADR-061 | Política e autorização vigentes; commit de domínio já persistido e publicação pendente | Consultar o estado antes da publicação do evento | ACK máximo `persisted_local`; estado de publicação separado como pendente e nenhuma materialização ou entrega inferida do commit local | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-F1 | ADR-061 | Restore de snapshot anterior a revogações e retenções vigentes | Servir cópia restaurada | Bloqueio até reconciliar autoridade, revogações e retenção; somente fluxos comprovadamente independentes prosseguem | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-061-F2 | ADR-061 | Crash entre commit e publicação de evento de disposição | Retry | Mesmo evento e digest publicados uma vez; nenhum efeito externo (bridge, notificação) repetido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-P1 | ADR-062 | Política aprovada identificada pela tupla `(finalidade, classe, cópia, jurisdição, prazo, gatilho)`; prazo ou jurisdição podem faltar | Persistir dado pessoal da classe | Com tupla completa, aceito com `policy_id/version` e tupla registrados e disposição por cópia planejada; com prazo ou jurisdição ausentes, `param_not_configured` com falha fechada e nenhuma retenção presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-N1 | ADR-062 | Classe sem política aprovada | Tentar persistir, replicar ou exportar | Falha fechada com erro explícito; apenas o mínimo do efeito imediato autorizado é mantido e a degradação é declarada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-N2 | ADR-062 | Prazo ou jurisdição não decididos (OPEN) | Consultar retenção da classe | Informa prazo não definido; nenhum valor presumido e nenhuma retenção indefinida apresentada como política | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-N3 | ADR-062 | Revogação aplicada a um destino; cópia offline e exportação a terceiro | Declarar revogação | Declaração por destino/cópia, nunca global; `out_of_control` reportado como limite de controle e `pending`/`unknown` não é sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão; auditoria sob hold | Acesso comum e expiração de prazo durante o hold | Descarte suspenso apenas no escopo; acesso comum segue negado e o hold não reativa grant | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-F1 | ADR-062 | Expurgo solicitado para classe e cópia específicas | Acionar descarte por destino | Recibo por objeto/classe/cópia com política, instante e evidência; `access_blocked` sem `delete_confirmed`; inventário incompleto impede conclusão global | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-062-F2 | ADR-062 | Chaves destruídas; réplica de chave ainda existente | Declarar descarte por criptografia | `crypto_inaccessible` não declarado como descarte enquanto houver cópia de chave ou recuperação não tratada; estado `pending` | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-P1 | ADR-063 | Artefato de cliente e servidor com SBOM, digest registrado e assinatura válida; `generation` acima do piso | Instalar | Assinatura verificada; instalação aceita com `generation` registrada | P | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-N1 | ADR-063 | Artefato adulterado ou assinatura inválida | Instalar | Rejeitado; nenhum código executado e motivo de integridade registrado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-N2 | ADR-063 | Dependência nova de criptografia sem versão resolvida, fonte, data, licença, digest e análise de vulnerabilidade | Adotar a dependência | Adoção bloqueada; nenhum release distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-B1 | ADR-063 | Piso de versão assinado e release atual | Apresentar artefato com `generation` igual ao piso | Aceito; `generation` e piso registrados na verificação | B | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-F1 | ADR-063 | Release anterior com vulnerabilidade conhecida e `generation` inferior ao piso | Tentar downgrade | Rejeitado pelo piso; instalação existente preservada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-F2 | ADR-063 | Chave de assinatura comprometida; lista de revogação publicada | Cliente verifica artefato assinado pela chave revogada | Rejeitado; artefato da chave reemitida aceito e revogação registrada no cliente | F | NOT_EXECUTED / NOT_ASSIGNED |
| S020-063-F3 | ADR-063 | Rollback de release autorizado e limitado no tempo | Aplicar o rollback | Aceito somente com auditoria e piso rebaixado por decisão registrada; sem exceção, rejeitado | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) prazos, bases legais e jurisdição por classe — decisão jurídica/produto; (2) algoritmos complementares, gestão e recuperação de chaves MLS/SRTP; (3) ratificação de produto para ASR no servidor; (4) perfis e enum de `confidentiality` — campo inexistente nos contratos (D9); (5) nome wire de `replicated_to_authority` (D8); (6) janela N/N-1 por perfil; (7) autoridade, escopo e revisão de hold; (8) fronteira de cancelamento de ASR e tratamento de ação já iniciada.

### Casos por requisito (P/N/B/F)

Uma linha por requisito da §2. Cada célula referencia caso S020-xxx já definido, cenário numerado da §9 ou cria ID local descrito na célula; `n/a` só com motivo. Estado: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-021 | S020-059-P1 | S020-059-N1 | §9 caso 2 (epoch antigo ou relógio sem frescor nega localização precisa e sync privado) | S020-059-F1 | Autorização de rede/RF por finalidade; política de rádio pertence a Gregor/Kim; `n/a` não se aplica porque a negação é observável aqui. |
| RF-022 | S020-060-P1 | S020-060-N1, S020-060-N2, S020-060-N3 | S020-060-B1 | S020-060-F1, S020-060-F2 | Perfis de confidencialidade; algoritmos complementares e gestão de chave OPEN. |
| RF-030 | n/a — motivo: proteção de avistamentos e adesões CrowdFind depende de token, validade e antirrastreamento ainda OPEN com Kim/Gregor; não há positivo aprovado | §9 caso 4 (AP exato sem prova/opt-in não é publicado) | §9 caso 4 (revogação aplicada com cache controlado não retorna exato; dispositivo desconectado permanece desconhecido) | S020-062-N3 | Cópia offline e exportação impedem declaração global. |
| RF-039 | n/a — motivo: prova de titularidade e publicação de AP exato não aprovadas; positivo seria publicação com evidência vigente | §9 caso 4 | §9 caso 4 (cache controlado após revogação) | S020-062-F1 | Invalidadores de cache e estado do controlador OPEN com Jouni/Graydon. |
| RNF-003 | §9 caso 1 (principal autorizado obtém apenas classe/finalidade permitidas) | §9 caso 1 (tenant B negado sem revelar existência) | §9 caso 5 (papel × AP × tenant × finalidade) | §9 caso 6 (snapshot anterior à revogação permanece isolado) | Isolamento multi-tenant e auditoria; decisão por operação conciliada com SPEC-002. |
| RNF-015 | §9 caso 5 | §9 caso 5 (terceiro sem escopo não infere existência) | S020-057-B1 (retenção reduzida limita exportação) | S020-062-F1 | Papéis, campos e prazo de dados administrativos de AP/visitantes seguem OPEN. |
| RNF-017 | S020-059-P1 | S020-059-N2 | S020-058-B1 | S020-061-F1 | Subescopo de privacidade em partição, revogação e recuperação. |
| RF-025, RF-026 | n/a — motivo: aceitação protegida de SOS exige predicado e prova por camada ainda não aprovados; não há positivo de atendimento | §9 caso 7 (ACK técnico não é atendimento; posição expirada gera estado honesto) | S020-058-B1 | §9 caso 3 (revogação entre inferência e publicação ASR/bridge) | Dependência de segurança de ASR/SOS, não cobertura funcional integral. |

Casos desta subseção são critérios documentais; nada foi executado, medido ou validado.
