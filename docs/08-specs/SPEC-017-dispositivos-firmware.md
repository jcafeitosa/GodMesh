# SPEC-017 — Dispositivos, firmware e energia

**Versão:** DRAFT-0.4  
**Estado:** DRAFT / OPEN — não aprovada  
**Autor documental:** Jack Ganssle  
**Revisor independente designado:** Vint Cerf  
**Data:** 2026-11-17  
**Requisito principal:** RF-010 (principal documental proposto; vínculo normativo a confirmar)  
**Casos de teste:** NOT_ASSIGNED; cenários abaixo são planejados e não executáveis/atribuídos.  
**Execução/implementação:** NOT_EXECUTED / não verificada.

## Mudanças desta versão

- ADR-056: firmware e adaptadores seguem N/N-1 por perfil com janela declarada; incompatibilidade rejeitada antes do efeito e sem downgrade silencioso; exceção somente em perfil publicado com autoridade registrada; Protobuf persistido segue regras de evolução.
- ADR-058: `ack_class` obrigatório com seis classes; ACK de adaptador/bridge nunca promove a `delivered_to_device` ou `acknowledged_human`.
- ADR-060: perna com proteção terminada declara `confidentiality=terminated`; áudio/telemetria declaram terminador e visibilidade do servidor; TLS/DTLS/SRTP não são E2EE.
- ADR-063: SBOM por release, build reproduzível, assinatura verificada no dispositivo, canal autenticado, rollback seguro e revogação de chave de assinatura.
- Casos por decisão (resposta ao veto C3-1).
- Casos por requisito e casos P desbloqueados (C3-1): subseção `Casos por requisito (P/N/B/F)` ao final, uma linha por requisito coberto; `S017-056-P`, `S017-059-P` e `S017-062-P` reescritos para o comportamento com parâmetro ausente (`param_not_configured`), sem inventar janela, frescor ou prazo; novos `S017-059-B`, `S017-061-B` e `S017-063-B` para bordas que faltavam.
- Estado documental: DRAFT-0.3 / OPEN; revisão independente do delta pendente.

## 1. Escopo e limites

Especifica requisitos documentais para adaptadores de dispositivos, proveniência, capacidades observadas, atualização/recuperação de firmware e estados de energia/saúde. Abrange dispositivos BLE, LoRa, LoRaWAN e GPS/LTE somente nas interfaces e limitações descritas pelas fontes; não presume modelo, MCU, bootloader, sensor ou mecanismo disponível. RF-010 é o vínculo principal atual da matriz, não prova de cobertura integral. B02 (adaptadores/localização) é coordenada por Gregor Hohpe; Jack contribui no subescopo de dispositivos, sem assumir a frente.

Esta SPEC não define criptografia própria, protocolo de atualização, valores de autonomia, TTL, janelas adicionais, política de rádio, autoridade de assinatura, downgrade, nem comportamento universal de fail-safe. Não substitui SPECs de identidade/autorização, integração, localização, UX/plataformas, SOS, operação ou conectividade. Decisões sem fonte estão marcadas OPEN.

## 2. Fontes e precedência

Fontes normativas lidas: `docs/README.md`, `docs/00-produto/visao.md`, `docs/01-arquitetura/arquitetura-referencia.md`, `docs/06-planejamento/decisoes.md`, `docs/06-planejamento/pendencias.md`, `docs/06-planejamento/matriz-rastreabilidade.md`; fontes do domínio: `docs/00-produto/requisitos.md` (RF-010, RNF-008), `docs/02-dominio/capacidades.md`, `docs/03-integracoes/lora-lorawan-ble.md`, `docs/03-integracoes/crowdfind-beacon.md`. Arquitetura de referência e ADRs prevalecem conforme README; esta SPEC não altera suas decisões.

Insumos do planejamento: `Planejamento-Integral-2026-11-16/contribuicoes/requisitos-jack.md` (RF-010) e `SPECS-EM-ANDAMENTO/contribuicao-jack-ganssle.md`. O manifesto registra SHA-256 `2351c4c73a873c70c766e2f5054ab330103bcbb497eff4042bee9d16cb079ebb` para a contribuição de mobilização. A contribuição anterior foi enriquecida aqui em contrato, estados, cenários, aplicabilidade e rastreabilidade, sem converter propostas em decisão.

