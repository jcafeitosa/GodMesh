# Contratos externos — API e GMP (especificação)

> URLs e métodos propostos; nenhum endpoint é declarado como já implementado. O servidor deriva principal e tenant do contexto autenticado e valida membership/ACL; qualquer tenant indicado pelo cliente é apenas contexto alegado e nunca concede autorização.

Contrato detalhado proposto, com schemas, perfis de transporte, handshake, autorização por operação e erros: [GMP / API v1](gmp-api-v1.md). O mapa de principal/scope vem de [identidade/autorização](identidade-autorizacao.md); IDs, lifecycle, attempts, ACK, idempotência e cursor de [eventos/sincronização](eventos-sincronizacao.md); epoch de [autoridade de escrita](../01-arquitetura/autoridade-escrita.md); política por cópia de [retenção/revogação](../04-seguranca/retencao-revogacao.md); proveniência de [UDNL](../01-arquitetura/normalizacao-dados.md) e aplicação bridge de [Gateway](../03-integracoes/gateway-universal.md). Todos permanecem propostas documentais. Este catálogo é índice, não contrato executável; OpenAPI formal ainda não foi criado ou ratificado.

## Convenções
- HTTPS com bearer token e DPoP sender-constrained para clientes públicos; mTLS é alternativa serviço-a-serviço. JWT versus token opaco/introspecção ainda não foi escolhido; identidade de workload/nó usa chaves verificáveis conforme perfil.
- `/v1` nas APIs públicas; `Idempotency-Key` em operações mutáveis apropriadas; paginação, cursores e limites documentados.
- Erro tipado com `code`, `message`, `request_id`, `retry_after` opcional e detalhes não sensíveis.
- `trace_id` em logs/traces; localizações e segredos não publicados nos logs por padrão.
- `tenant_id` e `device_id` validados contra vínculo/ACL; prova de posse em associação de hardware.

## Identidade pública e descoberta
Identificador `gmi_id` imutável (UUIDv7), `urn:uuid:<id>` como URN padronizada; alias `acct:apelido@domínio` (RFC 7565), descoberta por `/.well-known/webfinger` (RFC 7033) e descritor de serviço GodMesh autenticado. Os nomes **não** são credenciais. Consulte [GMI](../02-dominio/identidade-global.md).

## Indicativo digital pseudônimo
O GodMesh identifica participantes publicamente pelo **GMC Callsign** (ex.: `GM-7K4P-9R2Q-X`); `gmi_id` é interno, `acct:` é descoberta opt-in. Mensagens transportam indicativo e prova de origem; PTT sinaliza `TalkStart` e usa `stream_id` efêmero nos frames. Ver [GMC](../02-dominio/indicativos-digitais.md) e [GMP identidade e mídia](../01-arquitetura/identificacao-midia-mensagens.md).

