# SPEC-010 — Integrações e bridges

**Versão:** DRAFT-0.7
**Estado:** DRAFT / OPEN — não aprovada, não implementada e não validada.
**Autor documental:** Gregor Hohpe  
**Revisor independente designado:** Erich Gamma (revisão pendente desta versão/hash).
**Data:** 2026-11-17  
**Escopo:** UDNL, descoberta e adaptação de dados externos, bridges opcionais e seus limites de tradução.

## Retenção por classe em bridges (ADR-065)

Payload de bridge e telemetria seguem a classe declarada na ADR-065 (telemetria 30 dias brutos/13 meses agregados; conteúdo de mensagem 24 meses; áudio não retido por padrão). Bridge que termina proteção marca `confidentiality=terminated` e a classe de retenção do destino. Cópia sem classe ou jurisdição declarada falha fechada.

## Mudanças desta versão

- DRAFT-0.5 — casos por requisito e casos P desbloqueados (C3-1): nova subseção `### Casos por requisito (P/N/B/F)` ao final, com uma linha por requisito rastreado (RF-007, RF-008, RF-010, RF-017, RF-018, RF-021) e as quatro colunas preenchidas ou `n/a` justificado; casos P cujo esperado dependia de valor não decidido — janela N/N-1 dos perfis de bridge SDK/adaptador e prazo/gatilho da política de retenção de payload normalizado — reescritos para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID; caso de borda `C010-061-B` (ADR-061) acrescentado. Nenhum número, janela, versão ou prazo é criado; nada é aprovado ou executado.
- DRAFT-0.4 — casos por decisão (resposta ao veto C3-1): nova seção “Casos por decisão (ADR-056…063)” com uma linha por decisão aplicável, ao menos um caso negativo e um de falha/recuperação por decisão, esperado observável e todo caso `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`; parâmetro `OPEN` resulta em negar/informar desconhecido. Nada é aprovado, alterado em decisão ou executado.
- ADR-060: todas as pernas com proteção terminada declaram `confidentiality=terminated`; áudio/telemetria identificam terminador e visibilidade do servidor; TLS/DTLS/SRTP não são E2EE.
- ADR-056: bridge SDK e perfis/adaptadores seguem N/N-1 em janela declarada; rejeição antes do efeito, sem downgrade silencioso; exceção exige perfil publicado com autoridade registrada; schema persistido segue evolução Protobuf.
- ADR-058: `ack_class` obrigatório com seis classes; ACK da bridge/adaptador não promove `delivered_to_device` ou `acknowledged_human`.
- Estado documental: DRAFT-0.3 / OPEN; revisão independente do delta pendente.

## 1. Requisitos rastreados

| Requisito | Origem e situação | Cobertura nesta versão |
|---|---|---|
| RF-007 | `docs/00-produto/requisitos.md:13`; Mumble como bridge externa opcional, APROVADO | Parcial: contrato comum de bridge, sem homologação Mumble |
| RF-008 | `docs/00-produto/requisitos.md:14`; FRN opcional, isolada e autorizada, APROVADO | Parcial: política TX/RX, sem protocolo ou servidor homologado |
| RF-010 | `docs/00-produto/requisitos.md:16`; adaptadores LoRaWAN/mesh, BLE e GPS/LTE, APROVADO | Parcial: ingestão e limitações descritas; perfis concretos de hardware/origem faltam |
| RF-017 | `docs/00-produto/requisitos.md:23`; descoberta autorizada de serviços externos locais, ESPECIFICAR | Parcial: descoberta não conecta nem autoriza; alvo e interface ainda OPEN |
| RF-018 | `docs/00-produto/requisitos.md:24`; SDK de bridges/adaptadores isolado, APROVADO | Parcial: fronteira funcional, privilégios mínimos e falha isolada descritos; IPC/schema não escolhidos |
| RF-021 | `docs/00-produto/requisitos.md:27`; habilitação sem acesso não autorizado a RF/localização, APROVADO | Dependência de segurança. O contrato de acesso e localização pertence às SPECs de segurança/localização; esta SPEC define somente a aplicação na fronteira externa |

A situação APROVADO ou ESPECIFICAR acima reproduz a coluna de status do requisito. Não equivale a aprovação desta SPEC ou prova de implementação. Casos de aceite abaixo são planejados, não executados. IDs centrais de SPEC e testes permanecem NOT_ASSIGNED no índice de Ada até registro.

## 2. Fontes e procedência

Fontes normativas e de domínio consultadas:

- `docs/README.md`, regras de prevalência: arquitetura de referência e ADRs prevalecem; voz GodMesh nativa é obrigatória; terceiros são bridges opcionais; integração externa passa pela UDNL.
- `docs/01-arquitetura/normalizacao-dados.md`, ADR-037, §§1–3 e seções de identidade, proveniência e projeção: decisão arquitetural oficial, ainda não implementada.
- `docs/03-integracoes/gateway-universal.md`, missão, avaliação de capability/rota/readiness e ciclo de dispatch: candidatos, autorização e efeitos são conceitos distintos.
- `docs/03-integracoes/bridges-voz.md`, diretriz, contrato interno e segurança: bridge desligada por padrão, opcional, RX/TX separados e sem dependência do núcleo nativo.
- `docs/03-integracoes/lora-lorawan-ble.md`, §§1–5: topologias distintas, confirmação de fila não é entrega, classes LoRaWAN e descoberta BLE autorizada.
- `docs/03-integracoes/outros-adaptadores.md`, §§ GPS, perfil e conformidade: `location.observed`, candidato Traccar, proveniência, autorização e negativos; perfil geral não substitui contrato por origem.
- `docs/04-seguranca/uso-das-redes.md`, §§1–5: domínio internet-only, radioamador e business-radio; TX bloqueado sem autorizações aplicáveis.
- `docs/04-seguranca/politica-localizacao.md` e `docs/07-contratos/localizacao-privacidade.md`, ADR-041: separação proposta entre localização coarse e precisa e scopes específicos; ainda não implementados.
- `docs/06-planejamento/matriz-rastreabilidade.md:96,103–104,107` e `docs/06-planejamento/aceites-comportamentais.md`, aceites RF-010, RF-017, RF-018 e RF-021. Os critérios são propostas documentais.
- `SPECS-EM-ANDAMENTO/contribuicao-gregor-hohpe.md`, versão de entrada SHA-256 `efe252e1c2859db28de974c6dae9cddb93e72a739cb8686efe9072319b24aacd`, §§ UDNL, adaptadores, bridge e localização.
- `SPECS-EM-ANDAMENTO/contribuicao-erich-gamma.md`, §§ Revisão Fowler/Gregor e integração: descoberta separada de autorização e bridge opcional consideradas coerentes; revisão focal anotou ambiguidade potencial sobre tenant.
- `SPECS-EM-ANDAMENTO/contribuicao-lawrence-rosen.md`, § Revisão dos rascunhos pertinentes: requisito de evidenciar autorização regulatória cumulativamente com consentimento do operador; território, faixa, serviço, equipamento e fonte oficial permanecem OPEN.

Hash identifica a cópia de contribuição indicada no manifesto consultado, não todos os documentos fonte. Citações são localizadores nos arquivos atuais; nenhum arquivo de produto foi alterado nesta entrega.

## 3. Objetivo e limites

Integrar fontes e destinos externos ao domínio GodMesh por contratos explícitos, sem torná-los dependências do núcleo. A UDNL recebe e valida dados na fronteira, preserva origem, escopo, evidência e incerteza, normaliza a semântica e solicita autorização e rota. Para saída, produz projeção compatível e relata perdas. Não garante equivalência entre protocolos.

Plano conceitual de processamento: adaptador de origem → validação da fronteira → normalização → decisão de autorização → seleção de rota → projeção autorizada → destino. A ordem é uma descrição documental, não arquitetura de processos aprovada. Mídia de tempo real usa plano separado; áudio não é convertido em evento persistente por frame.

A SPEC não escolhe bibliotecas, SDKs, protocolo de transporte, schema wire, endpoints, países, hardware, conectores ou termos externos. Mumble, FRN, Traccar, ChirpStack, MQTT, LoRa, BLE e GPS/LTE são contextos/candidatos presentes nas fontes, não aprovações de integração.

## 4. Atores e autoridade

| Ator | Responsabilidade proposta | Limite |
|---|---|---|
| Principal GodMesh autenticado | Solicitar descoberta, leitura, publicação ou operação conforme grants | Não escolhe ou concede tenant por campo não confiável do request |
| Serviço de identidade/política | Derivar principal e tenant autenticados e avaliar escopos e política | Regras de grant e revogação dependem de SPEC-002/020 e decisão de política |
| Adaptador | Validar protocolo externo na fronteira, declarar capacidades e preservar evidências | Não concede identidade GodMesh, licença, permissão ou capability por declarar suporte |
| Gateway/roteador | Avaliar capability, autorização, rota e readiness separadamente | Uma conexão observada não prova autorização ou operação pronta |
| Operador/titular da rede externa | Conceder somente autorização sob sua autoridade | Consentimento não substitui licença ou autorização regulatória |
| Autoridade regulatória/condição normativa | Determinar autorização exigida para jurisdição e uso quando aplicável | País, faixa, serviço, equipamento e fonte vigente permanecem OPEN |
| Revisor independente | Revisar esta versão identificada por hash | Parecer documental não aprova política nem comprova execução |

O tenant é derivado pelo servidor a partir da sessão/principal autenticado. Uma operação pode solicitar escopo permitido, mas o request não o concede. Este texto adota a ressalva de fronteira identificada por Erich no parecer de Fowler/Gregor. Esquemas compatíveis com Graydon e Kim permanecem dependência.

## 5. Contratos e dados

### 5.1 Entrada e identidade

- A origem externa é identificada por tipo e instância de rede e pelo identificador nativo cujo escopo e estabilidade o adaptador declara.
- O adaptador preserva o valor original e o método/fonte de observação. Não associa automaticamente nickname, indicativo aparente, IMEI, DevEUI ou identificador de beacon a `gmi_id` ou GMC.
- O vínculo a identidade GodMesh exige `PrincipalBinding`, prova de controle e autorização segundo contratos de identidade. O adaptador não declara essa prova em nome do serviço de identidade.
- Payload, versão, formato, dimensão, integridade e campos obrigatórios são validados na fronteira antes de normalização. O schema e os limites quantitativos permanecem OPEN.