Achados relacionados (auditoria documental externa ao repositório; identificadores canônicos `GMD-A34-001` a `GMD-A34-003`, conforme o lote C26): GMD-A34-001 atualização/recuperação, GMD-A34-002 energia, GMD-A34-003 saúde/watchdog; C26 separa capability, AccessDecision, elegibilidade de rota e readiness, mantendo a projeção cumulativa como proposta. Não são considerados fechados por esta SPEC. Evidências de hardware ou fabricante não foram consultadas.

## 3. Produtos, plataformas e atores

Aplicabilidade depende de produto, hardware e protocolo concretos. O requisito inclui Node e integrações de terminal; Personal/Teams e SDK podem consumir capacidades/eventos, mas não alegam possuir rádio ou bootloader do dispositivo remoto. Servidor Linux pode receber/normalizar eventos e coordenar política, mas não prova o estado físico do terminal. macOS arm64/x86_64, iOS, Windows, Linux desktop, Android e web podem participar conforme contrato e interface disponível; N/A requer decisão justificada por produto, não exclusão global.

Atores: titular/operador autorizado; serviço GodMesh consumidor; adaptador; dispositivo; fabricante/fornecedor de firmware como fonte de evidência técnica; autoridade de política de identidade, assinatura e autorização (OPEN, interface Kim/usuário). O adaptador não é autoridade para conceder acesso.

## 4. Contrato de adaptador e proveniência

Cada perfil de dispositivo deve identificar produto/modelo/revisão quando conhecidos, classe de interface/protocolo, versão do contrato de adaptação e capacidades declaradas, verificadas ou operacionalmente observadas de forma distinta. Se a variante não estiver identificada, compatibilidade fica desconhecida e não pode ser elevada a verificada.

Cada evento normalizado deve preservar origem e identidade do emissor conforme o contrato, método e instante da observação quando disponível, incerteza, transformação realizada e escopo/finalidade. Não converter IMEI, MAC/BSSID, token BLE ou identificador de fornecedor em `gmi_id` sem mapeamento autorizado e documentado. BLE beacon observado é `proximity_sighting`, não posição GNSS do emissor. Dados sem fonte, prova ou frescor exigidos permanecem ausentes/desconhecidos/rejeitados conforme política; não são tratados como atuais por conveniência.