## Endpoints propostos
| Método | Endpoint | Responsabilidade |
|---|---|---|
| GET | `/v1/health/live` | Liveness |
| GET | `/v1/health/ready` | Readiness por serviço/dependência |
| GET | `/v1/regions` | Regiões elegíveis autorizadas |
| GET | `/v1/identity/me` | Identidade global da sessão autorizada |
| GET | `/v1/identity/resolve` | Público para alias `acct:` com publicação opt-in; somente projeção publicada, sem GMI/tenant oculto; não autentica |
| POST | `/v1/identity/handles` | Alterar handle próprio sob prova/autoridade |
| POST | `/v1/identity/callsigns/claim` | Solicitar GMC Callsign |
| POST | `/v1/identity/migration/initiate` | Solicitar transferência assinada de autoridade da identidade |
| POST | `/v1/identity/migration/confirm` | Confirmar transferência com provas do titular e emissor |
| GET | `/v1/callsigns/{callsign}/credential` | Credencial GMC publicada |
| GET/POST | `/v1/assets` | Ativos do tenant |
| GET/POST | `/v1/devices` | Dispositivos próprios/autorizados |
| GET | `/v1/devices/{id}/capabilities` | Declaradas, verificadas e disponíveis |
| GET | `/v1/devices/{id}/location` | Última posição, fonte e precisão |
| GET | `/v1/devices/{id}/observations` | Histórico autorizado |
| POST | `/v1/devices/{id}/commands` | Comando com TTL e idempotência |
| POST | `/v1/routes/resolve` | Serviço e caminho elegíveis por capacidades/ACL |
| GET/POST | `/v1/channels` | Canais nativos GodMesh |
| POST | `/v1/channels/{id}/sessions` | Sessão nativa de mídia |
| POST | `/v1/channels/{id}/floor` | Solicitar direito PTT (GMP control equivalente) |
| GET/POST | `/v1/bridges` | Listar/configurar bridges autorizadas |
| GET | `/v1/radio/services` | Descoberta de redes externas autorizadas |
| POST | `/v1/federation/join-requests` | Ingresso de nó validado |
| GET | `/v1/federation/peers` | Peers permitidos |
| POST | `/v1/federation/attest` | Atestar nó por emissor/autoridade autorizada |
| GET | `/v1/federation/manifests/current` | Manifesto de federação assinado |
| POST | `/v1/federation/leave` | Revogar próprio vínculo de nó |
| GET | `/v1/events/stream` | Stream autenticado com cursor opaco de C10 |

O contrato C09 também propõe `GET /.well-known/webfinger` e `GET /.well-known/godmesh`; são endpoints públicos fora de `/v1`, detalhados na seção de identidade pública. A tabela ampliada espelha a proposta C09 sem afirmar que os métodos estejam implementados; o processo OpenAPI ainda precisa fixar media types e respostas por operação.

## Eventos/stream
`/v1/events/stream`: streaming por escopo com cursor opaco vinculado a checkpoints contíguos, filtros, autorização, schema e retenção efetiva C10/C08. ACK somente após validação e commit durável na Inbox, avançando apenas o prefixo contíguo; não comprova materialização, entrega, conclusão de comando, réplica global ou descarte. WebSocket versus outro framing permanece por definir em GMP.

`GET /v1/events/stream` é a operação proposta; autorização usa `events.read`, e o cursor não substitui ACL nem pode ser transferido a outro principal/filtro. Cursor vencido pelo intervalo retido requer `CURSOR_EXPIRED` e recuperação por snapshot apenas se ainda autorizada.

## Controles GMP
Consultar [GMP](../01-arquitetura/protocolo-nativo.md). Domínios lógicos `gmp.control.v1`, `gmp.event.v1`, `gmp.federation.v1`, `gmp.media.v1` e `gmp.bridge.v1`. **O transporte de mídia não usa fila de eventos**.

## Entrega de comandos e SOS
Os nomes `created`, `persisted`, `routed`, `accepted_by_gateway`, `confirmed_by_device` (quando suportado), `acknowledged_by_person` e `resolved` preservam o vocabulário do rascunho inicial; não constituem uma máquina independente. O crosswalk proposto em [eventos/sincronização](eventos-sincronizacao.md) distingue `Command.lifecycle`, `DeliveryAttempt.state`, `HumanAcknowledgment.state` e estado de incidente. GMP propõe `CommandReceipt.status` para lifecycle e tentativa em campo separado. O contrato ainda não foi ratificado; consumidores não devem inferir que um ACK de gateway/rede/dispositivo ou `resolved` equivale a entrega, conclusão ou reconhecimento humano.

Resposta `202 Accepted` significa somente que a solicitação foi aceita para processamento; o corpo devolve um recibo/estado conforme [GMP / API v1](gmp-api-v1.md). Nunca responder `delivered=true` só por publicação MQTT. ACK de gateway, rede, dispositivo e pessoa permanecem distintos, conforme ADR-039.
