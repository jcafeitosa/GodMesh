# SPEC-023 — Capacidades, conectividade e contingência

**Versão:** DRAFT-0.3
**Data:** 2026-11-17
**Autor documental:** Gregor Hohpe
**Revisão independente designada:** Vint Cerf — pendente
**Estado:** proposta em rascunho, sem aprovação ou autorização de implementação.
**Mudanças desta versão:** casos por decisão (resposta ao veto C3-1); casos por requisito e casos P desbloqueados (C3-1).

## 1. Objetivo, escopo e rastreabilidade

Esta proposta cobre RF-003, RF-031, RF-032 e RF-033 nos clientes Android e iOS e nas interfaces de dispositivos, gateways e serviços GodMesh. Define avaliação de capacidades, evidência de conectividade, orientação manual e prioridade de tráfego próprio. Especificações de SOS, Guardian, autorização, sincronização e plataformas continuam responsáveis por seus contratos especializados.

| Requisito | Direção | Seções e aceite planejado |
|---|---|---|
| RF-003 | ADR-013, avaliação por sujeito e contexto | §3 e C003 |
| RF-031 | ADR-045, evidência de conectividade | §4 e C031 |
| RF-032 | ADR-046, ação manual | §5 e C032 |
| RF-033 | ADR-046/047, tráfego sob restrição | §6 e C033 |

Contexto e limites decorrem de [requisitos](../00-produto/requisitos.md), [arquitetura de referência](../01-arquitetura/arquitetura-referencia.md) e [decisões](../06-planejamento/decisoes.md). Critério documental é cobrir os quatro requisitos, seus casos e pendências com evidência rastreável. Verificação operacional depende de implementação e ambientes representativos.

## 2. Decisões, alternativas e dependências

ADR-013 exige capacidades verificadas e roteamento. ADR-045 distingue satélite de operadora e terminal Wi-Fi, condicionando APIs e operadores à validação. ADR-046 restringe automação e preserva funções essenciais. ADR-056 exige compatibilidade N/N-1 por perfil e rejeição antes do efeito. ADR-057 vincula cursor e posição ao escopo canônico. ADR-058 distingue classes de ACK. ADR-059 limita grants offline. ADR-060 rege confidencialidade. ADR-061 rege outbox e replay. ADR-062 exige política de retenção antes da persistência. ADR-063 rege artefatos e atualizações assinadas.

Alternativas de inferir capacidade a partir do modelo do dispositivo, inferir satélite pelo SSID ou acionar ajustes externos automaticamente são rejeitadas pelas fontes de capacidades e contingência. A consequência é maior frequência de estado desconhecido e necessidade de confirmação contextual. A projeção cumulativa de capacidades e o wire permanecem propostas, conforme [capacidades](../02-dominio/capacidades.md) e [gateway](../03-integracoes/gateway-universal.md). Dependências incluem SPEC-010 para adaptadores, SPEC-015 e SPEC-016 para APIs de plataforma, além de contratos de identidade, SOS, Guardian e sincronização. Nenhuma divergência de autoridade foi resolvida por este rascunho.

## 3. Capacidade e disponibilidade

Cada avaliação proposta identifica sujeito, capability, contexto, origem, evidência atribuível, instante de observação e política de frescor. Fabricante, usuário autorizado ou adaptador autorizado pode declarar suporte com proveniência. A declaração sustenta `supported`. Avaliador autorizado pode marcar `verified` mediante evidência atribuível e validada para sujeito e contexto. `operational` exige observação atual da função e dependências no contexto, método e janela de frescor configurados. Sem evidência suficiente, método ou frescor, apresentar `unknown` na dimensão pertinente.

Mudança de sujeito, perfil, contexto, dependência ou evidência provoca reavaliação. Expiração remove disponibilidade operacional sustentada, preservando declaração e evidência histórica conforme política. Falha observada produz degradação ou indisponibilidade contextual com motivo, sem transformar ausência de observação em falha. Recuperação exige nova evidência válida. Autorização, elegibilidade de rota e readiness são decisões independentes. Nenhuma capacidade concede acesso ou prova entrega. A projeção `operational ⊆ verified ⊆ supported` permanece proposta, dependente de conciliação contratual.

## 4. Evidência de conectividade

