# Adaptadores adicionais — GPS, APRS, rádios, MQTT

## GPS e rastreadores celulares
**Traccar** é candidato para aproveitar conectividade com vários modelos e protocolos de rastreamento. A integração GodMesh importa telemetria para o domínio (`location.observed`) e não confunde o IMEI ou outro identificador externo com a identidade principal do dispositivo.

## APRS-IS
APRS-IS permite integração com dados de posição e mensagens de radioamadores, nos termos da rede. Somente associar indicativo a dispositivo ou histórico privado quando existir autorização e controle de privacidade. APRS não é canal comercial/privado irrestrito.

## Outras redes consideradas no brainstorming (adaptadores opcionais a especificar e validar)
- AllStarLink/ASL3: integração analógica VoIP, mas rede pública depende de radioamador licenciado.
- EchoLink: contas/operadores verificados e autorização de rede.
- BrandMeister/DMR: acesso de hotspot e bridges sujeito a aprovação; codecs e regras específicas.
- D-STAR/XLX, C4FM/YSF, WIRES-X: gateways específicos, limitações de interoperabilidade.
- M17: protocolo aberto de rádio digital; candidato de médio prazo.
- OpenMQTTGateway: candidato para captar alguns beacons BLE autorizados.

## Conectores que podem ser descobertos
O `mesh-radio-registry` mantém `network`, `operator`, `endpoint`, `protocol`, `bridge_policy`, `rf_enabled`, `license_required`, `service_health`, `tenant_permissions` e capacidade. Descoberta **não equivale a autorização de conexão nem transmissão**.

## Critérios para novo adapter
Protocolo estável ou documentado, direitos de uso, compatibilidade IPv6/IPv4, tratamento de credenciais, métricas de disponibilidade, teste de interoperabilidade, comportamento offline e limites de tráfego.

## Perfil documental proposto para origem, deduplicação e autorização

Todo adaptador novo deve declarar no perfil versionado da integração os seguintes dados antes de o roteador anunciar uma rota como elegível. Esta lista complementa os critérios acima; não amplia direitos de uso nem aprova uma rede, jurisdição, aparelho ou modelo de operação.

| Propriedade do perfil | Valores e regra mínima |
|---|---|
| `network_kind` e `network_instance_id` | Identificam e escopam a origem. Instância descoberta sem autorização permanece não elegível para conexão. |
| `identity_methods` | Enumera o que a origem realmente autentica ou apenas reporta. Usar os níveis `IdentityEvidence` da [UDNL](../01-arquitetura/normalizacao-dados.md); não mapear um identificador externo diretamente a `gmi_id` ou GMC. |
| `origin_key_quality` | Declara se a origem garante ID estável, se o adaptador persiste um ID local por ingestão ou se não há ID estável. Não anunciar deduplicação entre reconexões quando a origem não oferece chave estável. |
| `preserves_provenance` | Booleano verificado para o caminho específico. Se a rede descarta os metadados, registrar continuidade `unknown` e impedir encaminhamento automático encadeado para outra rede externa. |
| `supported_operations` | Lista versionada de operações e direção RX/TX declaradas pelo perfil. A lista pode sustentar `supported[]`, mas não comprova `verified` ou `operational` nem concede autorização. Descoberta não ativa uma operação. |
| `command_guarantees` | Para cada operação mutável, declara suporte a chave de idempotência remota, ordem, confirmação, expiração, revogação e fencing. Capacidade ausente significa que o recurso não pode alegar essa garantia. |
| `credential_scope` | Descreve referência e escopo da credencial sem registrar segredo no catálogo, envelope ou log. Revogação e rotação seguem o contrato de identidade/autorização. |
| `retention_class` | Referência à classe de retenção aprovada para dados e registros de deduplicação. Valor ausente bloqueia prontidão da rota persistente. |
| `policy_ref` e `operator_ref` | Referenciam autorização do tenant e do operador da rede. A referência não substitui validação atual de autorização, licença, jurisdição ou consentimento. |

O `mesh-radio-registry` existente é catálogo de informação e descoberta, não autoridade de identidade, permissão ou licença. Os campos adicionais acima são uma proposta de extensão versionada. Até que a extensão seja definida e integrada ao catálogo de capacidades, o serviço não deve deduzir que um endpoint atende aos requisitos só porque já tem `network`, `endpoint`, `protocol`, `bridge_policy` ou `service_health` cadastrados.

O adaptador fornece declarações e evidências atribuíveis para o motor de capacidades; não define outra enum nem reclassifica uma capability como operacional porque a conexão está ativa. `operational` depende de observação atual da capability e de suas dependências no contexto específico. Autorização, rota elegível e readiness continuam avaliações separadas conforme o [perfil do Universal Gateway](gateway-universal.md#avaliação-de-capability-rota-e-readiness). Janela de frescor ausente impede usar essa observação para anunciar nova rota; nenhum prazo ou threshold está aprovado.

Para eventos, o adaptador usa o perfil comum `SourceProvenance` da UDNL. Ele não define sua própria interpretação de `message_id`, `origin_event_key`, `visited_bridge_ids`, `hop_count`, `hop_limit` ou TTL. Se o protocolo de origem não preservar cadeia, o evento pode ser ingerido no domínio nativo conforme política, mas o serviço recusa uma segunda projeção externa automática.

Para comandos, o adaptador usa o perfil comum de fila do [Universal Gateway](gateway-universal.md). Antes do efeito externo, o consumidor verifica ID e digest canônicos, expiração, autorização atual, epoch e fencing quando a operação exige exclusividade. Se o adaptador não consegue deduplicar uma operação remota não idempotente após resultado ambíguo, não deve reenviá-la automaticamente nem declarar sucesso.

## Casos de conformidade para entrada de adaptador

| Caso | Decisão exigida |
|---|---|
| Catálogo encontra servidor, mas não há prova de autoridade e permissão de conexão. | Registrar a descoberta sem estabelecer sessão nem anunciar rota. |
| Origem relata posição ou indicativo sem prova de autenticação. | Preservar origem e classe de evidência; não promover a identidade nem atribuir precisão não observada. |
| Rede externa remove `visited_bridge_ids` ou equivalente. | Marcar continuidade desconhecida e bloquear encadeamento automático para outra rede externa. |
| Origem não fornece ID estável de evento. | Limitar a deduplicação ao registro local persistido e não prometer exactly-once após reconexão. |
| Comando mutável tem resultado externo incerto e o protocolo não aceita chave idempotente. | Não reenviar automaticamente; preservar o estado canônico como não confirmado segundo o contrato comum. |
| Expiração, escopo de licença, autoridade, retenção ou direito de uso não foram definidos. | Bloquear prontidão da operação afetada; o usuário decide política, localidade e condições de uso. |

Este documento não fixa países, normas de rádio, modelos de hardware, prazos de retenção ou metas de disponibilidade. Esses valores dependem de decisões do usuário, parecer jurídico por jurisdição e evidência de homologação; a especificação não substitui essas aprovações.
