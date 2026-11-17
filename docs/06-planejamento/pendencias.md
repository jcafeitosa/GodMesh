# Riscos, dúvidas em aberto e validações

## Alto risco
| Risco / dúvida | Consequência | Mitigação / decisão pendente |
|---|---|---|
| “Espelhar tudo” globalmente | Vazamento, alto custo e residência de dados | Política de replicação por tenant/região; consentimento |
| Uso de FRN público com ponte RF | Uso indevido do espectro, bloqueio, responsabilidade | Iniciar RX-only e ambientes autorizados; revisão legal |
| PTT nativo em navegador | Transporte, jitter, compatibilidade e segurança de mídia | Implementar WebRTC/Opus e validar carga; Mumble apenas bridge opcional |
| Evolução do protocolo FRN legado | Instabilidade e incompatibilidade | Testar servidores reais autorizados, fuzz/parsing, isolamento |
| Controle de TX duplicado em failover | Áudio simultâneo/loops | Leases, fencing, anti-loop e auditoria |
| Disponibilidade após partição | Split brain e divergência de ACL | Separar operações eventuais de fortes, quórum |
| Integrações BLE proprietárias | Sem API pública, rastreio indevido | Não prometer suporte Apple/Google/Samsung; usar tags próprias/autorizadas |
| SOS como serviço crítico | Usuário pode supor garantia inexistente | SLOs reais, redundância, avisos, teste, fallback físico |

## Questões a decidir
1. Nome comercial e propriedade dos adaptadores; licença pública do repositório sob MIT foi decidida para o conteúdo publicado, sem decidir licença de eventuais módulos futuros.
2. Região inicial e países-alvo para exigências de privacidade e rádio.
3. Escala esperada (usuários ativos, dispositivos, uplinks/s, salas PTT concorrentes, tráfego de áudio).
4. Modelo de provisionamento de servidores independentes por terceiros e políticas de confiança.
5. Autoridade de escrita por tenant: “home region”, líder de partição, ou serviço dedicado.
6. NATS JetStream x log de eventos próprio, modelo de outbox/inbox e garantia de retenção.
7. O plano de mídia nativo WebRTC/Opus para o navegador e os requisitos de interoperabilidade do Bridge SDK Mumble.
8. Quais servidores FRN específicos aceitarão conexão e bridges, sob quais credenciais.
9. Firmware, formatos e fabricante dos primeiros terminais LoRaWAN/BLE.
10. Modelo de monetização e isolamento de tráfego pessoal/comercial/radioamador.
11. SLA/SLO e orçamento de infraestrutura; custo de egress entre continentes.
12. Como coordenar atualizações, migrações de schema e rollback em rede intermitente.

## Experimentos prioritários
- Teste de IPv6/IPv4 em três nuvens/ASNs e redes restritas.
- GodMesh Native Voice entre cliente web e nativo, com Mumble testado somente como bridge externa.
- Cliente FRN Rust testado em servidor próprio/autorizado.
- Partição de rede no sync + revogação de autorização durante isolamento.
- Balanceador diante de um nó que aparenta healthy mas está saturado.

## Integrações móveis/satelitais pendentes
- Homologar matriz de aparelhos Android/iOS para anúncio BLE e detecção em segundo plano; confirmar limites por OS e fabricante.
- Confirmar suporte a serviços satelitais por operadora e país. Uma assinatura **não** garante transporte de dados do GodMesh nem voz PTT.
- Especificar receitas de orientação manual por marca/versão, localizadas e atualizáveis sem alterar núcleos Rust.
- Validar medidas para bateria baixa que mantenham localização, alertas e funções essenciais ativas; definir comportamento em restrições severas do SO.
- Definir métricas e consentimento para monitoramento de rede móvel sem capturar identificadores privados da infraestrutura das operadoras.
- Validar proteção contra rastreamento indevido para tags físicas futuras e operações CrowdFind.

## Wi-Fi Share e Live Presence — pendências de homologação

- Homologar roteadores/controladores compatíveis com guest SSID, PPSK, Passpoint, RADIUS/EAP-TLS, isolamento, contadores e revogação por dispositivo.
- Definir política de minimização, retenção e revisão jurídica por país para logs de sessões, endereços IP, locais exatos de AP e tratamento de dados de visitantes.
- Especificar prova de autoridade sobre AP, inclusive delegação empresarial e contestação de ponto cadastrado indevidamente.
- Validar coordenadas exatas de AP, sua fonte/qualidade, consentimento separado e comportamento do cache/mapa após revogação.
- Testar capacidades Android/iOS por versão/entitlement para discovery, conexão, background e Live Activities/Live Updates.
- Definir quotas, antiabuso, rate limits, replicação e incident response de credenciais com controlador offline.
- Confirmar experiência de UX para multiplas atividades simultâneas PTT + Guardian + SOS, sem expor localização/segredos no lock screen.