Estado de rádio, caminho IP, alcance do serviço, satélite e capacidade são dimensões separadas. Registrar fonte, contexto, instante, permissão e frescor da observação. `normal`, `degraded`, `offline`, `limited_online` e `unknown` seguem o [contrato de contingência](../07-contratos/conectividade-contingencia.md). Critérios, histerese e janelas são OPEN.

| Classe | Evidência mínima para a afirmação | Limite da afirmação |
|---|---|---|
| IPv6 ou IPv4 | Caminho da família exposto pelo SO; alcance GodMesh requer verificação autorizada específica | Endereço ou interface não prova internet, destino, serviço ou entrega. IPv6 é preferencial quando elegível, IPv4 é fallback. |
| Celular terrestre | API oficial identifica transporte celular terrestre no contexto observado | Sinal ou registro não prova internet, operadora específica ou alcance GodMesh. |
| Wi-Fi | API oficial identifica caminho Wi-Fi | SSID não prova proprietário, provedor, internet ou terminal satelital. |
| Satélite de operadora | API oficial disponível identifica transporte satelital para o contexto | Indicador, dica do sistema ou relato do usuário conserva sua origem e não vira prova de API. Satélite não prova suporte a dados do app. |
| Terminal satelital Wi-Fi | Evidência atribuível de terminal e associação pertinente, além do caminho Wi-Fi | Wi-Fi isolado não identifica terminal ou provedor. Sua disponibilidade não prova rota GodMesh. |
| Local ou gateway | Descoberta permitida e observação da interface ou gateway autorizado | Não prova internet, autorização de transmissão ou prontidão de serviço. |

Ausência de telemetria, queda celular ou falha de probe não prova presença ou ausência de satélite. Elegibilidade de serviço exige evidência própria, política e autorização. APIs e suporte por versão, aparelho e operadora dependem de validação. Fontes de plataforma e limites estão em [DICE](../01-arquitetura/cliente-mobile-sensores-conectividade.md), [contingência](../01-arquitetura/conectividade-contingencia-satelite.md), [SPEC-015](SPEC-015-android-kotlin.md) e [SPEC-016](SPEC-016-ios-swift.md).

## 5. Orientação manual e erros

Quando API e consentimento permitirem, o app pode usar uma rota já disponibilizada e abrir ajustes suportados. Caso contrário, apresenta instruções adequadas à plataforma para o usuário consultar opções oficiais. `suggested`, `opened_settings`, `confirmed_by_user`, `not_possible` e `dismissed` descrevem a interação. Confirmação humana conserva sua origem. O estado externo permanece `unverified` sem observação oficial suficiente. Abrir ajustes não comprova ativação. O app não altera assinatura, operadora, satélite ou aplicativos terceiros, nem desliga comunicação essencial do SO.

Erros seguem o contrato proposto, incluindo `action_user_required`, `satellite_service_unavailable`, `satellite_app_data_not_supported` e `delivery_not_confirmed`. Permissão negada ou API ausente gera explicação e estado desconhecido. Cancelamento encerra a orientação sem alegar efeito. Revogação impede novas observações dependentes da permissão e aciona tratamento das cópias segundo política.

## 6. Prioridade, orçamento e recuperação

Sob restrição, a ordem proposta segue o contrato: `emergency` para SOS explícito e confirmação pertinente, `safety` para check-in e posição autorizada, `essential` para mensagens curtas e controle essencial, `interactive` para voz, `bulk` para anexos, mídia, ASR remoto e telemetria frequente. Cargas `bulk` cedem primeiro, depois `interactive` quando a qualidade necessária não for sustentada. Compressão, cadência e orçamento são por perfil e serviço, sujeitos a medição e aprovação. Comunicação de emergência do SO permanece acessível.

SOS nunca é descartado silenciosamente. Se persistência autorizada falhar ou não houver política ADR-062, apresentar falha explícita e orientação para meios oficiais disponíveis. Quando persistido, manter estado pendente e evidência real. `persisted_local`, `transport_accepted`, `delivered_to_device` e `acknowledged_human` são distintos conforme ADR-058. Não prometer entrega ou atendimento.