O adaptador separa: recebido; estruturalmente válido; autenticado/provado conforme método acordado; aceito pelo serviço; efeito funcional confirmado pelo dispositivo. Toda resposta inclui `ack_class` obrigatório: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`, com emissor, evidência e escopo. ACK de rádio, fila, bridge ou adaptador não confirma instalação, execução ou atendimento e nunca promove a `delivered_to_device`/`acknowledged_human`; essas classes exigem evidência própria do destino/pessoa. Não se infere classe superior por tempo, classe inferior ou ausência de erro. Duplicação, ordenação, idempotência, cursor e retenção seguem contratos proprietários de integração/eventos; permanecem OPEN aqui onde não especificados.

#### 4.2 Confidencialidade (ADR-060)

Toda perna em que dispositivo, servidor, bridge ou adaptador termina proteção declara `confidentiality=terminated` na API e na apresentação correspondente. TLS, DTLS e SRTP são proteção da perna e não podem ser alegados como E2EE. Perfis com áudio/telemetria por bridge/adaptador declaram qual componente termina proteção e que conteúdo/campos ficam visíveis ao servidor. Para variantes ainda não selecionadas, terminador e visibilidade são desconhecidos — decisão pendente.

### 4.2a Anti-rollback de firmware (emenda ADR-063, achado C4)

Assinatura prova origem e integridade, não atualidade. Cada release de firmware carrega `generation` monotônica autenticada e o dispositivo mantém piso de versão assinado pela autoridade de release. Dispositivo rejeita artefato com `generation` inferior ao piso e recusa atualização quando a lista de revogação de chave estiver `unknown`. Rollback só ocorre por exceção autorizada, limitada no tempo e auditada, com rebaixamento explícito do piso; rollback para versão vulnerável conhecida é proibido. Chave de assinatura comprometida tem procedimento de revogação, reemissão e recuperação documentado, e o dispositivo registra a tentativa de downgrade rejeitada.

### 4.3 Compatibilidade por perfil (ADR-056)

Cada perfil versionado de firmware e adaptador declara a janela N/N-1. Duração numérica e perfis suportados permanecem desconhecidos — decisão pendente. Versão/feature crítica incompatível é rejeitada antes de qualquer efeito, sem downgrade silencioso. Exceção somente por perfil publicado com janela, impacto, autoridade que autorizou e plano de rollback registrados. Schema persistido em Protobuf não reutiliza nem altera número de campo, preserva campos desconhecidos e move campos removidos para `reserved`.

## 4.1 Perfis de rádio

- LoRaWAN Classe A: downlink só pode ser planejado nas janelas protocolárias após uplink conforme fonte normativa e classe do terminal. Fila/aceite anterior não garante oportunidade ou entrega.
- Classe C: disponibilidade contínua depende de classe, energia, hardware e rede efetivamente suportados; não assumir autonomia equivalente à Classe A.
- LoRa mesh: anunciar rota/alcance apenas com base em topologia/evidência observada; partição e expiração degradam essa afirmação.
- BLE: avistamento expressa proximidade observada com incerteza, sob consentimento, política e limitações de SO; não usar MAC/BSSID como identidade permanente.
- GPS/LTE: localização direta exige fonte, horário, incerteza e autorização do serviço; preservar identidade do dispositivo separada da identidade GodMesh.

## 5. Capability, autorização, rota e readiness

Uma observação do dispositivo informa somente a dimensão a que sua evidência pertence. `supported`, `verified` e `operational` não concedem autorização. `AccessDecision` é independente; elegibilidade de rota é contextual; readiness pertence ao serviço. Consumidores verificam todas as dimensões requeridas e não inferem sucesso global de uma delas. Para `operational`, exigir evidência e política de frescor aprovadas; sem elas, não classificar como operacional. Nenhum TTL é fixado nesta versão. A projeção `operational ⊆ verified ⊆ supported` permanece proposta documental enquanto o contrato proprietário e aprovação não forem conciliados.

## 6. Atualização e recuperação de firmware

### 6.1 Cadeia de suprimento e atualização (ADR-063)

Cada release de firmware distribuído deve ter SBOM e build reproduzível. O dispositivo verifica a assinatura do artefato e a autenticidade do canal de atualização antes de instalar; falha de verificação rejeita antes do efeito. O processo define rollback seguro por variante e procedimento para revogar chave de assinatura comprometida, sem assumir A/B, bootloader ou mecanismo universal. Artefato sem assinatura verificável não é distribuível. Autoridade/raiz, algoritmo, formato da SBOM, procedimento operacional de revogação e mecanismos de rollback por variante permanecem desconhecidos — decisão pendente.

### 6.2 Pré-condições

Uma atualização só pode iniciar quando o perfil identifica combinação compatível de modelo/revisão, bootloader e versão; artefato/versão/origem e integridade são verificáveis; autorização de atualização e confiança/assinatura seguem política aprovada; estado de energia e conectividade satisfazem critérios específicos aprovados para a variante. Até a seleção de hardware, esses critérios são OPEN e não executáveis.

### 6.2 Estados propostos

`idle` → `requested` → `compatibility_checked` → `artifact_received` → `artifact_verified` → `installing` → `rebooting` → `post_boot_check` → `confirmed`.

Estados de término/falha: `rejected`, `interrupted`, `failed`, `recovery_required`, `unknown`. Transições e nomes são proposta desta SPEC, não API existente. Cada estado deve distinguir ação aceita, etapa concluída e confirmação originada do terminal. A perda de canal pode produzir `unknown` quando não houver evidência do dispositivo, nunca sucesso por ausência de erro.

### 6.3 Invariantes e falhas

- Artefato incompatível, corrompido ou não autorizado não inicia instalação.
- Downgrade é recusado salvo política explicitamente aprovada e prova de autorização.
- Perda de energia/link durante cada etapa tem resultado definido por bootloader/modelo: retomada segura, rollback suportado, recuperação manual ou estado indeterminado. Não presumir A/B, atomicidade ou rollback automático.
- Reinício só é considerado confirmado após evidência pós-boot adequada (versão e sinal funcional definidos por variante); ACK de transporte não substitui essa evidência.
- Chave revogada, assinatura inválida e versão fora da política terminam em recusa sem efeito de instalação; tentativa e razão podem ser auditadas sem gravar segredo.
- Se não houver mecanismo remoto, documentar procedimento físico/manual, pré-requisitos e risco residual; a limitação não é ocultada como compatibilidade plena.

## 7. Energia, saúde e watchdog

Cada variante deve distinguir dado medido por sensor, estimativa, declaração do fabricante e ausência de dado, com origem e instante. Metas de autonomia, amostragem, limiar de bateria e política de degradação são OPEN, não inferidas. Função limitada por energia deve expor estado/causa conhecida; telemetria ausente não significa bateria saudável nem dispositivo desligado.

Watchdog e reação a reset são específicos do hardware e do risco da função. Registrar timeout/reset apenas se a plataforma disponibilizar evidência; heartbeat não prova serviço funcional. Política fail-safe por operação requer decisão de produto/segurança: não presumir sempre desligar nem continuar. SOS e rádio mantêm prioridade/política de seus contratos próprios.

## 8. Segurança, privacidade e retenção

Minimizar identificadores e telemetria ao necessário à operação consentida. Não enviar credenciais, material de chave ou dados de localização além do escopo autorizado. Revogação invalida novos usos conforme política; comportamento de eventos já armazenados, TTL e exclusão depende das SPECs de segurança, localização e dados. Atualização exige autoridade de confiança e recuperação contra chave comprometida, ainda OPEN com Kim e autoridade de política. Criptografia usa padrões estabelecidos e especificação especializada; nenhum algoritmo é escolhido aqui.

## 9. Critérios de aceite documental e cenários planejados

Os cenários são planos para casos futuros; sem IDs atribuídos, ambiente ou execução.

1. **Normal — evento válido:** perfil conhecido, fonte dentro do escopo e observação válida; esperado evento preserva origem, incerteza e identidade distinta; invariante: capability não produz AccessDecision. Aplicável a adaptador com rádio; execução NOT_EXECUTED.
2. **Negativo — identidade/prova ausente:** evento sem origem ou prova requerida; esperado rejeição/unknown e nenhum dado promovido a identidade GodMesh. Aplicável às integrações de dispositivos.
3. **Negativo — BLE:** beacon encontrado; esperado `proximity_sighting` com incerteza, sem coordenada GNSS atribuída ao emissor. Aplicável a smartphone/observer habilitado; outras plataformas só se houver suporte declarado.
4. **Degradado — partição/janela fechada:** evento ou downlink não alcança terminal; esperado estado pendente/expirado/unknown segundo contrato, sem alegar entrega. Classe A e mesh aplicáveis conforme topologia.
5. **Atualização normal:** combinação compatível, artefato verificável e pré-condições aprovadas; esperado etapas distintas e confirmação pós-boot do terminal.
6. **Atualização negativa:** versão incompatível, integridade inválida ou autoridade ausente; esperado recusa antes de instalar e razão sem segredo.
7. **Recuperação:** perda de energia/link em cada etapa; esperado comportamento conforme bootloader documentado, ou `recovery_required` com procedimento manual; aplicabilidade limitada aos modelos selecionados.
8. **Energia/saúde degradada:** telemetria ausente, bateria baixa ou watchdog/reset observado; esperado origem/tipo da evidência e degradação explícitos; sem inferir serviço pronto ou falha funcional além da prova disponível.

Fechamento de implementação exigirá futura SPEC aprovada, casos atribuídos, parâmetros/autoridades decididos, matriz hardware/versão, testes autorizados e evidência por plataforma. Esta SPEC não satisfaz esses gates.

## 10. Decisões OPEN

| ID | Questão e responsável | Alternativas/recomendação | Evidência/fechamento |
|---|---|---|---|
| O-017-01 | Hardware e variantes iniciais — produto/usuário; Jack documenta | Selecionar variantes verificáveis antes de declarar suporte; permitir perfis com limitações explícitas | Modelos/revisões, fabricante/bootloader e interfaces identificados |
| O-017-02 | Assinatura, raízes, revogação e downgrade — Kim; política do usuário | Política de confiança e exceção controlada versus recusar downgrade; sem decisão nesta SPEC | Fluxos positivos/negativos e recuperação revisados/aprovados |
| O-017-03 | OTA/rollback obrigatório — produto/usuário e evidência do fabricante | Exigir como seleção ou aceitar atualização manual documentada; não impor universalidade | Por variante: mecanismo disponível, limitação e risco residual aceitos |
| O-017-04 | Energia, watchdog e fail-safe — produto/usuário, Jack, operação/segurança | Limiares por perfil com evidência versus não anunciar estado mensurável; não inventar números | Metas, método, efeito em funções e responsável aprovados |
| O-017-05 | Frescor de capability — contrato proprietário/Alan, Jack | Prazo/contexto por evidência ou manter `operational` desconhecido; sem TTL arbitrário | Política de frescor e invalidadores conciliada |
| O-017-06 | Identidade, retenção e privacidade espacial — Kim, Paul, Bruce, Gregor | Retenção mínima por finalidade versus retenção operacional justificada | Origem/autoridade, escopo, revogação e eliminação coerentes com SPECs relacionadas |
| O-017-07 | Aplicabilidade por produto/plataforma — Margaret, Lea, Donny, Chet, responsáveis de produto | N/A apenas com justificativa de ausência de interface/capacidade | Matriz completa Linux server, macOS arm64/x86_64, iOS, Windows/Linux desktop, Android e web quando aplicável |
| O-017-08 | Semântica de eventos, idempotência e ACK — Gregor, Alan e responsáveis de eventos/GMP | Reusar contrato proprietário validado ou propor extensão versionada; não criar garantia aqui | Contratos de origem, repetição, expiração e confirmação cruzados |
| O-017-09 | Timeout, cancelamento e repetição limitada de atualização e de comandos ao terminal — Jack; Gregor para adaptadores; Vint para transporte | Limites, janelas de cancelamento e número de tentativas por tipo de operação não definidos nesta SPEC; sem valores inventados; excedido o limite, o resultado é `unknown` ou `failed`, nunca sucesso | Política por operação e variante, com recusa ou `unknown` quando o limite for excedido, aprovada e revisada |

| O-017-10 | Perfis de compatibilidade N/N-1 e janelas de firmware/adaptador — Gregor e Jack; autoridade de compatibilidade/usuário | Declarar por perfil versionado; sem duração ou conjunto presumido | Perfil publicado registra janela, impacto, autoridade e rollback; negativos confirmam rejeição antes do efeito |
| O-017-11 | SBOM, build reproduzível, canal, assinatura no dispositivo, rollback e revogação de chave por variante — Jack; Kim/autoridade de assinatura e fornecedor | Capacidades específicas permanecem desconhecidas — decisão pendente | Evidência por release/variante demonstra controles ADR-063 e recuperação segura; sem afirmar suporte universal |
| O-017-12 | Terminador, visibilidade e classes de ACK em áudio/telemetria de dispositivo — Jack/Gregor; Vint revisa | Declarar por perfil e destino | Perfil identifica `confidentiality=terminated`, dados vistos pelo servidor, `ack_class`, emissor, evidência e escopo |

## 11. Dependências e revisão

Dependências: SPEC-002 (identidade/autoridade), SPEC-003/010 (GMP/adaptadores), SPEC-004 (eventos/replay), SPEC-011 (localização), SPEC-012/013/014/015/016 (produto e plataformas), SPEC-018 (observabilidade/recuperação), SPEC-020 (política de segurança/privacidade), contratos LoRa/BLE e Guardian. IDs e versões dessas SPECs devem ser conferidos no catálogo de Ada; não presumir cobertura só pelo título.

Revisão independente de Vint Cerf requerida para versão/hash desta SPEC, especialmente janelas/partições, limites de protocolo e afirmações de conectividade. Revisão não significa aprovação. Após revisão, autor integra ou registra divergência em nova versão, preservando este histórico.

## Casos por decisão (ADR-056…063)

Casos planejados desta versão. Nenhum foi executado; identificador formal de teste `NOT_ASSIGNED` em todas as linhas. Onde um parâmetro está OPEN (janela N/N-1, TTL, frescor, autoridade de assinatura, terminador por variante), o esperado é **negar ou declarar desconhecido**, nunca um número ou suporte presumido. Estados citados são os da §6.2 desta SPEC; `ack_class` é o conjunto fechado da ADR-058.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S017-056-P | ADR-056 compatibilidade | Perfil de firmware/adaptador sem janela N/N-1 aprovada e variante identificada (modelo/revisão/bootloader) | Solicitar release N e release N-1 do mesmo perfil | Compatibilidade exposta como `param_not_configured` (`unknown`); atualização não inicia; nenhuma transição para `installing` e nenhuma elevação de `supported` a `verified` por ausência de erro | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-056-N | ADR-056 compatibilidade | Perfil versionado sem feature crítica comum com o artefato ofertado | Enviar artefato fora da janela declarada ou de perfil divergente | Estado `rejected` antes de `installing`, nenhuma escrita no dispositivo, razão sanitizada registrada, sem downgrade silencioso | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-056-B | ADR-056 compatibilidade | Perfil de firmware/adaptador sem janela N/N-1 publicada (O-017-10 OPEN) | Solicitar atualização com release cuja compatibilidade não é demonstrável | Compatibilidade permanece `unknown`; atualização não inicia; nenhuma elevação de `supported` a `verified` por ausência de erro | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-056-F | ADR-056 compatibilidade | Falha de instalação em curso e artefato antigo disponível no servidor de atualização | Tentar retornar a versão anterior fora da janela declarada | Recusa antes do efeito; estado `failed` com a versão anterior mantida; razão registrada, sem rebaixamento de perfil | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-057-P | ADR-057 posição/cursor | Evento normalizado de adaptador com `origin_node_id`, `tenant_id`, `stream_id`, `partition`, `authority_epoch` e `seq` conhecidos | Ingerir evento e solicitar página seguinte por cursor | Posição canônica completa gravada; cursor devolvido opaco e versionado, vinculado a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-057-N | ADR-057 posição/cursor | Evento ou cursor legado sem `partition` | Consultar próxima página com cursor sem partição ou com `authority_epoch` incomparável | Requisição rejeitada com erro explícito; cursor não é traduzido nem completado por inferência; consumidor não avança posição | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-057-B | ADR-057 posição/cursor | Dois eventos de partições distintas com mesmo `seq` | Ingerir ambos no mesmo intervalo | Dois eventos distintos preservados; nenhum tratado como duplicata por igualdade de `seq`; colisão não silenciosa | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-057-F | ADR-057 posição/cursor | Cursor emitido antes de troca de `authority_epoch` | Reutilizar o cursor após a troca de epoch | Falha fechada: cursor recusado, nenhuma posição ou página inventada, resultado da leitura `unknown` | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-058-P | ADR-058 ACK por classe | Comando de leitura enviado a adaptador com recibo autenticado da etapa de transporte | Adaptador aceita o comando e devolve recibo com `attempt_id` e horário | Resposta carrega `ack_class=transport_accepted` com emissor, evidência e escopo; classes superiores ausentes | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-058-N | ADR-058 ACK por classe | Atualização de firmware aceita por rádio/fila/bridge, sem recibo do terminal | Consultar estado do comando na UI e na API | Estado exibido permanece classe inferior; jamais `delivered_to_device` ou `acknowledged_human`; nenhuma promoção por tempo ou ausência de erro | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-058-B | ADR-058 ACK por classe | Resposta de API sem campo `ack_class` | Receber resposta de comando ao dispositivo | Resposta tratada como inválida: campo ausente é erro explícito, não presumido sucesso nem classe mínima silenciosa | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-058-F | ADR-058 ACK por classe | Comando instalado com recibo do terminal perdido após envio | Recuperar estado sem nova resposta do destino | Resultado `unknown`, sem conversão em entrega; consulta/retry idempotente conforme contrato, sem declarar instalação | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-059-P | ADR-059 grant offline | Dispositivo/adaptador com grant assinado vigente, `epoch` comparável e sem revogação conhecida, mas sem limite de frescor aprovado | Executar operação enumerada (registrar observação de localização local / check-in de sessão existente) | `grant=valid` e frescor `unknown` com `freshness_limit=param_not_configured` registrados em dimensões separadas; operação não executada e nada enfileirado; nenhuma mutação federada criada | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-059-N | ADR-059 grant offline | Grant com `epoch` anterior, estado `stale`, `unknown` ou relógio incerto | Tentar transmitir comando ao dispositivo ou registrar observação | Efeito negado com motivo explícito (`stale`/`unknown`); nada enfileirado; adaptador não converte cache ou reconexão em autoridade | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-059-N2 | ADR-059 grant offline | Grant tecnicamente válido para as operações enumeradas | Solicitar transmissão RF, mudança de identidade, criação de grant ou transcrição remota | Operação negada: grant não enumera essas ações; recusa independe de frescor e de o dispositivo estar online | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-059-B | ADR-059 grant offline | Janela de frescor não aprovada e relógio do adaptador relatado como incerto | Executar operação enumerada após reboot do dispositivo | Estado `unknown` explícito; operação não inicia nem enfileira; nenhum prazo padrão aplicado pelo firmware e nenhuma revogação do grant declarada por parâmetro ausente | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-059-F | ADR-059 grant offline | Restore/anti-rollback deixa o dispositivo com grant ainda na validade declarada | Reutilizar o grant após o restore | Negado: `epoch`/contador monotônico não comparável ou revogação desconhecida; efeito dependente bloqueado e registrado | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-060-P | ADR-060 confidencialidade | Perfil de dispositivo/bridge com terminador identificado e conteúdo visível ao servidor declarado | Consumir telemetria e áudio pela API e pela UI | Resposta e apresentação exibem `confidentiality=terminated` com o componente terminador; nenhuma alegação de E2EE | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-060-N | ADR-060 confidencialidade | Perna protegida apenas por TLS/DTLS/SRTP com dispositivo que termina a proteção | Rotular o canal como E2EE na API ou na tela | Alegação rejeitada; UI não exibe E2EE; texto restrito a proteção de perna, sem promessa de anonimato | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-060-B | ADR-060 confidencialidade | Variante sem terminador selecionado e artefato sem metadado de confidencialidade (O-017-12 OPEN) | Entregar evento/áudio derivado sem o campo | Artefato tratado como `confidentiality=terminated`; ausência nunca vira E2EE nem é suprida por cache ou versão anterior | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-060-F | ADR-060 confidencialidade | Evento E2EE de origem exportado para gravação ou ASR com consentimento | Produzir derivado (transcrição, captura) e consultá-lo | Derivado herda `confidentiality=terminated`; API rejeita alegação de E2EE sobre ele; perna terminadora fica visível | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-061-P | ADR-061 transação/recuperação | Serviço de domínio com banco próprio e `outbox` na mesma transação | Ingerir observação de dispositivo e publicar evento a jusante | Commit único inclui estado e `outbox`; publicação só após commit; `ack_class=persisted_local` no máximo, com prova de commit | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-061-N | ADR-061 transação/recuperação | Ingestão com Inbox durável em ledger separado da projeção | Consultar o resultado da ingestão | ACK máximo `persisted_local`; `materialized` nunca devolvido; projeção pendente marcada, não presumida | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-061-F | ADR-061 transação/recuperação | Queda do processo entre commit e publicação do `outbox` | Reiniciar o serviço e republicar | Evento republicado de forma idempotente; nenhum comando duplicado ao terminal, nenhuma instalação nem tentativa repetida por replay | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-061-F2 | ADR-061 transação/recuperação | Restore com Inbox em voo e autoridade `stale`/`unknown` | Retomar operação e reenviar comando ao dispositivo | Replay reconstrói apenas projeção; mutação forte bloqueada; resultado incerto permanece `unknown` até reconciliação explícita | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-061-B | ADR-061 transação/recuperação | Ingestão já commitada com `outbox` e publicação ainda pendente | Ler o resultado da ingestão antes do publish | `ack_class` máximo `persisted_local` com prova de commit; `materialized` e efeito a jusante ausentes; leitor não publica nem promove classe | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-062-P | ADR-062 retenção fail-closed | Classe de telemetria de dispositivo com política aprovada em `(finalidade, classe, cópia, jurisdição)` e prazo/gatilho não decididos | Gravar telemetria e consultar a retenção aplicada | Registro persiste com `retention_policy_ref` identificado e `retention=param_not_configured`; nenhum prazo ou gatilho é aplicado como aprovado e o descarte por cópia permanece `unknown` | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-062-N | ADR-062 retenção fail-closed | Classe de dado pessoal de dispositivo sem política aprovada (O-017-06 OPEN) | Persistir evento ou histórico de posição | Escrita falha fechada com erro explícito na API/UI; apenas o mínimo para o efeito imediato autorizado, não gravado como histórico | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-062-B | ADR-062 retenção fail-closed | Política existente com prazo desconhecido | Encerrar o ciclo de vida do registro | Prazo desconhecido não equivale a retenção indefinida: o histórico não é gravado e o estado exibido é `unknown`, não "retido" | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-062-F | ADR-062 retenção fail-closed | Hold de auditoria ativo sobre eventos de dispositivo | Solicitar liberação do hold e novo acesso comum ao evento | Liberação exige autoridade, escopo e revisão registradas; hold não reativa acesso comum; revogação declarada por destino/cópia, nunca global | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-063-P | ADR-063 supply chain/anti-rollback | Release de firmware com SBOM, build reproduzível, assinatura válida e canal de atualização autenticado; piso de `generation` conhecido | Instalar em dispositivo limpo e repetir o build | Instalação aceita com `generation` ≥ piso; build repetido produz digest idêntico ao registrado | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-063-N | ADR-063 supply chain/anti-rollback | Artefato com assinatura inválida, ou release sem SBOM/assinatura | Distribuir e instalar o artefato | Recusa antes de `artifact_received`; release declarado não distribuível; tentativa e razão auditadas sem gravar segredo | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-063-N2 | ADR-063 supply chain/anti-rollback | Dispositivo com piso de versão assinado superior ao artefato ofertado | Instalar release com `generation` inferior ao piso | Rejeição registrada pelo dispositivo; nenhuma instalação; downgrade para release vulnerável proibido | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-063-B | ADR-063 supply chain/anti-rollback | Piso de `generation` não publicado para a variante e artefatos N e N-1 assinados | Instalar o artefato mais recente e consultar o estado anti-rollback | Piso exposto como `param_not_configured`; instalação não declara `generation` aceita contra piso desconhecido e nenhum piso é presumido a partir da versão instalada | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S017-063-F | ADR-063 supply chain/anti-rollback | Chave de assinatura comprometida ou lista de revogação em estado `unknown` (O-017-11 OPEN) | Tentar atualização e, em seguida, rollback autorizado | Atualização bloqueada enquanto revogação for `unknown`; revogação aplicada no cliente; rollback só como exceção autorizada, limitada no tempo e auditada, com rebaixamento registrado do piso | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito desta SPEC. Células citam casos da tabela `Casos por decisão` ou ID novo descrito na Nota; prefixo `S017-R###` agrupa os quatro tipos. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-010 (principal documental) | S017-058-P; S017-R010-P | S017-059-N2; S017-R010-N | S017-060-B; S017-R010-B | S017-061-F; S017-R010-F | P: adaptador BLE/LoRa/LoRaWAN/GPS-LTE expõe provenance e capacidade observada. N: adaptador sem proveniência não promove `supported` a `verified`. B: dispositivo presente mas sem classe de rádio homologada → `unknown`, não `unavailable`. F: atualização/telemetria interrompida não declara instalação nem leitura concluída. Vínculo normativo ainda a confirmar (§1). |
| RNF-008 (evolução interoperável) | S017-056-P | S017-056-N | S017-056-B | S017-056-F | Compatibilidade N/N-1 por perfil de firmware/adaptador, sem downgrade silencioso; janela concreta OPEN (O-017-10) e por isso o caso P verifica apenas `param_not_configured`. |
| ADR-057 (posição de eventos de dispositivo) | S017-057-P | S017-057-N | S017-057-B | S017-057-F | Sustenta a posição canônica das observações de dispositivo; cursor e epoch não são redefinidos aqui. |
| ADR-060 (telemetria/áudio) | S017-060-P | S017-060-N | S017-060-B | S017-060-F | Perna terminadora declarada; ausência de metadado nunca vira E2EE. |
| ADR-062 (dado pessoal de dispositivo) | S017-062-P | S017-062-N | S017-062-B | S017-062-F | Sem política por classe, escrita falha fechada; prazo OPEN aparece como `param_not_configured`. |
| ADR-063 (firmware assinado) | S017-063-P | S017-063-N; S017-063-N2 | S017-063-B | S017-063-F | SBOM, build reproduzível, canal autenticado e anti-rollback; piso é decisão assinada, não parâmetro desta SPEC. |
