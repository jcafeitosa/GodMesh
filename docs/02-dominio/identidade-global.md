# GodMesh Global Identity (GMI) — identidade mundial e reconhecível

> **Status:** decisão de arquitetura aprovada como especificação, implementação pendente. **ADR-034**. Data: 2026-11-15.  
> O GodMesh mantém sua identidade e serviços **nativos**; provedores OIDC, redes de rádio e outras plataformas são integrações compatíveis, não dependências obrigatórias para comunicação GodMesh↔GodMesh.

## Objetivo

Uma pessoa deve ter **uma identidade estável e mundialmente única**, reconhecida por todos os nós autorizados do GodMesh, qualquer que seja sua região, servidor, dispositivo, organização ou rede externa. A identidade não depende de número telefônico, e-mail, indicativo de radioamador, IP ou marca/modelo de rádio.

**Padrão de identidade interna, indicativo público e autenticação (ADR-034 e ADR-035):**

1. **ID permanente interno (GodMesh Global ID / `gmi_id`)**: UUIDv7 de 128 bits conforme IETF RFC 9562, representável como URN padronizada `urn:uuid:<uuid>` (imprevisibilidade e colisões devem seguir requisitos de geração segura). Pode ser criado offline sem coordenação global. Imutável, nunca reciclado. O UUIDv7 contém informação aproximada de tempo de criação; considerar pseudônimos/identificadores públicos por contexto se necessário para privacidade.
2. **Indicativo público nativo**: `gmc_callsign` no formato GodMesh, reservado globalmente e apresentado em mensagens/voz; **não contém identidade real**. Consulte [Indicativos digitais GMC](indicativos-digitais.md). Um `acct:apelido@dominio` (RFC 7565) pode ser oferecido **opcionalmente** para descoberta voluntária, não é obrigatório e não substitui o indicativo.
3. **Autenticação e comprovação**: OpenID Connect (identidade `iss` + `sub` da sessão autenticada), WebAuthn/passkeys, credenciais de workloads e domínios autorizados. **Não** tratar o handle, UUID ou DID como segredo ou prova suficiente de controle.

O perfil proposto de tipos de principal, credenciais, autorização, migração e revogação está em [Contrato de identidade e autorização GodMesh](../07-contratos/identidade-autorizacao.md). Os formatos de prova e a política de frescor ainda dependem de revisão e aprovação.

## Representação e identificação da plataforma

**Regra de privacidade posterior (ADR-035):** o indicativo digital GMC é o identificador usado por padrão na comunicação GodMesh. A publicação de `acct:` e perfis é opcional. O `gmi_id` é interno e não deve ser enviado ao destinatário como metadado público de voz/mensagem. Consulte também [Transporte da identidade em PTT](../01-arquitetura/identificacao-midia-mensagens.md).

A indicação visual de uma conta *GodMesh verificada* deve ser consequência de um **registro de federação autenticado** e não de um texto que qualquer servidor possa colocar no perfil.

- Cada domínio participante opera HTTPS e o recurso `/.well-known/webfinger` (RFC 7033), expondo relação para um URI de identidade e descritor de serviço GodMesh.
- Cada provedor federado anuncia capacidades GMP, versões, chaves/credenciais, políticas de confiança e endpoints em documento próprio **autenticado e versionado** (`/.well-known/godmesh`, convenção específica do produto; **não é padrão IETF/W3C**).
- Nós validam a autoridade do domínio e as credenciais de associação à federação (cadeia de confiança, assinaturas, revogação). Usar a palavra "GodMesh" no nome ou URI **não basta**.
- O endereço `acct:` é um **alias de descoberta opt-in**, não endpoint de mensagens por si só; WebFinger retorna metadados para encaminhar à identidade interna e protocolo GMP.

Exemplos conceituais (nenhum domínio/identificador abaixo está provisionado):

```text
ID permanente: urn:uuid:019a8cf1-1234-7abc-8def-123456789abc
Indicativo público: GM-7K4P-9R2Q-X (exemplo fictício)
Endereço opt-in: acct:ana@id.godmesh.example
Exibição nativa por padrão: GM-7K4P-9R2Q-X
Endereço federado opt-in: acct:ana@empresa.example
Perfil web: https://id.godmesh.example/u/019a8cf1-1234-7abc-8def-123456789abc
```

Consulta de descoberta:

```http
GET /.well-known/webfinger?resource=acct%3Aana%40id.godmesh.example HTTP/1.1
Host: id.godmesh.example
```

Representação WebFinger ilustrativa (as relações GodMesh abaixo são **extensões propostas**, não nomes registrados internacionalmente):

```json
{
  "subject": "acct:ana@id.godmesh.example",
  "links": [
    {
      "rel": "self",
      "type": "application/json",
      "href": "https://id.godmesh.example/u/019a8cf1-1234-7abc-8def-123456789abc"
    },
    {
      "rel": "https://spec.godmesh.example/rel/identity",
      "href": "https://id.godmesh.example/.well-known/godmesh"
    }
  ]
}
```

Os domínios nos exemplos devem ser substituídos por domínios efetivamente sob controle do operador antes da implantação. WebFinger público não deve revelar telefone, localização, lista de dispositivos, associações privadas nem perfil desnecessário.

## Estabilidade, migração e federação

- `gmi_id` permanece constante entre regiões, clientes, celulares, servidores e organizações. `home_authority` e endpoints de roteamento podem mudar.
- Apelidos/handles são únicos **dentro de seu domínio**, não globalmente por nome curto. Normalização, Unicode, caracteres semelhantes, enumeração e nomes reservados exigem política rigorosa. Handle opcional alterado deve manter redirecionamento assinado ou tombstone; evitar reciclar nomes para não facilitar impersonação.
- A migração entre provedores exige prova criptográfica, verificação do novo emissor, transferência de direitos e eventos de revogação. Caso provedor antigo esteja indisponível, seguir recuperação previamente definida, não confiar só em novo nome semelhante.
- `iss` + `sub` identifica conta no provedor OIDC (e **não** substitui o `gmi_id`), já que provedores podem mudar. Vincular emissores externos a identidade GodMesh após autenticação e prova de vínculo.
- Nos períodos offline, identidades locais podem ser geradas e operar localmente sob escopo restrito; a federação só as reconhece após validação de confiança e resolução de conflitos.
- Para auditoria, guardar histórico de alterações de handle, migração, vinculação e revogação sem expor tudo ao público.

## Identidades distintas por domínio

| Entidade | Identificador interno | Identificador externo/alias | Observação |
|---|---|---|---|
| Pessoa/usuário | `gmi_id` UUIDv7 | GMC público; `acct:` opcional | Um usuário pode participar de muitos tenants; nome civil não é exibido por padrão |
| Organização | `tenant_id` UUIDv7 | Domínio DNS validado, nome comercial | Domínio ≠ autorização automática |
| Dispositivo | `device_id` UUIDv7 | DevEUI, IMEI ou ID de fabricante quando aplicável | Prova de posse e chaves por dispositivo |
| Nó servidor | `node_id` UUIDv7 | Peer ID libp2p/chave pública | IP nunca é identidade |
| Canal | `channel_id` UUIDv7 | Slug / URI de convite | Escopo e ACL próprios |
| Rádio amador externo | Vinculado ao `gmi_id` | Indicativo de chamada, IDs de rede | Licenças/autorizações separadas |

Número telefônico em padrão E.164, e-mail, indicativo, DevEUI, IMEI e identificadores de terceiros podem ser usados como **atributos verificáveis opcionais**, jamais como chave primária de usuário. Nenhum desses identificadores autoriza sozinho acesso a contas, localização, dispositivos ou rádio.

## Autenticação, sessão e credenciais

- Usar OIDC/OAuth2. Para clientes e sessões, `iss` + `sub` é a chave confiável **dentro do emissor**, segundo OpenID Connect; e-mail ou `preferred_username` não são chaves estáveis.
- Priorizar passkeys/WebAuthn (Recomendação W3C WebAuthn Level 3, 25/08/2026), com múltiplos autenticadores e recuperação de conta auditável. MFA adicional para administradores, pontes de rádio e ações críticas.
- Identificadores públicos **não** devem embutir segredo. Chaves podem ser rotacionadas sem alterar o `gmi_id`; dispositivos e nós têm credenciais próprias.
- Avaliar suporte opcional a **DIDs (W3C DID Core v1.0)** para credenciais portáveis. Não adotar `did:web` como único identificador imutável, pois depende do domínio; métodos DID precisam avaliação de interoperabilidade e recuperação. Não exigir blockchain.
- Diferenciar conta humana, bot, organização, dispositivo e nó de federação, sem reutilizar o mesmo tipo de credencial ou política de emissão.