## Transição do planejamento para SPECs — SDD + TDD

Diretriz do usuário: adotar SDD (desenvolvimento orientado por especificações) e TDD (desenvolvimento orientado por testes). A autoria de SPECs pode ocorrer em paralelo à resolução das pendências. Planejamento concluído e aprovação de todas as SPECs continuam condições anteriores à implementação.

Critérios para encerrar o planejamento:

1. Cada achado da auditoria tem correção confrontada com seu critério original ou dependência residual explícita; propostas e validação pendente não contam como fechamento.
2. Decisões bloqueadoras têm alternativas, recomendação, impacto, responsável e decisão registrada antes do encerramento integral; o escopo afetado deve estar identificado; o rascunho paralelo não implica liberação para implementação.
3. Requisitos e contratos possuem responsáveis e critérios de aceite verificáveis; divergências transversais foram conciliadas e revisadas independentemente.
4. A matriz permite ligar requisito → futura SPEC → aceite → testes. Identificadores de SPEC e testes ainda inexistentes permanecem como não atribuídos, nunca como evidência de cobertura.
5. A revisão final explicita dependências e impedimentos por escopo; a coordenação registra a conclusão do planejamento e a aprovação de todas as SPECs antes da implementação. A autoria pode avançar em paralelo.

Cada SPEC em elaboração deve declarar escopo, requisitos, contratos, invariantes, falhas, aceite, casos de teste e decisões/dependências. Contratos documentais atuais são insumos; não se tornam SPECs aprovadas por renomeação. Na implementação futura autorizada, o ciclo será SPEC rastreável → teste que falha pelo comportamento esperado → implementação mínima para passar → refatoração → evidência. Testes não executados permanecem assim identificados. Esta diretriz não amplia a autorização documental atual.

## Escopo integral do planejamento

- Planejar o GodMesh como sistema completo e avançado, sem redução a MVP. Etapas de entrega preservam o escopo integral e os critérios de qualidade; resolver planejamento, correções e decisões bloqueadoras em paralelo à autoria de todas as SPECs → concluir planejamento e aprovar todas as SPECs → implementação futura autorizada com SDD + TDD. A autoria atual não comprova conclusão nem autoriza implementação.

## Plataformas — fechamento da ADR-055

Plataformas e SwiftUI Apple estão decididos. Permanecem pendentes: avaliação e seleção final das recomendações WinUI 3 (Windows), Qt Quick (Linux) e Kotlin/Jetpack Compose (Android); bindings Rust; versões mínimas e arquiteturas por plataforma; imagem Linux e distribuição; licenças por módulo; metas e evidências de recursos, acessibilidade e jornadas. A escolha da plataforma não fecha validação nem autoriza implementação. Critérios, fontes e responsáveis propostos estão na ADR-055.

## Classificação das saídas de planejamento — integração 2026-11-16

As questões históricas acima permanecem abertas até decisão rastreável ou reclassificação justificada. Não são todas automaticamente bloqueadoras: nome comercial, monetização e homologação externa exigem análise de impacto. Nenhuma classificação ou checklist fecha GMD por si só. Alternativas abaixo são de planejamento, não escolhas aprovadas.