### 5.2 Proveniência e incerteza

- Eventos normalizados mantêm origem atribuível, identificador original, horário observado, classe de evidência, escopo e qualidade realmente fornecidos.
- Campo ausente ou não confiável permanece unknown/indisponível. Não inferir localização, horário, identidade, continuação de cadeia ou entrega.
- Identificador estável e chave de deduplicação são propriedades do protocolo/adaptador declaradas com evidência. Sem chave estável, não prometer deduplicação entre reconexões ou exactly-once.
- Se a rede de origem descarta `visited_bridge_ids` ou metadado equivalente, continuidade fica unknown e projeção automática encadeada é bloqueada conforme política. Não inventar campo substituto.

### 5.3 Capability, acesso e rota

- `CapabilitySet` descreve evidências de capacidade; `AccessDecision` representa autorização; `ServiceRoute` descreve rota candidata/selecionada. Seus estados não são intercambiáveis.
- A declaração `supported` não prova `verified` ou `operational`. Disponibilidade técnica não concede acesso. A rota só pode ser anunciada elegível quando os gates definidos nos contratos respectivos forem satisfeitos e a observação estiver suficientemente fresca. Limiar/idade requer decisão dos donos, sem valor inventado nesta SPEC.
- Toda operação verifica o principal/tenant autenticados e escopo permitido. TX precisa de autorização específica do produto e das condições externas aplicáveis.

### 5.4 Projeção e perda

- A projeção de saída lista quais campos foram representados, transformados ou descartados e por quê, por meio de relatório conceitual `ProjectionReport` já previsto pela UDNL.
- A projeção não aumenta audiência, privilégio, precisão ou confiança. Destino incapaz de preservar um atributo obrigatório causa recusa ou resultado degradado explícito, de acordo com política ainda OPEN.
- Schema final, taxonomia de perdas, severidade e retenção do relatório aguardam Graydon, Erich e responsáveis de política.

### 5.5 Mídia e bridge

- Controle PTT, identidade e sessão têm contrato distinto do fluxo de mídia. Frames não percorrem barramento genérico de eventos persistentes.
- Bridge permanece desabilitada por padrão. RX não implica TX. TX exige grant GodMesh atual, concessão de floor/PTT e direitos/autorizações externos efetivamente aplicáveis.
- Antes de TX, verificar separadamente: autorização GodMesh, anuência do titular/operador da infraestrutura, e licença/autorização regulatória quando exigida. Não deixar uma condição substituir outra. Quando não for possível determinar aplicabilidade regulatória, bloquear TX e manter OPEN.
- Revogação durante TX interrompe a transmissão segundo contratos PTT/lease/fencing. Formato de controle e tempo máximo de TX não estão definidos aqui.
- Cada perna em que bridge/adaptador termina proteção expõe `confidentiality=terminated`; TLS/DTLS/SRTP não é E2EE. O perfil de áudio/telemetria identifica o terminador e os campos/conteúdo que o servidor consegue ver. Se isso não estiver definido para um perfil, visibilidade e terminador são desconhecidos — decisão pendente.

### 5.6 Compatibilidade e ACK (ADR-056, ADR-058)

Cada perfil versionado de bridge SDK e adaptador declara a janela de suporte N/N-1. A duração numérica da janela e os perfis publicados permanecem desconhecidos — decisão pendente. Versão ou feature crítica incompatível é rejeitada antes de qualquer efeito; downgrade silencioso é proibido. Exceção somente por perfil publicado que registre janela, impacto, autoridade que autorizou e plano de rollback. Para schema persistido em Protobuf, número de campo não é reutilizado nem alterado, campos desconhecidos são preservados e campos removidos entram em `reserved`.

