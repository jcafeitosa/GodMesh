# Contratos — GodMesh Live Presence

> **Proposta de esquema lógico**, não endpoint implantado. Projeção client-side; nunca contém coordenadas precisas nem tokens de acesso.

## Evento normalizado

```json
{
  "specversion": "1.0",
  "type": "gmp.live_presence.updated.v1",
  "source": "godmesh://app/activities",
  "id": "019ab123-4567-7abc-8def-123456789abc",
  "time": "2026-11-15T18:00:00Z",
  "data": {
    "activity_id": "activity-example",
    "kind": "guardian",
    "state": "active",
    "priority": "high",
    "summary_redacted": "Acompanhamento em andamento",
    "last_update_at": "2026-11-15T18:00:00Z",
    "expires_at": "2026-11-15T20:00:00Z"
  }
}
```

## Políticas de projeção

- `kind` ∈ `guardian`, `ptt`, `sos`, `wifi_provisioning`.
- `state` ∈ `starting`, `active`, `stale`, `ending`, `ended`, `failed`.
- SOS `state=active` não implica socorro confirmado. Confirmar entrega técnica e humano usando eventos próprios de [SOS](transcricao-sos.md).
- A camada Swift/Kotlin decide elegibilidade; um evento não força exibição pelo SO.
- Sem `ssid`, `bssid`, `latitude`, `longitude`, `gmi_id`, `access_grant_token` ou dados do registro de acessos.
- `activity_id` não deve possibilitar consulta de dados privados sem autenticação/ACL.

[Live Presence](../01-arquitetura/live-presence.md) · [App](../01-arquitetura/aplicativo-operacao-normal.md).