| Classe | Escopos / saída necessária | Responsáveis documentais propostos | Alternativas / recomendação sustentada | Critério e estado |
|---|---|---|---|---|
| Decisão bloqueadora: carga e qualidade | Cenários Personal/Teams/Node/Platform, geografia, carga nominal/pico, recursos, latência, custo e risco aceitável; servidor e clientes | Margaret, Grady, Brendan | Consolidar cenários e premissas antes de aprovar limites; não inventar medições | Metas/limites aprovados ou impedimento explícito; aberto |
| Decisão bloqueadora: autoridade e recuperação | Escrita por organização, fencing, retenção/dedupe/recibos e restauração; dados, federação, sync e dispatch | Leslie, Kleppmann, Michael, Momjian | Inbox/projeção atômicas quando na mesma transação ou Inbox durável/ledger e projetor idempotente quando separadas: alternativas já existem; escolher fronteira física e dono | Texto normativo conciliado, política aprovada e esperados de recuperação; aberto |
| Decisão bloqueadora: política e segurança | Jurisdição, E2EE por operação, retenção, consentimento/revogação e autoridade de contenção | Schneier, Kim, Lawrence, Richard | Preservar controles atuais; complementar crosswalk por operação e sinal sanitizado, em vez de presumir inexistência de defesa | Política e autoridades registradas; sem inferir nomes de plantão; aberto |
| Decisão bloqueadora: compatibilidade e stack | Rust–Swift, armazenamento/cancelamento/lifecycle, SO mínimo, distribuição e seleção das bibliotecas; plataformas futuras preservadas | Fowler, Alex, Donny, Graydon | Arquitetura única com contratos por camada; candidatos ainda propostos; fontes oficiais versionadas antes da seleção | Compatibilidade/alternativa e risco fundamentados, decisões registradas; aberto |
| Decisão bloqueadora: observabilidade operacional | Dependências por rota, fonte, janela, SLO, alertas, acesso, retenção, RPO/RTO e responsáveis operacionais | Brendan e principais por domínio | Registro por sinal/SLI/alerta e famílias C13 já existentes; sem duplicar autoridade | Alvos/políticas e efeitos aprovados, esperado de teste explícito; aberto |
| Detalhe de futura SPEC sob direção aprovada | Wire/schema/enums, contratos de erro, parâmetros, casos determinísticos e integração API/eventos/dispatch | Graydon, Kleppmann, Gregor e donos | Crosswalk único recomendado por Fowler; não definir retry/lifecycle isolados por serviço | Só é detalhe de SPEC quando autoridade, política e invariantes já decididas; caso contrário retorna a bloqueador |
| Detalhe de futura SPEC: cenário e evidência | Pré-condição, estímulo, esperado, falha/recuperação, ambiente e caso por requisito/produto/plataforma | Kent, James e donos | Matriz central de referências com procedimentos locais; proposta de James/Kent | IDs NOT_ASSIGNED até autoria; formato não é execução nem aprovação |
| Validação futura | Builds, compatibilidade real, energia, carga, ataques autorizados, restauração, hardware/integrações e acessibilidade | Donos técnicos e validação | Planejar método/critério agora; executar somente em etapa autorizada | Observado NOT_EXECUTED; dependência de viabilidade ainda desconhecida pode impedir planejamento |
| Reprodução da infraestrutura | Artefatos OrbStack/Compose fora do projeto segundo Brendan; origem, configuração sanitizada e integração ao repo | Brendan e Scott | Documentar origem e caminho real, segredos separados; nenhuma cópia de segredo ou mudança de stack nesta integração | Localização/proveniência e plano de reprodução conferidos; implantação não equivale a aceite de produto |

### Crosswalk de lifecycle e recuperação

Conciliar eventos C10, dispatch C15 e GMP sem colapsar event_sequence, dispatch_sequence, dispatch_epoch e authority_epoch. As alternativas de Inbox/materialização já documentadas em eventos-sincronizacao.md §6.2 não estão ausentes: falta selecionar fronteira física, dono transacional/ledger, retenção e integração com dados. Registrar restore/perda de registro, deadline com outcome_unknown e efeitos proibidos. Vínculo de GMD individual exige conferência do registro original pela coordenação; não atribuído por esta integração.

### Encerramento integral

A classificação identifica onde decidir e o que detalhar; permite autoria documental paralela, sem liberar implementação. Alan concilia todas as parcelas e achados com seus critérios originais; Edsger revisa o delta integrado; usuário decide alternativas que lhe competem. Não exigir produto implementado ou testes passando para encerrar planejamento, nem declarar completo enquanto houver decisão bloqueadora não resolvida. Esperado pode ser planejado; observado permanece NOT_EXECUTED. Esta integração não declara encerramento.

## Decisões residuais de produto e engenharia — atualização 2026-11-16

As questões seguintes permanecem propostas documentais e pendentes de decisão; nenhuma integração documental aprova política ou substitui revisão do usuário.

| Decisão real / saída | Escopos dependentes | Estado |
|---|---|---|
| Esclarecer sentido de acessibilidade | RNF-001; transporte e eventual UX | Aberto; alcance de rede sustentado pelo contexto não redefine a origem |
| Definir predicados de entrega/ACK, retenção e autoridade | RF-011, RNF-009, RNF-017; sync, comandos e SOS | Aberto; separar evidência de camada, presença efêmera e reconhecimento humano |
| Aprovar consentimento, revogação offline, E2EE e retenção | Dados sensíveis, ASR, replay/restore e clientes | Bloqueador quando muda garantias; não inferir política de cenário proposto |
| Aprovar carga, energia, frescor, limites, custo e recuperação | Serviços, clientes, observabilidade e AP | Limiares e método devem ser decididos; resultados só em execução futura |
| Conciliar evidências Wi-Fi e titularidade | RNF-013/RNF-014; API/eventos e plataformas | Preservar provação na origem; sem novo enum; internet_verified sem método/fonte/frescor definidos |
| Confirmar elegibilidade/compatibilidade por versão | Apple, Android e demais clientes | Responsabilidade documental não comprova API disponível ou suporte real |