Toda resposta carrega `ack_class` obrigatório: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`, com emissor, evidência e escopo. ACK de bridge/adaptador nunca promove a `delivered_to_device` ou `acknowledged_human`; não há promoção por tempo, camada inferior ou ausência de erro.

## 6. Estados e transições

Estados lógicos abaixo organizam o aceite; não são enums wire ratificados. Estado ausente ou contraditório não é sucesso.

| Estado lógico | Condição | Transição permitida | Efeito proibido |
|---|---|---|---|
| `disabled` | Integração não habilitada explicitamente | Configuração autorizada pode solicitar descoberta | Conectar ou transmitir |
| `discovered` | Candidato conhecido por método permitido | Solicitar avaliação de identidade, capacidade e acesso | Criar sessão, ler payload privado, anunciar rota elegível ou TX |
| `unverified` | Identidade, origem, frescor ou capability sem evidência suficiente | Obter evidência adicional por caminho autorizado | Promover identidade/capability por inferência |
| `denied` | Política/grant/condição externa recusa operação | Revisar configuração/autoridade, depois reavaliar | Fazer fallback para canal, tenant ou rede menos restrita |
| `authorized` | Operação e escopo autorizados para contexto atual | Conectar ou executar a operação permitida | Assumir que outra operação/direção também está liberada |
| `connected_rx` | Sessão autorizada com recepção disponível | Receber conteúdo conforme consentimento e contrato | TX sem concessões e autorizações próprias |
| `connected_tx` | Todos os gates TX confirmados no contexto | Enviar durante a concessão válida; parar ao revogar/expirar | Persistir TX após revogação/fencing inválido |
| `degraded` | Dependência/capability reduzida ou incerta | Reavaliar ao recuperar evidência/dependência | Alegar entrega/capacidade não comprovada |
| `disconnected` | Sessão encerrada ou dependência perdida | Descoberta/reconexão sob nova validação | Reutilizar credencial/grant vencido ou assumir estado anterior |

Transições para operação podem ser bloqueadas por decisão unknown. A semântica de autorização durante partição, validade de grants e revalidação após reconexão é dependência de SPEC-002/003/004/020.

## 7. Cenários planejados e resultados esperados

Os casos são roteiros documentais para futura SPEC/teste. Não foram executados. IDs são locais a este draft até registro no índice.

| Caso | Pré-condição e estímulo | Resultado esperado | Requisito |
|---|---|---|---|
| C010-01 | Descoberta encontra endpoint sem autorização de conexão; consultar descoberta | Retorna candidato, origem e estado; não abre sessão nem anuncia rota/TX | RF-017 |
| C010-02 | Adaptador recebe payload inválido ou versão desconhecida | Rejeita antes da normalização; resposta não expõe segredo nem altera estado canônico | RF-018 |
| C010-03 | Origem apresenta IMEI/nickname/DevEUI sem prova de binding | Preserva ID externo e sua evidência; não cria `gmi_id`/GMC | RF-010 |
| C010-04 | Origem remove cadeia de bridges necessária | Marca continuidade unknown e bloqueia encaminhamento externo encadeado | RF-010/018 |
| C010-05 | GPS/LTE envia observação atrasada ou sem qualidade/fonte suficiente | Marca frescor/qualidade unknown ou stale; não inventa coordenada/precisão nem publica fora da política | RF-010, dependência RF-021 |
| C010-06 | BLE detecta tag sem autorização aplicável ou apenas proximidade | Bloqueia coleta não autorizada; quando permitida, registra sighting de proximidade, nunca GNSS do beacon | RF-010/021 |
| C010-07 | LoRaWAN Classe A recebe solicitação fora da janela após uplink | Não afirma entrega imediata; relata estado pendente/indisponível conforme contrato | RF-010 |
| C010-08 | Broker confirma fila downlink, dispositivo ainda não confirmou | Registra somente aceitação pela fila; não marca entrega funcional | RF-010/011 |
| C010-09 | Capability está `supported` mas falta evidência `verified`/operational ou freshness | Não anuncia rota elegível; pede nova evidência ou indica unavailable/unknown | RF-017/018 |
| C010-10 | Principal autenticado não tem membership, escopo ou autorização atual para o tenant-alvo solicitado | Servidor rejeita antes de consultar ou alterar recurso; não revela existência nem dados do tenant-alvo; registra motivo sanitizado | RF-018 |
| C010-16 | Principal solicita projeção autorizada para destino compatível, mas um campo opcional não é representável | Projeção permitida somente se política/taxonomia aprovada admitir a perda; `ProjectionReport` registra campo, representação/transformação/descarte e motivo; resultado não afirma equivalência | RF-018 |
| C010-17 | Principal tem vínculos válidos com mais de um tenant e seleciona um contexto autorizado para a operação e recurso pedidos | Servidor valida seletor contra sessão, membership, autorização e recurso; opera apenas no contexto autorizado escolhido; sem vazamento entre contextos | RF-018 |
| C010-11 | Bridge está conectada para RX; usuário solicita TX | TX segue negado até obter grants/floor e cada autorização externa/regulatória aplicável | RF-007/008/018/021 |
| C010-12 | Anuência do operador existe, mas aplicabilidade/licença regulatória do país/uso é unknown | TX permanece bloqueado; condição e evidência pendem de análise oficial competente | RF-008/021 |
| C010-13 | Revogação ocorre durante PTT ou lease/fence perde validade | TX é interrompido/recusado antes de novos frames; auditoria não inclui conteúdo | RF-007/008/018 |
| C010-14 | Bridge forma caminho A→B→A, ou serviço externo cai | Bloqueia retorno; serviço GodMesh nativo segue independente; registra degradação | RF-007/008/018 |
| C010-15 | Destino não representa campo obrigatório para a semântica ou política aprovada | Bloqueia projeção antes do efeito externo; informa motivo sanitizado e perda não permitida; não trata como equivalência ou sucesso | RF-018 |

## 8. Segurança, privacidade e auditoria

1. Aplicar privilégio mínimo no Bridge Host e no adaptador; segredo não entra em catálogo, evento, log ou relatório de projeção.
2. Identificar principal, tenant, operação, audiência e finalidade a partir de autoridade confiável; não permitir que payload externo altere ACL.
3. Localização pública/canal segue política coarse; posição precisa de SOS usa canal/escopo de incidente restrito e nunca é projetada para bridge por padrão. Regras detalhadas pertencem a ADR-041/SPEC-011/SPEC-020.
4. Registrar evento administrativo mínimo para descoberta, autorização, conexão, revogação, TX e falha. Payload e conteúdo de mídia não são gravados por padrão. Campos, acesso e retenção de audit log permanecem OPEN.
5. TX externa é negada por padrão e falha fechada na ausência de qualquer evidência de autorização aplicável. Requisito de região e conformidade sem ID atribuído: precisa de fontes oficiais atuais por jurisdição; revisão legal não é certificação de produto.
6. Não afirmar E2EE, anonimato, entrega, alcance, identidade ou autorização acima da evidência observada.

## 9. Aplicabilidade de plataformas

| Alvo | Aplicabilidade planejada | OPEN / validação futura |
|---|---|---|
| Servidor Linux em contêiner | Gateway, adaptadores e Bridge Host conforme perfil | Isolamento, IPC, permissões de rede, imagem e requisitos operacionais dependem de SPEC-018/019 e decisão de stack |
| macOS arm64/x86_64 | Cliente nativo e possíveis adaptadores que a plataforma exponha | APIs e entitlements, compatibilidade, lifecycle e suporte real não verificados |
| iOS | Cliente SwiftUI, permissões de localização/Bluetooth e acionamento de serviços | Background, permissões e APIs compatíveis dependem de SPEC-016 e decisão de produto |
| Windows desktop | Cliente e integração de serviço quando houver interface aprovada | Stack e APIs de sensores/radio não selecionadas; sem pressupor acesso direto |
| Linux desktop | Cliente/gateway conforme interfaces autorizadas | Distribuição, permissões e suporte ao dispositivo dependem de stack/versão |
| Android | Cliente nativo, sensores e conectividade permitida pelo SO | APIs, permissões e execução em background dependem de SPEC-015 e validação futura |
| Web | Interface de controle/estado e mídia quando elegível | Web não implica acesso a rádio/sensor arbitrário; APIs disponíveis e permissões devem ser avaliadas por navegador |

Nenhuma plataforma ganha acesso irrestrito a hardware ou RF. Plataforma não aplicável a um caminho específico deve ser justificada no contrato daquele conector, sem excluir a plataforma do produto.

## 10. Decisões e dependências OPEN

| Questão | Dono/interface | Alternativas ou recomendação | Fechar quando |
|---|---|---|---|
| Janela N/N-1 e perfis publicados de bridge SDK/adaptador | Gregor; autoridade de compatibilidade/usuário; Erich revisa | Duração por perfil permanece desconhecida — decisão pendente; exceção só perfil publicado | Perfil versionado declara janela, impacto, autoridade e plano de rollback; rejeição precede efeito |
| Terminador, visibilidade do servidor e forma/evidência de ACK por conector | Gregor + responsáveis de bridge/áudio/telemetria; Erich/Radia revisam | Especificar por perfil de conector, sem generalizar | `confidentiality=terminated` e `ack_class` observável na API/UI com emissor, evidência e escopo |
| Schema GMP/API para identity evidence, tenant e erros | Graydon, Kim, Erich | Reutilizar tipos canônicos reconciliados; não publicar schema wire antes de ratificação | Schemas e fonte de autoridade compatíveis, com negativos tenant-crossing |
| Contrato completo de `ProjectionReport` e política de perda | Gregor/Erich/Graydon | Relatório por projeção com campo, transformação, motivo e severidade proposta; taxonomia OPEN | consumidor trata perdas sem inventar equivalência e contratos aprovados |
| Gateway, IPC e isolamento de processos | Fowler, Graydon, Radia | IPC autenticado ou outro mecanismo a decidir por stack; sem criar serviço por presunção | fronteira, autenticação, recursos e falhas documentados por implantação |
| Conectores e versões GPS/LTE, LoRa, BLE, Mumble, FRN | Gregor; Jack; Radia; Vint | Selecionar fontes por necessidade e autorização, não por menção no documento | protocolo/versão, direitos, evidência de origem, compatibilidade e homologação documentais definidos |
| Frescor, retry, spool, retenção e deduplicação | Gregor; Paul; Michael; Kleppmann | Usar capacidades demonstradas pela origem; não prometer exactly-once | valores/políticas aprovados por finalidade e casos de crash/reconexão completos |
| RX/TX, direitos RF e recorte territorial | Bruce/Kim; Lawrence; Radia; Alan | RX-only até comprovação; autorizações cumulativas para TX, conforme aplicável | país/faixa/serviço/equipamento/operador identificados e fonte oficial vigente revisada |
| Offline, grants e revogação | Kim/Bruce; Graydon; donos cliente | Negar sensível sem prova; grants assinados limitados são alternativa ainda não aprovada | política de validade, revogação e recuperação aprovada com cenários por classe |
| Localização e privacidade espacial | Paul/Bruce; Gregor como interface de projeção | Separar coarse de precise; retenção e região por finalidade OPEN | contrato e política compatíveis, decisão por oferta/região documentada |
| Custo, carga, frescor e disponibilidade | Margaret/Grady/Brendan | Propor envelopes após cenários; não inventar meta nem registrar medição inexistente | decisão do usuário e critérios de observabilidade atribuídos |

A oferta hospedada e autogerida e a configuração de regiões estão aprovadas pelo usuário no escopo registrado pela coordenação. Esta SPEC não amplia essa aprovação para fonte de localização, países, residência, retenção, prazo ou permissão.

## 11. Critérios de aceite documental

Esta DRAFT-0.3 só pode avançar para revisão de completude quando:

- Cada requisito listado tem fluxo, ator, permissão, contrato de entrada/saída e estado positivo, negativo e degradado.
- Tenant e principal derivam da sessão autenticada e autorização; não de campos declarativos do adaptador/request.
- Ingestão e projeção preservam identidade original, proveniência, incerteza e perdas; campos unknown não são promovidos.
- Capability, authorization, rota e readiness não colapsam; descoberta não cria sessão, rota ou TX.
- Mídia não usa evento persistente genérico; bridge não interrompe serviço GodMesh nativo.
- RX e TX são separados; TX exige provas independentes e cumulativas de cada autorização aplicável. Aplicabilidade desconhecida bloqueia TX.
- Localização coarse e precisa não se misturam nem escapam por bridge, erro, auditoria ou timeline comum; política detalhada é aprovada pelas SPECs proprietárias.
- B02 mantém OPEN em conectores, fonte, frescor, retry, retenção, revogação offline e jurisdição até decisão e evidência rastreáveis.
- Casos C010-01 a C010-17 são revisados contra o índice e requisitos; casos não executados continuam NOT_EXECUTED. C010-10 cobre escopo não autorizado; C010-17 cobre seleção legítima de contexto multi-tenant. C010-15 bloqueia campo obrigatório não representável; C010-16 verifica relatório de perda opcional permitida.
- Erich reavalia o arquivo após identificar versão/hash; Graydon, Kim, Radia, Jack, Paul e Bruce resolvem dependências de domínio. Discordâncias ficam explícitas.

## 12. Estado, cobertura e limites

**Resultado:** autoria documental concluída como DRAFT-0.1 para revisão. Cobertura parcial em RF-007, RF-008, RF-010, RF-017 e RF-018; RF-021 é somente dependência de fronteira. Isso não fecha requisitos nem o planejamento global.

**Ainda OPEN:** contratos de wire/API, taxonomia de perdas, conectores homologados, parâmetros de frescor/retention/retry, autorização offline/revogação, RF/jurisdição, evidência por plataforma, custo/carga e política final de localização.

**Não realizado:** nenhuma SPEC aprovada, implementação, build, teste, operação de serviço, validação de hardware, exame de lei por jurisdição ou certificação de segurança.

## 13. Revisão independente desta versão

Erich Gamma revisou integralmente DRAFT-0.1, SHA-256 `39520deace9ca5969d644101cc239a235793c6c27a9061cf175ae53e0b65ccaa`, conforme parecer `SPECS-EM-ANDAMENTO/contribuicao-erich-gamma.md`, seção “Revisão SPEC-010 DRAFT-0.1”. O parecer encontrou dois pontos P2: C010-10 não distinguia escopo negado de seleção autorizada multi-tenant; os casos não verificavam um `ProjectionReport` para perda opcional permitida nem o bloqueio de campo obrigatório.

**Resposta do autor nesta DRAFT-0.2:** C010-10 agora exige ausência de membership/escopo/autorização para o tenant-alvo; C010-17 cobre separadamente seleção autorizada multi-tenant. C010-16 verifica perda opcional e campos/motivos registrados; C010-15 bloqueia atributo obrigatório não representável. A taxonomia, severidade e decisão sobre quais perdas são permitidas continuam OPEN; nenhum resultado é presumido.

**Revisão do delta DRAFT-0.2:** pendente de Erich. Até nova conferência por hash, os dois achados foram respondidos no texto, não marcados como fechados pelo revisor.

## Casos por decisão (ADR-056…063)

Casos planejados, sem ID formal: `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED` em todas as linhas. `Tipo`: **P** positivo, **N** negativo, **B** borda, **F** falha/recuperação. Cada decisão aplicável tem ao menos um caso N e um F. Onde o parâmetro é `OPEN`, o esperado é negar ou informar desconhecido — nunca número, prazo, janela, versão ou código inventado. Estes casos complementam §7 e não o substituem.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| C010-056-P | ADR-056 | Bridge SDK e adaptador com perfis N e N-1; **janela N/N-1 não declarada no perfil** (`OPEN` — §10) | Estabelecer handshake e abrir sessão RX | Esperado reformulado com parâmetro ausente: handshake recusado com `param_not_configured`; nenhuma sessão aberta e perfil exibido como não suportado sem downgrade silencioso. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-056-N | ADR-056 | Adaptador sem a feature crítica exigida pelo perfil da bridge | Abrir sessão | Rejeição antes de qualquer efeito: nenhuma sessão, nenhum evento normalizado, nenhuma chamada ao serviço externo; sem downgrade silencioso | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-056-B | ADR-056 | Payload persistido com campo removido e campo desconhecido novo | Reler e regravar o registro | Campo removido em `reserved`, nunca reciclado; campo desconhecido preservado; nenhum número de campo alterado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-056-F | ADR-056 | Perfil de bridge sem janela N/N-1 declarada (`OPEN` — §10) | Conectar versões distintas | Conexão recusada com indicação de perfil não suportado; estado `unverified`; nenhuma exceção sem autoridade registrada | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-057-P | ADR-057 | Evento normalizado em partição conhecida e `authority_epoch` vigente | Consultar a projeção com cursor versionado da tupla completa | Eventos daquela partição/epoch devolvidos; cursor com versão declarada e vínculo a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-057-N | ADR-057 | Evento/cursor de entrada sem `partition` | Normalizar e projetar o evento | Evento rejeitado com erro explícito; nenhuma projeção criada; nenhuma tradução de cursor para outra forma | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-057-B | ADR-057 | Duas partições com a mesma `seq`; mesma chave de deduplicação declarada pelo adaptador | Ingerir ambos | Eventos distintos preservados; dedupe aplicado somente pela chave declarada com evidência; nenhum colapso por `seq` repetida | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-057-F | ADR-057 | Replay de cursor após troca de `authority_epoch` ou após queda do adaptador | Reapresentar o cursor | Rejeição fechada; continuidade marcada `unknown`; nenhum evento antigo projetado como atual; encadeamento automático bloqueado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-058-P | ADR-058 | Publicação entregue à fila/broker da bridge | Consolidar a etapa | Resposta com `ack_class=transport_accepted` e recibo daquele salto; estado do evento permanece pré-entrega | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-058-N | ADR-058 | Broker confirmou fila downlink; dispositivo externo não confirmou | Exigir `delivered_to_device` | UI/API não promovem; apenas aceitação pela fila é registrada; etapa de entrega permanece aberta e nomeada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-058-B | ADR-058 | Resposta de adaptador sem `ack_class` | Consumir a resposta | Resposta inválida; estado `degraded`/`unknown`; nenhuma confirmação exibida | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-058-F | ADR-058 | Recibo perdido após envio ao serviço externo | Reconsultar o recibo antes de novo envio | Resultado `unknown`; nenhum efeito externo repetido; consulta do recibo precede nova tentativa; nenhuma classe promovida por tempo | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-059-P | ADR-059 | Sessão RX autorizada com grant vigente e comparável | Receber conteúdo de bridge | Recepção autorizada; `connected_rx`; nenhum TX emitido e nenhum grant de TX criado implicitamente | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-059-N | ADR-059 | Estado de autorização `stale`/`unknown` duranto isolamento do adaptador | Solicitar TX | TX negado antes de qualquer frame; estado não convertido em sucesso; motivo registrado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-059-N2 | ADR-059 | Pedido de transmissão RF usando grant offline existente | Transmitir | Negado — transmissão RF está entre as ações sempre vedadas ao grant offline; nenhum frame externo emitido | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-059-F | ADR-059 | Reconexão após partição com revogação conhecida | Retomar TX | TX permanece negado até revalidação de autoridade e audiência; nenhum efeito duplicado; estado `denied`/`unverified` exibido | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-059-B | ADR-059 | Grant com `epoch` anterior ao observado na fronteira externa | Executar operação condicional | Operação negada por epoch incomparável; nenhuma mutação federada; motivo registrado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-060-P | ADR-060 | Perfil de bridge de dados sem áudio nem conteúdo terminado | Consultar estado da integração | Campo de confidencialidade presente e fiel ao perfil; nenhuma terminação declarada indevidamente; nenhuma alegação de E2EE | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-060-N | ADR-060 | Bridge de voz com transcodificação ativa | Ler estado da sessão e da bridge | `confidentiality=terminated` na API e na UI, com terminador e campos visíveis ao servidor; proibido alegar E2EE entre redes | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-060-N2 | ADR-060 | ASR em servidor consumindo áudio da bridge, com consentimento por canal | Publicar derivado de transcrição | Derivado herda `terminated`; texto não promete E2EE ou anonimato; consentimento registrado por canal | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-060-B | ADR-060 | Perfil de conector sem terminador/visibilidade definidos (`OPEN` em §5.5) | Consultar o perfil | Terminador e visibilidade declarados desconhecidos; artefato tratado como `terminated`; nenhuma suposição de proteção | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-060-F | ADR-060 | Metadado de confidencialidade ausente em payload de bridge ou em `ProjectionReport` | Consumir o payload e o relatório | Ambos tratados como `terminated` (falha fechada); ausência declarada; nenhuma herança de proteção por cache | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-061-P | ADR-061 | Gateway/adaptador com transação única de domínio e `outbox` | Ingerir uma leitura autorizada | Evento normalizado persistido e `outbox` na mesma transação; publicação após commit; `event_id` único | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-061-N | ADR-061 | Outbox com evento destinado a bridge/rádio pendente | Executar replay do outbox | Projeção reconstruída sem reemitir chamada à bridge, ao rádio, a pagamento ou a notificação; nenhum efeito externo repetido | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-061-N2 | ADR-061 | Ingestão assíncrona em Inbox/ledger separado da projeção | Consolidar a recepção do payload | ACK máximo `persisted_local`; `materialized` não emitido; UI não exibe evento aplicado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-061-B | ADR-061 | Duplicata do mesmo `event_id` no outbox de ingestão ou de envio à bridge | Executar replay do outbox | Um único evento normalizado e um único envio externo; duplicata descartada pela chave idempotente e registrada; nenhum comando repetido ao rádio, à bridge ou à notificação | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-061-F | ADR-061 | Crash entre commit e publicação, ou queda do serviço externo durante o envio | Reiniciar e reconciliar | Evento publicado uma única vez; estado `degraded` até nova evidência; autoridade `stale`/`unknown` bloqueia mutação forte; nenhum TX persistido após revogação | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-062-P | ADR-062 | Política com finalidade, classe e cópia declaradas para payload normalizado e `ProjectionReport`, mas **prazo e gatilho da ADR-065s** (`OPEN`) | Persistir evento e `ProjectionReport` | Esperado reformulado com parâmetro ausente: objetos gravados com a política identificada e `retention_policy_ref` presente; prazo e gatilho reportados como `param_not_configured`, sem expiração automática anunciada; conteúdo de mídia ausente por padrão | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-062-N | ADR-062 | Nenhuma política para a classe de payload/dead letter daquela cópia | Persistir payload externo ou dead letter | Persistência negada com erro explícito; nenhum conteúdo em log, catálogo ou relatório de projeção; motivo informado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-062-B | ADR-062 | `ProjectionReport` e trilha de auditoria com prazos distintos | Expurgar o relatório | Expurgo aplicado somente à classe/cópia autorizada; auditoria mantém prazo próprio; nenhuma disposição global declarada | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-062-F | ADR-062 | Descarte de cópia réplica/backup sem confirmação | Consultar e restaurar a cópia | Cópia permanece `unknown`/`pending`; restauração não devolve conteúdo antes da reconciliação; hold exige autoridade, escopo e revisão | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-063-P | ADR-063 | Artefato de bridge/adaptador com SBOM por release, build reproduzível, assinatura válida e `generation` igual ou superior ao piso | Instalar e carregar no Bridge Host | Carregamento aceito após verificação; digest comparado; `generation` registrada; nenhuma capability declarada como `verified` só por instalar | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-063-N | ADR-063 | Release nova de dependência sem versão resolvida, fonte, data, licença, análise de vulnerabilidade e digest registrados | Publicar o release | Release não distribuível: pipeline recusa a publicação e registra o campo faltante; nenhum artefato entra em canal de atualização | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-063-N2 | ADR-063 | Adaptador com `generation` inferior ao piso assinado | Instalar sobre o piso atual | Rejeição por `generation` inferior ao piso; artefato não carregado; nenhuma sessão de bridge aberta | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-063-F | ADR-063 | Chave de assinatura comprometida e lista de revogação publicada | Carregar artefato assinado pela chave revogada | Lista verificada no Bridge Host e artefato rejeitado; carregamento bloqueado até reemissão; evento auditável registrado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C010-063-B | ADR-063 | Rollback autorizado de adaptador como exceção limitada no tempo, com piso rebaixado por decisão registrada | Instalar o release anterior | Instalação aceita e auditada com autoridade e janela; sem a decisão, o mesmo artefato permanece rejeitado | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |

### Casos por requisito (P/N/B/F)

Uma linha por requisito rastreado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §7 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-007 | C010-059-P | C010-059-N, C010-060-N | C010-060-B | C010-061-F | Mumble permanece candidato, não homologado; bridge desligada não afeta o núcleo nativo |
| RF-008 | C010-058-P | C010-059-N2, C010-059-F | C010-059-B | C010-061-N | TX exige autorização GodMesh, anuência do operador e licença aplicável, cumulativas |
| RF-010 | C010-057-B, C010-061-P | C010-RF010-N1 (novo) | C010-RF010-B1 (novo) | C010-057-F | Perfis concretos de hardware/origem faltam; confirmação de fila não é entrega |
| RF-017 | C010-01 | C010-10, C010-RF017-N1 (novo) | C010-056-B | C010-057-F | Descoberta não cria sessão, rota elegível ou TX |
| RF-018 | C010-056-P (reformulado) | C010-056-N, C010-061-N | C010-056-B | C010-056-F, C010-058-B | Isolamento de privilégios; IPC/schema não escolhidos |
| RF-021 | C010-060-P | C010-12, C010-RF021-N1 (novo) | C010-RF021-B1 (novo) | C010-060-F | Dependência de segurança: RF e localização não são concedidos pela bridge; aplicabilidade regulatória desconhecida bloqueia TX |

Casos novos, descritos em uma linha: **C010-RF010-N1** — payload externo com versão/formato desconhecido: rejeitado antes da normalização, sem efeito canônico e sem segredo na resposta. **C010-RF010-B1** — origem que descarta cadeia de bridges: continuidade `unknown`, encadeamento automático bloqueado e nenhum campo substituto inventado. **C010-RF017-N1** — endpoint descoberto sem autorização de conexão: candidato listado com origem, nenhuma sessão aberta e nenhuma rota anunciada. **C010-RF021-N1** — pedido de TX com licença ou jurisdição indeterminada: TX negado com motivo sanitizado, sem fallback para canal menos restrito. **C010-RF021-B1** — localização recebida por bridge: `confidentiality=terminated`, precisão original e transformada declaradas, sem alegação de anonimato.