Offline, operações dependem do grant limitado da ADR-059. Repetições têm limite, timeout e backoff por perfil ainda OPEN. Exaustão, cancelamento e armazenamento indisponível são visíveis, preservando o estado comprovado. Retentativa usa identidade idempotente e reconciliação por contrato aprovado. Replay não repete efeito externo conforme ADR-061. Reconexão reavalia rota, autorização, validade e prioridade antes de enviar. Nenhum valor de prazo, taxa, tamanho ou energia foi escolhido.

## 7. Segurança, dados e evolução

Observações são privadas ao dispositivo e serviços autorizados, com escopo de tenant derivado de identidade autenticada. Minimizar coleta e exposição, sem IMSI, número telefônico, identificadores de célula ou BSSID em chat. Compartilhamento exige finalidade e autorização. Persistência exige política por finalidade, classe, cópia, jurisdição, prazo e descarte conforme ADR-062. Sem política, falhar explicitamente e degradar. Revogação é por destino e cópia. Aplicar confidencialidade da ADR-060 e limites de rádio de [uso das redes](../04-seguranca/uso-das-redes.md).

Eventos propostos em [contrato de contingência](../07-contratos/conectividade-contingencia.md) precisam conciliação com GMP, identidade e sincronização antes de aprovação. Migração preserva dados e campos desconhecidos conforme perfil e ADR-056, rejeita incompatibilidade crítica antes de efeitos e exige plano de rollback. Cursores seguem ADR-057. Não há dependência nova selecionada nesta proposta.

## 8. Aceite e verificação planejada

Todos os casos abaixo estão **NOT_EXECUTED**. Cada linha representa resultado observável planejado, incluindo recuperação onde aplicável.

| Caso | Positivo | Negativo | Borda | Falha e recuperação |
|---|---|---|---|---|
| C003 / RF-003 | Evidência atribuível e fresca sustenta nível contextual | Declaração isolada não promove verificação nem autoriza | Frescor ausente resulta desconhecido operacional | Dependência falha, degrada; nova evidência válida permite reavaliação |
| C031 / RF-031 | API identifica classe e teste autorizado distingue alcance | SSID ou perda celular não afirma satélite | IPv6 e IPv4 coexistem com resultados independentes | API indisponível resulta desconhecido; nova observação atualiza estado |
| C032 / RF-032 | Ajustes suportados abrem e confirmação humana é identificada | Abrir ajustes não afirma ativação | Usuário dispensa instrução sem efeito | API ausente ou permissão negada apresenta instrução e estado não verificado; nova observação confirma somente o que expõe |
| C033 / RF-033 | Restrição prioriza SOS, check-in e mensagem na ordem proposta | ACK local não vira entrega ou atendimento | Orçamento indisponível preserva estado explícito e pausa carga discricionária | Falha de armazenamento de SOS é visível; reconexão reavalia e retoma envio autorizado sem duplicar efeitos |

## 9. OPENs e evidências

| Pendente | Responsável proposto | Condição de fechamento |
|---|---|---|
| Método de prova, atores autorizados e frescor por capability | Gregor, Kim | Política e contrato aprovados com casos por contexto; experimento de frescor de capacidades mede janelas. |
| Matriz de APIs, versões, operadoras e evidência satelital | Vint, Chet, Donny | Matriz documentada e ensaio em dispositivos e serviços autorizados. |
| Orçamento, thresholds, histerese, timeout e retry | Brendan, Gregor, Lea | Experimento de conectividade restrita mede tráfego, energia e recuperação por perfil; valores aprovados após resultados. |
| Contratos, idempotência, estados e migração | Graydon, Kleppmann, Gregor | Wire e integração com SPEC-010/015/016 e ADR-056–061 conciliados e aprovados. |
| Retenção, consentimento e revogação por cópia | Kim, Bruce e responsáveis de produto/jurídico | Políticas ADR-062 por finalidade e jurisdição aprovadas. |
| Revisão independente e aprovação | Vint Cerf; CTO para aprovação | Parecer identificado sobre esta versão, resolução dos achados e decisão formal registrada. |

Revisão independente **pendente**. Aprovação **pendente**. Código, testes, medições, hardware, operadoras e serviços externos **NOT_EXECUTED / não verificados**. Este documento não demonstra disponibilidade operacional.

## Casos por decisão (ADR-056…063)

