# Contratos GMP — Guardian, contexto e CrowdFind

> **Especificação lógica v1.** Exemplos são esquemas conceituais e não representam implementações compiladas, APIs oficiais do SO ou dados reais.

## Contexto operacional
`gmp.guardian.context.v1`: `guardian_session_id`, `environment=land|air|sea|unknown`, `activity`, `source=user_selected|device_profile|inferred_suggestion`, `effective_at`, `expires_at`, `policy_ref`.

## Sessão temporária
`gmp.guardian.session.created.v1`: `session_id`, `owner_gmi_ref`, `expires_at`, `expected_checkin_at`, `privacy_mode=precise|coarse_50km`, `recipient_grant_refs`, `consent_proof_ref`, `beacon_opt_in=false`.

`gmp.guardian.session.state.v1`: `state=draft|consent_pending|scheduled|active|paused|completed|expired|revoked`, `connectivity_state=online|offline_buffering|stale|unknown`, `event_id`, `occurred_at`.

`gmp.guardian.checkin.v1`: `session_id`, `checkin_id`, `type=ok|arrived|need_contact`, `occurred_at`, `ack_status`.

`gmp.guardian.overdue.v1`: check-in pendente, sem equivalência automática a SOS.

## Beacon e observação
`gmp.beacon.rotating.v1` fica **no anúncio BLE** e não transporta GMI/GMC/nome/coords: um token opaco efêmero e dados mínimos compatíveis com a plataforma. O formato real BLE dependerá da implementação e das restrições Android/iOS.

`gmp.crowdfind.sighting.v1` — evento **cifrado e destinado** ao serviço de localização, sujeito a autorização; dados protegidos não entram em tópico global:

```json
{
  "type": "gmp.crowdfind.sighting.v1",
  "event_id": "example-sighting-01",
  "rotating_beacon_ref": "opaque-token-reference",
  "observer_ref": "opaque-authorization-reference",
  "measured_at": "2026-11-15T19:00:00Z",
  "received_at": "2026-11-15T19:00:10Z",
  "location_source": "observer_gnss",
  "position_ciphertext": "<encrypted-for-authorized-location-service>",
  "uncertainty_m": null,
  "confidence": "unknown",
  "ttl_seconds": 900
}
```

O evento nunca deve ser interpretado como posição GNSS **do Beacon**: indica avistamento de proximidade por um observador. Sem localização confiável do observador, registrar apenas o avistamento, sem inventar coordenadas.

## Aplicação de autorização
- `guardian.precise.read` não é acesso permanente ao histórico de todos os dispositivos.
- `crowdfind.observe` e `guardian.beacon.emit` são independentes.
- `incident.location.precise.read` depende do SOS, não de pertencer ao mesmo canal PTT.
- Toda revisão de estado de sessão e concessão é assinada e idempotente por `event_id`.

## Testes
Conferir [testes móveis](../05-operacao/testes-mobile-guardian.md), [privacidade](../04-seguranca/privacidade-crowdfind.md) e [serviço SOS](../02-dominio/servico-sos.md).