Decisões acima e demais decisões por requisito são bloqueadoras quando alteram contratos, risco ou garantias. Uma questão só é classificada como detalhe de SPEC quando direções, invariantes e políticas aplicáveis estiverem aprovadas; se ainda alteram contratos, risco ou garantias, permanecem decisões bloqueadoras. Rascunhos podem registrar alternativas e OPEN em paralelo, sem antecipar decisão ou aprovação. Homologação, carga, segurança, restauração e compatibilidade executadas são validação futura, observado NOT_EXECUTED; dúvida de viabilidade não resolvida retorna ao planejamento. Nenhum checklist fecha GMD. Não exigir produto passando para encerrar planejamento, nem liberar implementação antes do encerramento do planejamento e aprovação integral das SPECs. Próxima saída: conciliação normativa e revisão independente do delta canônico; planejamento ainda não encerrado.

## Autorização posterior de desenvolvimento — 16/11/2026

Ada Lovelace transmitiu à coordenação a instrução explícita do usuário: “Vamos continuar o desenvolvimento de forma continua e sem parar ate que o sistema esteja 100% desenvolvido”. A autorização amplia o trabalho futuro para desenvolvimento e testes locais do produto integral. A orientação posterior autoriza criar SPECs em paralelo à conclusão do planejamento, correções e decisões bloqueadoras; preservam-se conclusão do planejamento e aprovação integral das SPECs rastreáveis antes de implementar com SDD + TDD, revisão independente e validação por critério. A autorização não aprova automaticamente decisões de produto, SPECs, pagamentos, publicação, mudanças destrutivas ou integrações reais externas. Trabalho contínuo não implica criar agendamentos ou rotinas.

Este registro não afirma encerramento de planejamento, aprovação de SPEC, implementação ou resultado de teste. Os estados anteriores e `NOT_ASSIGNED`/`NOT_EXECUTED` permanecem até evidência de sua efetiva alteração. Origem do registro: comunicação de Ada à coordenação, mensagem `7f997d94-41f0-4eb8-a106-88dcf5bd863a`; a coordenação não teve acesso direto à fala original nesta etapa.

## Direções de produto transmitidas em 16/11/2026

Ada comunicou que obteve respostas do usuário em duas perguntas estruturadas (mensagem `81f9d9d9-dffe-4bd8-907b-e77d1cc5d02d`). A coordenação registra a origem como relato de Ada; não acessou diretamente as respostas da interface. As escolhas informadas são direções de produto, sujeitas a conciliação e detalhamento, e não significam contratos integrados ou produto validado:

1. **Responsabilidade por modalidade:** aprovada a divisão-base da pauta 1A: provedor para a camada hospedada e cliente/operador para a instalação autogerida, conforme escopos e acordos; detalhamento por oferta, terceiros, cópias e suporte permanece pendente.
2. **Perda de prova ou histórico:** escolhida a linha de base 3A: bloquear mutações fortes sem autoridade/fence e operações dependentes de histórico não comprovado, preservando fluxos comprovadamente independentes segundo suas próprias políticas. O estado desconhecido não vira sucesso por decurso de tempo.
3. **Proteção e retenção:** escolhido piso global de proteção com restrições locais e perfis de retenção por finalidade, classes e cópias (4A e 6A da pauta). Não foram escolhidos prazos, finalidades concretas, acesso a conteúdo, exceções legais ou política de processamento por serviço/bridge.
4. **Offline:** escolhida “Autorização assinada e limitada”, com ações locais previamente autorizadas, grant válido e limites por operação, sem afastar as negativas vigentes para estado de autoridade `stale/unknown` nem presumir propagação instantânea de revogação. Ações, duração, frescor, limites e eventual tratamento SOS continuam a definir e revisar.
5. **Identidade:** escolhidas “Provas previamente cadastradas”; sem provas válidas, a identidade não é transferida. Não foi aprovada via assistida excepcional, recuperação de chaves de conteúdo, mecanismo ou política de contestação.
6. **PTT:** escolhida a possibilidade de preempção “Sim, com política e controle comprovado”: somente classes e autoridades aprovadas e prova de invalidação do grant anterior no controlador e no ponto de mídia; sem prova, negar ou adiar. Classes, limites, desempate, lease e comportamento em partição permanecem pendentes. Prioridade não concede autorização de transmissão RF nem comprova recebimento ou atendimento SOS.
7. **Capacidade e qualidade:** a equipe foi autorizada a propor perfis quantitativos com fontes e consequências para aprovação posterior. Nenhum número, orçamento, prazo, RPO/RTO, SPEC ou resultado de teste foi aprovado por essa autorização.

Confrontar estas direções com os contratos, preservar a precedência normativa e registrar eventuais diferenças materiais antes de integração. O gate de planejamento concluído e SPECs integralmente aprovadas antes de implementação e validação continua em vigor; a autoria pode ocorrer em paralelo, sem liberação parcial pela existência deste registro.