No esquema GMI abaixo, `subject_kind=person|service` descreve somente as identidades guardadas em `global_identities`. `service` cobre bot/workload. `tenant_id`, `device_id` e `node_id` continuam identificadores tipados em seus próprios registros, e podem atuar como principais de autorização somente com credencial e vínculo próprios. Não os modele como conta humana. O detalhamento e a matriz de autorização estão no contrato vinculado acima; as constraints necessárias para relações entre esses registros ainda precisam ser alinhadas ao modelo de dados.

## Persistência proposta (PostgreSQL)

```sql
CREATE TABLE global_identities (
  gmi_id UUID PRIMARY KEY,
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ('person', 'service')),
  home_authority TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'suspended', 'revoked')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE identity_handles (
  id UUID PRIMARY KEY,
  gmi_id UUID NOT NULL REFERENCES global_identities(gmi_id),
  normalized_handle TEXT NOT NULL,
  domain TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'redirect', 'reserved', 'revoked')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (domain, normalized_handle)
);

CREATE TABLE identity_providers (
  id UUID PRIMARY KEY,
  issuer_url TEXT NOT NULL UNIQUE,
  trust_status TEXT NOT NULL,
  last_verified_at TIMESTAMPTZ
);

CREATE TABLE identity_auth_links (
  id UUID PRIMARY KEY,
  gmi_id UUID NOT NULL REFERENCES global_identities(gmi_id),
  provider_id UUID NOT NULL REFERENCES identity_providers(id),
  subject TEXT NOT NULL,
  UNIQUE (provider_id, subject)
);
```

Esquema de referência, **não migração completa nem schema executável aprovado**: exige FKs/constraints adicionais para tenant e relações tipadas de `device_id`/`node_id`, política de unicidade de aliases, retenção, RLS por autoridade, índices, normalização IDNA/domínio e event log com revogações. Se o serviço de autenticação for externo, a autoridade deve manter o vínculo de `iss` + `sub` e definir o cache de claims. A enumeração `person|service` não classifica tenants, dispositivos ou nós. O contrato de identidade descreve a proposta de mapeamento; implementação e RLS não foram validadas.

## API e eventos propostos

```text
GET  /.well-known/webfinger?resource=acct%3Aana%40id.godmesh.example
GET  /.well-known/godmesh
GET  /v1/identity/me
GET  /v1/identity/resolve?address=acct%3Aana%40id.godmesh.example
POST /v1/identity/handles
POST /v1/identity/migration/initiate
POST /v1/identity/migration/confirm
```

Eventos internos: `identity.created`, `identity.handle.changed`, `identity.provider.linked`, `identity.migration.requested`, `identity.migration.completed`, `identity.revoked`. Sempre com escopo, assinatura/verificação de origem, cursor, idempotência e proteção contra replay.

## Testes de conformidade

1. Dois servidores separados criam IDs UUIDv7 sem coordenação, não confundindo identidades e sem duplicação no banco.
2. Usuário acessa em múltiplas regiões mantendo o mesmo `gmi_id` e controles de permissão.
3. WebFinger resolve aliases de domínios autorizados; domínio desconhecido não recebe badge verificado.
4. Troca de handle ou região não altera `gmi_id`; tombstones impedem apropriação indevida de endereços antigos.
5. Rotação de credenciais, recuperação e migração auditável preservam histórico e revogações.
6. `iss` + `sub` vincula autenticação ao usuário correto; e-mail semelhante não basta.
7. Interrupção regional não permite que nós não autorizados assumam identidades; políticas offline mostram informação potencialmente desatualizada.
8. Ao remover todas as bridges externas, identidade, descoberta e PTT nativos continuam operando.

## Fontes normativas

- IETF RFC 9562 (UUIDv7 e namespace `urn:uuid`): https://www.rfc-editor.org/rfc/rfc9562.html
- IETF RFC 7565 (`acct:`): https://www.rfc-editor.org/rfc/rfc7565.html
- IETF RFC 7033 (WebFinger): https://www.rfc-editor.org/rfc/rfc7033.html
- W3C DID Core v1.0 (Recommendation): https://www.w3.org/TR/did-core/
- W3C WebAuthn Level 3 (Recommendation, 2026): https://www.w3.org/TR/webauthn-3/
- OpenID Connect Core — claims `iss` e `sub`: https://openid.net/specs/openid-connect-core-1_0.html
