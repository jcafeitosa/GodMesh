# Contratos GMP — monitoramento de rede e ações manuais

> **Proposta de eventos v1**, sem prometer acesso à rádio base, ativação por software de satélite nem controle de aplicativos terceiros.

## Estados
- `connection_quality`: `normal|degraded|offline|limited_online|unknown`.
- `transport_class`: `wifi_ip|terrestrial_cellular|satellite_carrier|satellite_terminal_wifi|local_peer|radio_gateway|unknown`.
- `satellite_evidence`: `platform_api|system_hint|user_report|none|unknown`.
- `capability`: `messaging|location|ptt|sos_delivery|unknown` com disponibilidade avaliada separadamente.
- `user_action`: `open_settings|follow_instructions|confirm_status|none`.
- `action_status`: `suggested|opened_settings|confirmed_by_user|not_possible|dismissed`.

## Exemplo de alteração de rede
```json
{
  "type": "gmp.client.connectivity.changed.v1",
  "event_id": "example-net-001",
  "device_ref": "own-device-ref",
  "observed_at": "2026-11-15T19:05:00Z",
  "network": {
    "connection_quality": "offline",
    "transport_class": "unknown",
    "signal_report": "unavailable",
    "satellite_evidence": "unknown"
  },
  "delivery": { "offline_queue": true, "last_ack_at": null }
}
```

Esse evento contém estado operacional minimizado; não expor IMSI, número de telefone, célula, BSSID ou histórico exato de deslocamento em chat/PTT.

## Exemplo de orientação manual
```json
{
  "type": "gmp.client.manual_action.suggested.v1",
  "action_id": "battery-saver-example-01",
  "reason": "battery_low_during_guardian",
  "target": "user_device_settings",
  "action": "open_settings",
  "instructions_id": "battery_saver_android_or_ios",
  "optional": true,
  "requires_user_confirmation": true,
  "acknowledgement": "suggested"
}
```

O aplicativo não envia um evento `activated` apenas porque mostrou instruções ou abriu Ajustes. A confirmação é do usuário, ou uma API oficial que realmente informe o novo estado. Na ausência de API, marcar `unverified`.

## Mensagens de erro importantes
- `satellite_service_unavailable`: nenhuma integração da operadora confirmada.
- `satellite_app_data_not_supported`: o aparelho pode acessar uma função satelital, mas o serviço GodMesh não é elegível.
- `action_user_required`: abrir configurações/orientar, sem automação.
- `delivery_not_confirmed`: solicitação em fila ou encaminhada, sem ACK do destino.
- `essential_services_preserved`: política de economia de dados não desabilitou SOS/Guardian essenciais.

## Priorização e privacidade
`priority=emergency|safety|essential|interactive|bulk`, com TTL/retry e limites próprios. `gmp.client.connectivity.changed.v1` é telemetria privada para o dispositivo e serviços autorizados, não presença pública. Nenhuma mudança de rede cria SOS automaticamente. Contatos podem receber, se autorizado, apenas estado operacional mínimo e idade da última sincronização.

## Relações
- [Contingência móvel](../01-arquitetura/conectividade-contingencia-satelite.md)
- [Cliente e sensores](../01-arquitetura/cliente-mobile-sensores-conectividade.md)
- [Guardian](../02-dominio/guardian-acompanhamento.md)
- [SOS](../02-dominio/servico-sos.md)
