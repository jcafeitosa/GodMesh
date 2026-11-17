# Contratos iniciais — Transcrição e SOS

> **Status:** proposta de GMP/API, ainda não congelada. Complementa [Transcrição multimodal](../01-arquitetura/transcricao-multimodal.md) e [Serviço SOS](../02-dominio/servico-sos.md).

## GMP — Eventos de transcrição

## GMP — Evento de voz sempre presente, com ou sem transcrição

- `gmp.voice.talk.started.v1`: registro autenticado do início da transmissão, com `talk_session_id`, `channel_id`, `speaker_ref`, `origin_network`, `evidence_type` e `started_at`.
- `gmp.voice.talk.ended.v1`: encerra ou marca interrupção da transmissão, com `ended_at`, `duration_ms` quando verificável, `end_reason` e referência ao evento inicial.
- `gmp.channel.timeline.voice.v1`: projeção idempotente para o chat (uma entrada por `talk_session_id`); existe **mesmo sem transcrição** e pode conter texto ASR quando permitido.
- `gmp.transcript.unavailable.v1`: relaciona `talk_session_id` e `reason` (`disabled_by_policy`, `consent_missing`, `processor_unavailable`, `processing_failed`, `audio_unintelligible`, `unsupported_source`, `e2ee_unavailable`).

```json
{
  "type": "gmp.channel.timeline.voice.v1",
  "event_id": "event-voice-example",
  "channel_id": "operations",
  "talk_session_id": "talk-42",
  "speaker_ref": {
    "display_id": "GM-7K4P-9R2Q-X",
    "origin_network": "godmesh",
    "identity_assurance": "godmesh_verified"
  },
  "evidence_type": "ptt_granted",
  "started_at": "2026-11-15T14:32:18Z",
  "duration_ms": 7000,
  "transcription_state": "unavailable",
  "transcription_reason": "processor_unavailable",
  "text": null
}
```

Exemplo ilustrativo; autenticação, ACL, retenção e proveniência completas pertencem ao envelope GMP. Nunca afirmar que uma pessoa falou quando somente a portadora RF foi detectada. Registros de metadados seguem política de visibilidade, mesmo quando a transcrição está desativada.


- `gmp.transcript.partial.v1`: hipótese transitória; não é histórico definitivo.
- `gmp.transcript.final.v1`: texto final normalizado com `channel_id`, `talk_session_id`, `speaker_ref`, `origin_network`, `language`, `start/end`, `revision`.
- `gmp.transcript.corrected.v1`: revisão ligada a evento final, autor e motivo auditáveis.
- `gmp.transcript.unavailable.v1`: falha/desativação, sem inventar texto.

## GMP — Eventos SOS

- `gmp.sos.raised.v1`: cria incidente com idempotency key, assinatura e escopo.
- `gmp.sos.status.changed.v1`: transição justificada e verificável.
- `gmp.sos.delivery.observed.v1`: ACK técnico por transporte/destino.
- `gmp.sos.human.claimed.v1`: destinatário humano autenticado assumiu responsabilidade.
- `gmp.sos.escalated.v1`, `gmp.sos.cancelled.v1`, `gmp.sos.resolved.v1`.

## Endpoints conceituais

```text
GET  /v1/channels/{channel_id}/timeline
GET  /v1/channels/{channel_id}/transcription-policy
PUT  /v1/channels/{channel_id}/transcription-policy
POST /v1/incidents/sos
GET  /v1/incidents/{incident_id}
POST /v1/incidents/{incident_id}/claim
POST /v1/incidents/{incident_id}/cancel
POST /v1/incidents/{incident_id}/resolve
GET  /v1/incidents/{incident_id}/delivery
```

Todos exigem autenticação e ACL; endpoints de SOS devem admitir reconciliação de incidentes criados offline. Não inferir sucesso de despacho por respostas HTTP 200/202; expor estados e evidências separadamente. Rate limiting deve proteger infraestrutura sem impedir solicitações legítimas em situações críticas. A interface deve sempre mostrar claramente quais destinatários, se algum, confirmaram o recebimento humano.

## Modelo SQL (proposta de entidades)

```text
channel_transcription_policies
channel_timeline_entries
transcript_segments
transcript_revisions
transcription_processor_audit
incidents
incident_events
incident_recipients
incident_delivery_attempts
incident_human_acknowledgements
incident_location_observations
incident_escalation_policies
```

Campos comuns: `tenant_id`, `channel_id`, `event_id`, `origin_node_id`, `created_at`, `policy_id`, `retention_until`, `provenance`, `actor_ref` e `test_mode` quando pertinente. Isolamento multi-tenant e criptografia ao repouso são obrigatórios. Projeto de índices espaciais e partições segue a política regional do GodMesh.

## Contratos e escopos de localização (ADR-041)

A timeline de voz pode anexar somente `gmp.channel.location.coarse.v1` (incerteza espacial de pelo menos **50 km de raio**) mediante permissão explícita. O SOS utiliza `gmp.sos.location.precise.v1` em canal segregado, sem arredondamento artificial, `accuracy_m` e `observed_at` preservados, restrito a `incident.location.precise.read`. **Nunca** inserir posição precisa no chat comum ou nas bridges por padrão. Ver [Contrato detalhado](localizacao-privacidade.md).
