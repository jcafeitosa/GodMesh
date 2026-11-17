# Contratos REST/GMP — GodMesh Wi-Fi AutoDiscover, AutoShare e AutoConnect

> **WIFI-API-001 · Versão lógica 1.0 · 2026-11-15 · Proposta ainda não implementada.** Segurança por tenant, grants, verificação de controlador, idempotência, autenticação forte e política de divulgação exata.

## 1. Identidades, privacidade e origem da informação

- `gmi_id`: identidade interna de usuário; `gmc` é indicativo público e não substitui autenticação.
- `network_id`: rede lógica; `ap_id`: ponto de acesso físico; `share_id`: oferta de acesso; `grant_id`: permissão individual ou de grupo; `connection_session_id`: sessão operacional.
- `location_exact`: dado privado do AP, nunca posição GPS da pessoa; `map_publication`: projeção permitida com `exact_opt_in | coarse | hidden`.
- `discovery_observation` pertence ao inventário privado; observação de SSID por telefone não registra nem publica rede em nome do titular.
- Operações com efeito devem receber `Idempotency-Key` e autenticação/escopos suficientes. Identificadores e tokens exemplificados são fictícios.

## 2. Endpoints propostos

| Método e rota | Objetivo | Escopo mínimo |
|---|---|---|
| `GET /v1/wifi/discovery/capabilities` | Capacidades e permissões por dispositivo | `wifi.discovery.read` |
| `POST /v1/wifi/inventory/sync` | Sincronizar inventário **privado** com consentimento | `wifi.inventory.write` |
| `POST /v1/wifi/networks/claims` | Solicitar comprovação de administração | `wifi.claim.write` |
| `POST /v1/wifi/controllers/{id}/pair` | Parear agente, validando desafio assinado | `wifi.controller.admin` |
| `POST /v1/wifi/access-points` | Criar AP administrado com coordenada/qualidade | `wifi.ap.manage` |
| `PATCH /v1/wifi/access-points/{id}/location` | Atualizar ponto e evidência | `wifi.ap.manage` |
| `POST /v1/wifi/shares` | Criar política de compartilhamento | `wifi.share.manage` |
| `PATCH /v1/wifi/shares/{id}/publication` | Opt-in de localização exata para mapa | `wifi.share.publish` |
| `GET /v1/wifi/map?bbox=...` | Consultar projeção autorizada | `wifi.map.read` |
| `POST /v1/wifi/shares/{id}/grants` | Emitir autorização revogável | `wifi.grant.issue` |
| `POST /v1/wifi/grants/{id}/enroll` | Provisionar credencial temporária ao usuário | `wifi.connect` |
| `POST /v1/wifi/connections` | Registrar solicitação do cliente | `wifi.connect` |
| `GET /v1/wifi/me/sessions` | Histórico próprio de conexões | `wifi.session.self.read` |
| `GET /v1/wifi/access-points/{id}/sessions` | Auditoria restrita do AP | `wifi.session.admin.read` |
| `POST /v1/wifi/grants/{id}/revoke` | Revogar grant e iniciar enforcement | `wifi.grant.revoke` |

Todos os endpoints de escrita fazem validação server-side de tenant, função, escopo temporal, contrato de consentimento e estado do controlador. `GET /v1/wifi/map` não retorna segredos, visitantes, GMI, BSSID bruto ou inventário privado.

## 3. Criar AP e consentir publicação exata

```json
{
  "network_id": "019ab123-4567-7abc-8def-123456789abc",
  "display_name": "Wi-Fi Comunitário",
  "location_exact": {
    "latitude": -8.0500,
    "longitude": -34.9000,
    "accuracy_m": 12.0,
    "coordinate_source": "owner_gnss",
    "observed_at": "2026-11-15T15:00:00Z"
  },
  "controller_id": "controller-example",
  "publication": {
    "mode": "hidden",
    "exact_location_opt_in": false
  }
}
```

Após comprovar controle e autorizar **separadamente** a publicação exata:

```json
{
  "mode": "exact_opt_in",
  "exact_location_opt_in": true,
  "consent_version": "wifi-location-publication-v1",
  "consented_at": "2026-11-15T15:02:00Z"
}
```

A API não deve aceitar apenas esse booleano como prova suficiente: ela também valida titularidade/controlador, autenticação recente, escopo e trilha de auditoria. `accuracy_m` expressa qualidade informada/estimada da posição do AP; não garante precisão absoluta.

## 4. Solicitação de conexão e estados

Exemplo de resposta, sem senha exposta:

```json
{
  "connection_session_id": "session-example",
  "network_id": "network-example",
  "ap_id": "ap-example",
  "state": "os_approval_required",
  "method": "ios_hotspot_configuration",
  "grant_expires_at": "2026-11-15T18:00:00Z",
  "credential_delivery": "platform_secure_provisioning",
  "internet_verified": false
}
```

Estados: `requested`, `eligible`, `credential_requested`, `credential_ready`, `os_deciding`, `os_approval_required`, `associating`, `associated`, `local_only`, `captive_portal`, `internet_verified`, `failed`, `disconnected`, `revocation_pending_router`.

`requested != associated != internet_verified`; não inferir um pelo outro. `provisioning` não concede garantia de uso, e controle de sessão depende do SO e controlador.

## 5. Evento de sessão autenticada pelo controlador

```json
{
  "specversion": "1.0",
  "type": "gmp.wifi.session.started.v1",
  "source": "godmesh://wifi/controller/controller-example",
  "id": "019ab123-4567-7abc-8def-123456789abd",
  "time": "2026-11-15T15:10:00Z",
  "data": {
    "tenant_id": "tenant-example",
    "ap_id": "ap-example",
    "grant_id": "grant-example",
    "connection_session_id": "session-example",
    "evidence_level": "controller_attested",
    "started_at": "2026-11-15T15:09:58Z",
    "bytes_rx": null,
    "bytes_tx": null
  }
}
```

Eventos do app chegam com `evidence_level=client_claim`, nunca `controller_attested` sem atestação de roteador/RADIUS. Assinaturas e autenticação do canal fazem parte da camada de transporte/identidade; não confiar em campos `source` fornecidos por cliente arbitrário.

## 6. Federação e consistência

`wifi.network.observed_private` não é publicado globalmente. Publicações autorizadas: `gmp.wifi.share.published.v1`, `gmp.wifi.ap.location.changed.v1`, `gmp.wifi.share.revoked.v1`, `gmp.wifi.grant.revoked.v1`. Replicar apenas projeções e grants permitidos. Uso e sessões permanecem no domínio do AP ou do titular, exceto sincronização explicitamente autorizada. Descartar versões anteriores após revogação/tombstone.

## 7. Erros de domínio

`WIFI_PERMISSION_REQUIRED`, `WIFI_OS_UNSUPPORTED`, `WIFI_OWNER_UNVERIFIED`, `WIFI_AP_NOT_REACHABLE`, `WIFI_APPROVAL_REQUIRED`, `WIFI_SHARE_EXPIRED`, `WIFI_GRANT_REVOKED`, `WIFI_CONTROLLER_OFFLINE`, `WIFI_ACCESS_LIMIT_REACHED`, `WIFI_NOT_IN_RANGE`, `WIFI_INTERNET_UNVERIFIED`, `WIFI_LOCATION_PUBLICATION_DENIED`.

[Modelo Wi-Fi](../02-dominio/wifi-modelo-dados.md) · [Access Registry](../02-dominio/wifi-registro-acessos.md) · [Segurança](../04-seguranca/wifi-compartilhamento-privacidade.md).
