# Eventos, comandos e modelo de sincronização

> Todo evento que ingressa por bridge deve seguir a [Camada de Normalização UDNL](../01-arquitetura/normalizacao-dados.md), preservando identidade externa escopada, proveniência e qualidade da observação. O plano de mídia não publica cada quadro de áudio como evento.

## Envelope canônico
O envelope deve ser estável, versionado e incluir escopo. Exemplo **antes** de assinar/criptografar:

```json
{
  "schema_version": 1,
  "event_id": "019a0000-0000-7000-8000-000000000001",
  "tenant_id": "tenant-example",
  "origin_node_id": "node-example",
  "origin_sequence": 42,
  "stream": "device/device-example/observations",
  "type": "location.observed",
  "occurred_at": "2026-11-15T12:00:00Z",
  "payload": {
    "device_id": "device-example",
    "source": "gnss",
    "latitude": 10.0,
    "longitude": -74.0,
    "accuracy_m": 15
  }
}
```

`event_id` identifica o evento lógico e serve à deduplicação de eventos; não é a chave idempotente de um comando recebido por API/GMP. `origin_sequence` ordena dentro do namespace de origem, escopo e stream descrito no [contrato de eventos e sincronização](../07-contratos/eventos-sincronizacao.md). O envelope versionado inclui `tenant_scope`, `authority_epoch` quando houver autoridade por partição, hash do conteúdo canônico e referências à prova/autorização e à política de retenção aplicável. A codificação criptográfica e o formato wire continuam no GMP. O exemplo acima é ilustrativo, não um contrato wire definitivo.

## Eventos propostos
- `device.registered`, `device.endpoint.attached`, `device.capability.verified`
- `location.observed`, `gateway.proximity.detected`, `geofence.entered`
- `message.created`, `message.accepted`, `message.delivered`, `message.read`
- `sos.raised`, `sos.acknowledged`, `sos.escalated`, `sos.resolved`
- `ptt.floor.requested`, `ptt.floor.granted`, `ptt.floor.released`
- `radio.connector.connected`, `radio.connector.disconnected`
- `node.joined`, `node.quarantined`, `node.revoked`, `service.health.changed`

## Estados de um comando
O estado do comando, o resultado de cada tentativa de transporte e o reconhecimento humano são dimensões separadas. A projeção canônica, as transições permitidas e o mapeamento de compatibilidade constam em [eventos e sincronização §2](../07-contratos/eventos-sincronizacao.md#2-estados-de-comando-e-evidências). `completed` exige evidência específica da operação; não significa automaticamente entregue a um dispositivo nem reconhecido por uma pessoa. `resolved` pertence ao ciclo de vida do incidente SOS. **Não equiparar publicação MQTT a recepção no terminal**. Quando o protocolo não oferece confirmação fim a fim, registrar `confirmation_unavailable` na tentativa e não inferir `confirmed_by_device` ou `delivered`.

## Ingestão
1. Adaptador autentica conexão; valida tipo, tamanho, integridade, origem e autorização.
2. Normaliza evento; preserva `raw_metadata` de forma controlada, sem guardar credenciais.
3. Insere operação local e outbox na mesma transação.
4. Worker distribui para filas internas e replicação autorizada.
5. Receptores conferem integridade, versão, sujeito/tenant autorizado, origem, epoch e política de replay; persistem Inbox antes de avançar o cursor contíguo. A deduplicação de evento é distinta da idempotência do comando. Ver [eventos e sincronização §3–§6](../07-contratos/eventos-sincronizacao.md).

## Backpressure
Quotas por tenant/dispositivo, filas com limites, timeout, janelas de retry exponenciais, DLQ e degradação gradual. Alertas SOS recebem recursos reservados, mas ainda dependem da disponibilidade física da rede.

## Transcrição multimodal
## Registro de atividade de voz sem depender de transcrição
`gmp.voice.talk.started.v1` e `gmp.voice.talk.ended.v1` produzem uma entrada `gmp.channel.timeline.voice.v1` em cada transmissão observável e identificável. A entrada existe antes do processamento ASR e mostra quem transmitiu (se verificável), o canal, o horário, a duração quando conhecida e a origem. Quando transcrever não for possível, acrescentar o estado e o motivo sem texto fabricado. Uma portadora RF sem evidência individual deve ser descrita como atividade de rádio, não fala atribuída. Ver [Transcrição multimodal](../01-arquitetura/transcricao-multimodal.md) e [Contratos](../07-contratos/transcricao-sos.md).

`gmp.transcript.partial.v1`, `gmp.transcript.final.v1`, `gmp.transcript.corrected.v1`, `gmp.transcript.unavailable.v1` com `channel_id`, `talk_session_id`, indicativo GMC verificado ou identidade externa escopada, idioma, versão e política de retenção. Ver [Especificação de transcrição](../01-arquitetura/transcricao-multimodal.md).

## SOS: incidente, transporte e confirmação humana
`gmp.sos.raised.v1`, `gmp.sos.status.changed.v1`, `gmp.sos.delivery.observed.v1`, `gmp.sos.human.claimed.v1`, `gmp.sos.escalated.v1`, `gmp.sos.cancelled.v1`, `gmp.sos.resolved.v1`. **ACK de gateway não é ACK do dispositivo; ACK do dispositivo não é reconhecimento humano**. Ver [Serviço SOS](servico-sos.md) e [Contratos](../07-contratos/transcricao-sos.md).

## Localização pública e emergência (ADR-041)

`gmp.channel.location.coarse.v1` representa a área generalizada divulgável em chat autorizado, com incerteza mínima de 50 km de raio. `gmp.sos.location.precise.v1` é evento sensível de incidente, com posição de origem e qualidade reais, exclusivamente para destinatários autenticados. São **envelopes, canais, escopos e políticas de retenção distintos**. Detalhes: [Contrato GMP](../07-contratos/localizacao-privacidade.md).