Casos planejados desta versão, complementares a §8 (C003/C031/C032/C033) e sem substituí-los. Nenhum executado; identificador formal de teste `NOT_ASSIGNED`. Estados de conectividade seguem o contrato de contingência (`normal`, `degraded`, `offline`, `limited_online`, `unknown`); `ack_class` é o conjunto fechado da ADR-058. Parâmetro OPEN (frescor por capability, histerese, orçamento, timeout, retry, janela N/N-1, retenção) resulta em **negar ou declarar `unknown`**, nunca em valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S023-056-P | ADR-056 compatibilidade | Cliente e serviço no par declarado, sem janela N/N-1 aprovada para o contrato de contingência e o schema de eventos | Enviar evento de observação de conectividade | Evento carrega `compatibility=param_not_configured` no registro de versão; campo desconhecido preservado na leitura; nenhuma versão declarada suportada nem janela presumida | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-056-N | ADR-056 compatibilidade | Cliente com campo obrigatório ausente ou feature crítica incomum | Enviar evento e solicitar atualização de estado | Rejeição antes do efeito; evento não persistido; estado do cliente permanece `unknown`, sem downgrade silencioso | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-056-B | ADR-056 compatibilidade | Schema sem perfil publicado ou com número de campo removido | Migrar registro antigo e lê-lo com binário novo | Campo removido em `reserved`; nenhum número reutilizado; registro antigo legível; incompatibilidade crítica rejeitada antes do efeito | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-056-F | ADR-056 compatibilidade | Migração em curso com versões mistas e falha de cliente | Reenviar evento a partir do cliente antigo | Evento rejeitado; nenhum registro gravado com schema divergente; cliente exibe indisponibilidade com motivo; migração não avança sem par admitido | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-057-P | ADR-057 posição/cursor | Consulta de histórico de observações com cursor versionado vinculado a principal, escopo, filtros, perfil e retenção | Solicitar a próxima página | Página aceita; posição canônica completa registrada; nenhuma observação omitida ou duplicada | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-057-N | ADR-057 posição/cursor | Cursor sem `partition` ou de `authority_epoch` incomparável | Reapresentar o cursor após partição/reconexão | Cursor rejeitado; nunca traduzido; estado de leitura `unknown`; nenhuma posição sintetizada | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-057-B | ADR-057 posição/cursor | Duas partições com a mesma `seq` e `dispatch_epoch` presente no mesmo fluxo | Consultar e comparar posições | `seq`, `authority_epoch`, `dispatch_epoch` e cursor mantidos como tipos distintos; nenhuma comparação numérica entre eles; dois eventos preservados | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-057-F | ADR-057 posição/cursor | Falha de armazenamento e restauração com cursor emitido antes | Retomar envio com o cursor antigo | Falha fechada; cursor recusado; estado comprovado preservado; nenhum reenvio que dependa de posição inventada | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-058-P | ADR-058 ACK por classe | Operação de SOS/mensagem persistida localmente com prova de commit | Consultar estado na UI | `ack_class=persisted_local` exibido como tal; `delivered_to_device`/`acknowledged_human` ausentes sem evidência do destino/pessoa | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-058-N | ADR-058 ACK por classe | Apenas ACK local ou de transporte disponível | Exibir estado de entrega ou de atendimento | UI e API não convertem ACK inferior em entrega ou atendimento; texto mostra a classe real; nenhuma promoção por tempo ou ausência de erro | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-058-B | ADR-058 ACK por classe | Resposta de API sem `ack_class` | Consumir resposta de envio | Resposta tratada como inválida; nenhum sucesso presumido; registro de erro explícito no cliente | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-058-F | ADR-058 ACK por classe | Exaustão de retry com confirmação perdida e armazenamento local íntegro | Consultar estado e reenviar | Estado `unknown` visível, nunca sucesso; reenvio idempotente com identidade estável; nenhuma duplicação de efeito | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-059-P | ADR-059 grant offline | Grant assinado vigente, `epoch` comparável e sem revogação conhecida, mas sem limite de frescor aprovado | Abrir incidente SOS local e enfileirar offline | `grant=valid` e frescor `unknown` com `freshness_limit=param_not_configured` registrados em dimensões separadas; nada enfileirado e nenhuma promessa de entrega; nenhum limiar padrão aplicado | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-059-N | ADR-059 grant offline | Grant `stale`, `unknown` ou relógio incerto | Tentar enviar mensagem ou registrar observação offline | Operação negada com motivo explícito; se for SOS, falha visível e orientação para meios oficiais disponíveis; nada descartado silenciosamente | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-059-N2 | ADR-059 grant offline | Grant vigente para as operações enumeradas | Solicitar divulgação de posição precisa, transmissão RF, mutação federada ou transcrição remota | Negação por operação não enumerada, independente de frescor e de conectividade; nenhuma fila criada | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-059-B | ADR-059 grant offline | Janela de frescor não aprovada e relógio do aparelho sem confiança | Avaliar grant e exibir estado de conectividade restrita | Negação classificada com `freshness_limit=param_not_configured`; estado de conectividade permanece `unknown`/`limited_online` conforme evidência; nenhum limiar embutido e nenhuma revogação do grant declarada por parâmetro ausente | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-059-F | ADR-059 grant offline | Snapshot restaurado com grant na validade e revogação conhecida em seguida | Reexecutar a operação após restore | Grant negado; revogação prevalece sobre cache; efeito dependente bloqueado; preservação apenas de fluxos independentes autorizados | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-060-P | ADR-060 confidencialidade | Perfil que declara perna terminadora (bridge, ASR remoto consentido ou gravação) | Consultar observação/relato pela API e pela UI | `confidentiality=terminated` presente e fiel, com o terminador identificado; nenhuma alegação de E2EE | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-060-N | ADR-060 confidencialidade | Canal com TLS apenas na perna | Rotular a transmissão como E2EE ou anônima | Alegação rejeitada; UI não promete anonimato; texto restrito a proteção de perna | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-060-B | ADR-060 confidencialidade | Artefato de conectividade/telemetria sem metadado de confidencialidade (D9 aberta) | Consumir a cópia em relatório | Cópia tratada como `confidentiality=terminated`; ausência não é suprida por cache, versão anterior ou ausência de erro | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-060-F | ADR-060 confidencialidade | Nova perna terminadora entra no fluxo de um artefato antes E2EE | Reprocessar derivados e reemitir estado | Derivados herdam `terminated`; API rejeita alegação de E2EE; terminador visível ao usuário | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-061-P | ADR-061 transação/recuperação | Serviço com `outbox` na mesma transação do evento de contingência | Persistir observação e publicar | Commit único com `outbox`; publicação após commit; `ack_class` máximo coerente com o commit comprovado | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-061-N | ADR-061 transação/recuperação | Inbox durável separado da projeção | Consultar resultado do relato | ACK máximo `persisted_local`; `materialized` nunca devolvido; projeção pendente explícita | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-061-F | ADR-061 transação/recuperação | Queda entre commit e publish durante reconexão | Reiniciar e reconectar | Republicação idempotente, sem duplicar mensagem, chamada a bridge ou notificação; reconexão reavalia rota, autorização, validade e prioridade antes de enviar | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-061-F2 | ADR-061 transação/recuperação | Partição com autoridade `stale`/`unknown` e operação antiga em voo | Retomar envio e mutações dependentes | Mutação forte bloqueada; resultado incerto permanece pendente até reconciliação; replay reconstrói projeção e não repete efeito irreversível | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-061-B | ADR-061 transação/recuperação | Observação commitada com `outbox` e publicação pendente | Consultar o resultado da observação antes do publish | `ack_class` máximo `persisted_local` com prova de commit; `materialized` e efeito a jusante ausentes; leitor não publica nem promove classe | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-062-P | ADR-062 retenção fail-closed | Classe de observação/telemetria com política aprovada em `(finalidade, classe, cópia, jurisdição)` e prazo/gatilho não decididos | Gravar observação e consultar a retenção aplicada | Registro grava `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo ou gatilho aplicado como aprovado; estado de descarte por cópia permanece `unknown` | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-062-N | ADR-062 retenção fail-closed | Classe de observação sem política aprovada | Persistir histórico de conectividade | Falha explícita e degradação visível; apenas mínimo necessário ao efeito imediato, não gravado como histórico; nenhum `persisted_local` para conteúdo não autorizado | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-062-B | ADR-062 retenção fail-closed | Política com prazo desconhecido (OPEN) | Consultar retenção da observação | Prazo `unknown`; nenhuma retenção indefinida implícita; histórico não é criado apenas por falta de prazo | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-062-F | ADR-062 retenção fail-closed | Permissão do SO revogada durante coleta | Revogar permissão e reabrir o app | Novas observações dependentes cessam; cópias tratadas por destino; estado por cópia (bloqueio, pendência, descarte confirmado) exibido; nenhuma alegada eliminação global | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-063-P | ADR-063 supply chain/anti-rollback | Build de app com SBOM, assinatura verificada, canal autenticado e `generation` ≥ piso | Instalar a release e repetir o build | Instalação aceita; digest do build repetido igual; SBOM e `generation` registrados na evidência de release | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-063-N | ADR-063 supply chain/anti-rollback | Release sem assinatura verificável ou artefato adulterado | Instalar e habilitar as capacidades | Instalação rejeitada antes do efeito; capacidades não habilitadas; release não distribuível; erro sanitizado | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-063-N2 | ADR-063 supply chain/anti-rollback | Piso de versão assinado superior ao artefato ofertado | Instalar artefato com `generation` inferior | Rejeição registrada; nenhuma substituição; downgrade para release vulnerável proibido | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-063-B | ADR-063 supply chain/anti-rollback | Piso de `generation` não publicado para o app e releases N/N-1 assinadas | Instalar a release mais recente e consultar anti-rollback | Piso exposto como `param_not_configured`; instalação não declara `generation` aceita contra piso desconhecido; nenhum piso presumido a partir da versão instalada | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S023-063-F | ADR-063 supply chain/anti-rollback | Lista de revogação `unknown` ou chave comprometida | Atualizar e, depois, executar rollback autorizado | Atualização bloqueada enquanto a revogação for `unknown`; revogação/reemissão verificada no cliente; rollback aceito só como exceção autorizada, limitada e auditada | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §1. Células citam casos de §8 e da tabela `Casos por decisão`, ou ID novo descrito na Nota; prefixo `S023-R###` agrupa os quatro tipos. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. Frescor, histerese, orçamento, timeout e retry seguem OPEN (§9) e aparecem como `param_not_configured`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-003 | C003; S023-R003-P | C003 (negativo); S023-R003-N | S023-R003-B | C003 (falha); S023-R003-F | P: evidência atribuível sustenta `supported`/`verified` no contexto. N: declaração isolada não promove a `verified` nem autoriza. B: frescor não configurado → `operational=unknown`, sem herdar suporte. F: expiração remove disponibilidade operacional preservando histórico. Projeção `operational ⊆ verified ⊆ supported` segue proposta. |
| RF-031 | C031; S023-056-P | C031 (negativo); S023-R031-N | C031 (borda); S023-R031-B | C031 (falha); S023-R031-F | P: apresentação de classe com fonte, instante e permissão. N: SSID, queda celular ou ausência de probe não afirmam satélite. B: IPv6 e IPv4 coexistem com evidência independente por classe. F: API indisponível → `unknown` e orientação manual (RF-032). |
| RF-032 | C032; S023-R032-P | C032 (negativo) | C032 (borda) | C032 (falha) | P: rota já disponibilizada e ajustes suportados abertos quando permitido. N: abrir ajustes não comprova ativação. B: `dismissed` encerra a orientação sem efeito declarado. F: permissão negada/API ausente → instrução e `unverified`. |
| RF-033 | C033; S023-058-P | C033 (negativo); S023-058-N | C033 (borda); S023-059-B | C033 (falha); S023-061-F2 | P: ordem proposta de prioridade com classe de ACK real. N: ACK local não vira entrega ou atendimento. B: orçamento não configurado mantém estado explícito e pausa carga discricionária. F: falha de armazenamento de SOS é visível; reconexão reavalia rota/autorização/prioridade sem duplicar efeito. |
| ADR-057 (cursores de conectividade) | S023-057-P | S023-057-N | S023-057-B | S023-057-F | Posição canônica preservada; nenhum `seq` de partições distintas colapsado. |
| ADR-060 (observações) | S023-060-P | S023-060-N | S023-060-B | S023-060-F | Metadado de confidencialidade obrigatório; ausência tratada como `terminated`. |
| ADR-063 (app que coleta) | S023-063-P | S023-063-N; S023-063-N2 | S023-063-B | S023-063-F | Piso e revogação de chave verificados antes de habilitar capacidades de conectividade. |
