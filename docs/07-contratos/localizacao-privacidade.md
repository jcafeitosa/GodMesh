# Contratos GMP — Localização aproximada no chat e precisa no SOS

> **Status:** proposta de contrato normativo — **ADR-041**; ainda não implementado. Alinhado à [Política de localização](../04-seguranca/politica-localizacao.md), [SOS](../02-dominio/servico-sos.md) e [Eventos](../02-dominio/eventos.md).

## 1. Eventos separados por público e ACL

### Chat/canal — localização aproximada

```json
{
  "type": "gmp.channel.location.coarse.v1",
  "channel_id": "operations",
  "speaker_ref": {"display_id": "GM-7K4P-9R2Q-X"},
  "location": {
    "precision_class": "coarse_50km_plus",
    "region_label": "Região ampla compartilhada",
    "geometry_ref": "public-coarse-zone-001",
    "uncertainty_radius_min_km": 50,
    "generated_at": "2026-11-15T18:00:00Z"
  },
  "visibility": "channel_members_authorized"
}
```

`geometry_ref` aponta para uma área generalizada do catálogo de regiões **sem exposição do ponto GPS original**. O servidor deve rejeitar campos `lat`, `lon`, `altitude`, `raw_gps`, `precise_position` neste evento e em qualquer mensagem/transcrição pública. Nem a API nem a bridge deve retornar coordenadas exatas disfarçadas de campos auxiliares.

### SOS — localização precisa protegida

```json
{
  "type": "gmp.sos.location.precise.v1",
  "incident_id": "incident-example-01",
  "location": {
    "latitude": null,
    "longitude": null,
    "accuracy_m": null,
    "observed_at": "2026-11-15T18:00:00Z",
    "source": "gnss",
    "quality": "not_yet_available"
  },
  "visibility": "incident_authorized_responders",
  "required_scope": "incident.location.precise.read"
}
```

O exemplo usa `null` para não inventar posição GPS real. Em uma ocorrência com GNSS autorizado, substituir pelos valores de origem **sem arredondamento intencional**, preservando estimativa de precisão e horário. O evento deve circular somente por canal/armazenamento protegido, com criptografia e auditoria de acesso; não ser publicado em tópico gossip global ou timeline de chat.

## 2. Controle de autorização

- `channel.location.coarse.read`: visualizar região generalizada de canal.
- `channel.location.coarse.publish`: enviar localização aproximada mediante consentimento/ACL.
- `incident.location.precise.read`: acesso individual/autenticado por incidente e propósito autorizado.
- `incident.location.precise.update`: atualização da fonte com prova de origem e relação válida com incidente.
- `incident.location.precise.share`: delegação explícita e limitada; proibida para participantes comuns por padrão.

## 3. APIs de referência

```text
GET  /v1/channels/{channel_id}/location-coarse
GET  /v1/incidents/{incident_id}/location-precise
POST /v1/incidents/{incident_id}/location-observations
```

Autorização deve ser verificada no servidor em cada requisição; retorno exato de SOS utiliza `Cache-Control: no-store`, sem link público ou cache CDN. Auditoria e expiração de credenciais fazem parte da implementação. Acesso de emergência e identidade não podem ser derivados apenas da posse de um ID de incidente.
