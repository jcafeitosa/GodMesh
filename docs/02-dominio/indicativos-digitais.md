# GodMesh Callsign (GMC) — Indicativo digital pseudônimo

> **Status:** decisão arquitetural proposta e aceita para especificação GMP; ainda não implementada. **ADR-035**. Data: 2026-11-15.  
> **Prevalece** sobre a interpretação anterior de que `acct:apelido@domínio` seja o identificador público obrigatório.

## Objetivo

Cada pessoa recebe um **indicativo digital único, reconhecível como GodMesh e não derivado de seu nome civil, telefone, e-mail, IP ou documento**. É o indicativo apresentado durante mensagens, chamadas PTT e comunicações autorizadas. Não é um indicativo de radioamador expedido por autoridade pública. Um indicativo é um **pseudônimo, não anonimato forte**; voz e metadados também podem revelar informações pessoais.

## Três níveis de identificação

| Campo | Função | Divulgação |
|---|---|---|
| `gmi_id` | Identidade interna permanente UUIDv7 (RFC 9562) | Não transmitido como identificador público padrão |
| `gmc_callsign` | Indicativo público único na federação GodMesh | Exibido aos participantes autorizados da comunicação |
| `session_talker_id` | Referência compacta e efêmera de sessão/fluxo | Transportada nos pacotes de mídia quando necessário |

Um endereço `acct:apelido@domínio` continua **opcional**, para descoberta voluntária, não é necessário para falar nem é prova de identidade. A publicação de perfil, nome ou outros atributos deve ser opt-in.

## Formato e concessão

**Formato ilustrativo v1:** `GM-7K4P-9R2Q-X` (prefixo de plataforma + sequência legível + verificador). Alfabeto recomendado: Crockford Base32 ou variante de caracteres sem ambiguidade; o comprimento/algoritmo e algoritmo de check-digit exigem definição normativa antes da implementação. **Não interpretar o exemplo como indicativo reservado ou já atribuído.**

- `GM` identifica a **classe de indicativos GodMesh**; a validade real é conferida por credencial assinada de uma autoridade federada confiável, não pela aparência da string.
- Uma autoridade de emissão (registro lógico com quórum ou partições de alocação sem conflito) reserva o indicativo e vincula-o ao `gmi_id` e à chave/credencial autorizada.
- Reservar `callsign` globalmente de maneira atômica. Se o registro estiver indisponível, criar um **identificador provisório local não reconhecido globalmente**, nunca declarar unicidade global sem confirmação.
- Indicativos são persistentes e **não reciclados**; revogação não permite que outra pessoa herde a reputação do antigo titular. Política explícita para recuperação/renovação sem trocar o código quando permitido.
- O processo de apresentação pública não expõe `gmi_id`, dados pessoais, perfis privados, localização, endereços de rede ou histórico de canais.
- A autenticação de usuário/dispositivo e a prova de vinculação de indicativo são operações distintas. Apenas digitar o indicativo nunca autoriza transmissão.

## Credencial de indicativo (conceitual)

```json
{
  "format": "gmp.callsign-credential.v1",
  "callsign": "GM-7K4P-9R2Q-X",
  "key_id": "key-ephemeral-example",
  "issuer": "issuer-federation-example",
  "valid_until": "2027-01-01T00:00:00Z",
  "signature": "<assinatura_verificavel>"
}
```

O vínculo entre `gmi_id`, titular e chave é protegido na infraestrutura de identidade; a credencial exibida não deve permitir resolver o nome civil, e o esquema final deve escolher claramente entre credencial de chave delegada, proof-of-possession, expiração, rotação e revogação. Assinaturas são verificadas com chaves de emissores autorizados. Proteção contra replay combina contexto da sessão, `channel_id`, `grant_id`, nonce, expiração e sequência.

## Privacidade e pseudônimos por contexto

O `gmc_callsign` é estável e torna possível reconhecer participantes ao longo do tempo. Isso também cria possibilidade de **correlação e rastreamento entre canais**. Para espaços sensíveis à privacidade, oferecer um `on_air_alias` temporário **derivado e atestado para a sessão ou canal**, sem divulgar globalmente o indicativo estável. O servidor de confiança vincula alias efêmero à credencial real, quando necessário para abuso, auditoria e revogação sob políticas legais; demais usuários verão somente o alias. Isso não deve criar mecanismo de evasão de responsabilização.

**Em redes de radioamador:** um indicativo digital GodMesh não substitui indicativos legalmente exigidos nem as regras de identificação de estação. Pontes externas mostram/transportam apenas metadados que a rede permitir, sem prometer pseudonimato na rede externa.

## Dados e API propostos

```sql
CREATE TABLE callsign_registry (
  callsign TEXT PRIMARY KEY,
  gmi_id UUID NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK (state IN ('active','suspended','revoked')),
  issuer_id TEXT NOT NULL,
  issued_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ
);
```

`GET /v1/identity/me/callsign`, `POST /v1/identity/callsigns/claim`, `GET /v1/callsigns/{callsign}/credential` (retornar apenas metadados públicos necessários, limitar enumeração). Contratos de APIs/credenciais e autoridade de escrita devem ser definidos para produção.

## Critérios de aceitação

1. Dois nós concedem indicativos distintos sem colisão e com autoridade validada; partição não produz duplicação reconhecida globalmente.
2. A mensagem e o PTT exibem somente o indicativo/alias autorizado, sem enviar nome real ou `gmi_id` público.
3. Cliente falsificando `callsign` é rejeitado em ingresso, sinalização e autorização de PTT.
4. Rotação de chave, reconexão e movimentação de servidor preservam o indicativo; identificadores efêmeros mudam por sessão.
5. Revogação impede novos grants; cache de credenciais respeita expiração e política de invalidação.
6. Perfis públicos não permitem pesquisa de pessoas por nome/telefone por padrão; a camada antifraude e políticas legais mantêm governança apropriada.
