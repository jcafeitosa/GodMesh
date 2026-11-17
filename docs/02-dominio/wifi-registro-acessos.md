# GodMesh Wi-Fi Access Registry — histórico de acessos e controle operacional

> **WIFI-DOM-002 · Versão 1.0 · 2026-11-15 · Estado: especificação aprovada, implementação pendente.** Complementa o [modelo de dados Wi-Fi](wifi-modelo-dados.md).

## 1. Objetivo

Registrar **quem foi autorizado e quais sessões de uso realmente ocorreram** nos pontos de acesso compartilhados pelo GodMesh. O serviço suporta cobrança/quotas futuras, auditoria operacional, segurança, capacidade e resolução de problemas, **sem** coletar conteúdo de navegação, mensagens ou URLs.

### Distinção de evidências

| Evento | Fonte | O que prova |
|---|---|---|
| `connect.requested` | App | O usuário pediu conexão, não que se conectou |
| `credential.issued` | Provisionador | Uma credencial foi criada/entregue, não que foi usada |
| `os.associated` | App/OS | O dispositivo informou associação ao AP, ainda não prova autenticação auditável do controlador |
| `ap.authenticated` | Roteador/RADIUS | Controlador informou autenticação aceita |
| `ap.session.started` | Controlador confiável | Sessão operacional iniciada |
| `ap.usage.reported` | Contadores do controlador | Bytes/duração medidos naquele ponto, com origem identificada |
| `ap.session.ended` | Controlador ou timeout | Encerramento confirmado ou inferido, com `evidence_level` explícito |

O evento de sessão deve carregar `source`, `observed_at`, `evidence_level` e a versão do mecanismo de coleta. Nunca confundir “solicitado” com “conectado” ou “navegação concluída”.

## 2. Dados permitidos por finalidade

- **Identidade:** `gmi_id` interno de quem recebeu grant, indicativo GMC apenas quando necessário à UI, organização/tenant e dispositivo GodMesh; não divulgar identidade civil.
- **Rede:** `wifi_network_id`, `wifi_ap_id`, `share_id`, `grant_id`, controlador que atestou e posição interna do AP.
- **Sessão:** IDs aleatórios, início/fim/último heartbeat, tempo, outcome, falhas e causa da desconexão quando conhecida.
- **Uso:** bytes ingress/egress, banda/qualidade e contagem de reconexões, **somente quando medidos**; indicadores de consumo ausentes ficam `null`.
- **Segurança:** resultado de autenticação, revogação, limite de quota e evento de política; nunca registrar senha, token ou payload de navegação.
- **Endereçamento:** IP transitório e identificadores de camada 2 somente quando indispensáveis a diagnóstico específico e com retenção curta; MAC aleatório não é identidade persistente.

## 3. Visibilidade e retenção

| Papel | Pode consultar |
|---|---|
| Participante | Suas próprias conexões e consumo disponível |
| Administrador do AP | Sessões no AP sob sua administração, com pseudônimo de conta e mínimo necessário |
| Operador de segurança GodMesh | Eventos restritos mediante função e auditoria |
| Público no mapa | **Nenhum** registro de pessoas que se conectaram |

Recomenda-se retenção parametrizável por jurisdição/organização, com minimização e política de exclusão. **Não definir retenção indefinida como padrão**. Dados brutos de IP/MAC devem ter janela bem menor que agregados de consumo. Toda consulta administrativa precisa de trilha de auditoria.

## 4. Extensão SQL de referência

As tabelas `wifi_networks`, `wifi_access_points`, `wifi_access_grants` e `wifi_connection_sessions` são definidas no [modelo Wi-Fi](wifi-modelo-dados.md). O esquema abaixo complementa, mas não é migração pronta.

```sql
CREATE TABLE wifi_access_session_evidence (
  event_id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id),
  connection_session_id UUID NOT NULL REFERENCES wifi_connection_sessions(id),
  evidence_source TEXT NOT NULL CHECK (
    evidence_source IN ('client','router_agent','radius','operator')),
  evidence_level TEXT NOT NULL CHECK (
    evidence_level IN ('client_claim','controller_attested','inferred')),
  event_type TEXT NOT NULL,
  occurred_at TIMESTAMPTZ NOT NULL,
  received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  bytes_rx BIGINT CHECK (bytes_rx >= 0),
  bytes_tx BIGINT CHECK (bytes_tx >= 0),
  rssi_dbm SMALLINT,
  metadata_redacted JSONB NOT NULL DEFAULT '{}'::jsonb,
  UNIQUE(connection_session_id, event_id)
);

CREATE INDEX wifi_session_evidence_lookup
  ON wifi_access_session_evidence(connection_session_id, occurred_at);
```

Implementação exige também FK composta de escopo tenant, RLS, auditoria de leitura, idempotência por fonte e `controller_id` com assinatura/identidade forte. A contabilidade pode ser eventual e atrasada. Quotas só podem ser aplicadas no controlador compatível.

## 5. Eventos GMP

`gmp.wifi.access.requested.v1`, `gmp.wifi.auth.accepted.v1`, `gmp.wifi.session.started.v1`, `gmp.wifi.session.usage.v1`, `gmp.wifi.session.ended.v1`, `gmp.wifi.access.revoked.v1`.

O **Access Registry não deve servir como rastreador de deslocamento do visitante**. O fato de a pessoa ter usado um AP não concede ao proprietário acesso ao histórico Guardian ou às suas coordenadas GNSS.

## 6. Aceite

- Sessão apenas iniciada pelo app permanece `requested`, não `authenticated`.
- Duas tentativas ou dois eventos repetidos não duplicam cobrança/consumo.
- Apenas controlador autenticado eleva evento a `controller_attested`.
- Administrador consulta apenas seus APs; outros tenants e visitantes não recebem dados de terceiros.
- Sem telemetria do roteador: campos de consumo ficam desconhecidos, sem inventar valores.

[Privacidade](../04-seguranca/wifi-compartilhamento-privacidade.md) · [Contratos API](../07-contratos/wifi-autodiscover-autoshare-api.md) · [Testes](../05-operacao/testes-wifi.md).
