# Contrato de identidade e autorização GodMesh

> **Perfil documental v1, proposta para revisão.** Este documento detalha GMI, GMC, ingresso e autorização sem declarar implementação concluída nem alterar o status das ADR-034, ADR-035 e ADR-036. As escolhas marcadas como proposta precisam de revisão de segurança, alinhamento com o contrato GMP v1 e aprovação das políticas que dependem do produto.

## Escopo e autoridades

Este contrato define como os serviços GodMesh reconhecem um principal, avaliam uma permissão e validam credenciais GMI/GMC. O contrato de API GMP define os envelopes, caminhos e transporte. O contrato de eventos e sincronização define IDs de evento, idempotência, cursores, replay e inbox. A autoridade de escrita define home region, epoch e transferência. A política de retenção define cópias e efeitos de revogação. Enquanto esses documentos transversais estão em elaboração, as referências abaixo são dependências de integração, não contratos já publicados.

O serviço que protege o recurso executa a decisão de autorização. Uma origem autoriza sua saída, e o serviço de destino repete a verificação antes de aceitar dados federados ou uma tradução de bridge. Ter uma conta, um indicativo, um domínio ou uma chave válida, isoladamente, não concede acesso.

| Autoridade | Responsabilidade | Não pode delegar automaticamente |
|---|---|---|
| Autoridade de identidade | Liga `iss` + `sub` autenticados a `gmi_id`; emite e revoga credenciais de identidade | Permissão de tenant, canal ou localização |
| Autoridade do tenant | Mantém membros, papéis e delegações daquele tenant | Acesso a outro tenant ou à federação inteira |
| Proprietário do recurso ou controlador de política | Define ACL, finalidade, destinatários e consentimento do recurso | Prova de identidade do solicitante |
| Autoridade da federação | Emite, limita e revoga certificados de ingresso de nós | Escopos fora do certificado, nem acesso por mera descoberta |
| Serviço de autorização | Combina identidade, credencial, escopo, vínculo, estado e política no momento do acesso | Decisão baseada apenas em `tenant_id` fornecido pelo cliente |

## Tipos de identidade e de principal

`gmi_id` identifica uma pessoa GodMesh e permanece interno. Uma identidade de serviço pode representar um bot ou workload. Os demais identificadores permanecem tipos próprios. Não converta um dispositivo ou nó em pessoa para satisfazer uma ACL.

| Principal de autorização | Identificador | Credencial esperada | Relação com GMI |
|---|---|---|---|
| Pessoa | `gmi_id` UUIDv7 | Sessão autenticada OIDC e credencial de dispositivo/chave conforme operação | É a identidade humana GMI |
| Serviço ou bot | `service_id` UUIDv7 | Credencial de workload vinculada a chave e audiência | Identidade de workload, não login humano |
| Organização/tenant | `tenant_id` UUIDv7 | Delegação assinada por operador autorizado | Contexto e principal de administração; não é `gmi_id` |
| Dispositivo | `device_id` UUIDv7 | Chave própria e prova de associação/posse | Entidade vinculada a usuário/tenant, não `gmi_id` |
| Nó federado | `node_id` UUIDv7 | Chave persistente e certificado de ingresso escopado | Entidade federada, não `gmi_id` |
| Canal/recurso | `channel_id` ou ID tipado | ACL e concessão do recurso | Contexto do recurso, não prova do chamador |

No esquema ilustrativo `global_identities.subject_kind`, `person` e `service` classificam as identidades GMI armazenadas nessa tabela. `service` inclui bot/workload. Organizações, dispositivos e nós continuam em seus registros com `tenant_id`, `device_id` e `node_id`; suas credenciais e permissões não devem reutilizar a política de emissão humana. Esta classificação alinha o escopo do esquema GMI; o modelo de dados ainda precisa refletir as relações e constraints correspondentes.

Um principal autenticado tem a forma lógica abaixo. `tenant_context` vem da associação validada no servidor, nunca de um campo de requisição sem verificação.

```json
{
  "kind": "person",
  "id": "urn:uuid:019a8cf1-1234-7abc-8def-123456789abc",
  "tenant_context": "tenant-uuid",
  "authn_context": "passkey",
  "credential_ref": "cred-opaque-reference"
}
```

O exemplo é ilustrativo. O protocolo GMP v1 define a codificação no fio.

## Autenticação e credenciais

### Conta humana e OIDC

1. O cliente autentica a pessoa no emissor OIDC permitido.
2. O cliente ou edge valida o ID Token para autenticação, incluindo emissor, audiência, validade e nonce conforme OIDC Core.
3. O serviço GodMesh mapeia o par sensível a maiúsculas `(iss, sub)` a exatamente um `gmi_id` sob autoridade local. E-mail, `preferred_username`, `acct:`, GMC, UUID informado pelo chamador ou domínio DNS não criam esse vínculo.
4. O autorizador emite ou valida um token de acesso destinado ao serviço GMP requerido. Um ID Token não é um token de acesso à API.
5. O serviço de recurso valida issuer, audience, expiração, algoritmo/chave e scope antes de avaliar tenant e ACL.

O formato de token de acesso permanece a cargo do contrato GMP v1. JWTs de acesso podem seguir RFC 9068; tokens opacos com introspecção são uma alternativa. A escolha, algoritmos e cache do contrato de API precisam concordar. Não aceite token emitido para outro audience. Claims de perfil do provedor não se transformam em papéis ou scopes GodMesh sem mapeamento aprovado.

### Credencial assinada GMC

A credencial GMC afirma que uma autoridade confiável reservou um indicativo e o vinculou a uma chave pública. Ela não contém nome civil, `gmi_id`, e-mail, telefone ou localização. A autoridade mantém o vínculo privado entre indicativo, GMI e estado do titular.

O perfil `gmp.callsign-credential.v1` abaixo é uma **proposta técnica**, não parte aprovada da ADR-035:

| Campo | Tipo | Regra proposta |
|---|---|---|
| `typ` | string | Valor fixo `gmp.callsign-credential.v1`; separa a credencial GMC de tokens OIDC/API |
| `issuer` | HTTPS URI | Emissor presente no catálogo de confiança e com chave válida |
| `credential_id` | UUID | Identificador único da emissão, sem reutilização |
| `callsign` | string | GMC reservado pelo registro autorizado |
| `key_id` | string | Referência a uma chave pública do emissor no manifesto autenticado |
| `holder_key_thumbprint` | string | Impressão digital JWK da chave do titular; não é segredo |
| `audience` | URI/string | Serviço ou domínio GMP que pode aceitar esta credencial |
| `scope` | lista de strings | Permissões limitadas concedidas pela autoridade; nunca substitui ACL |
| `issued_at`, `not_before`, `expires_at` | timestamps UTC | Validade explícita. Cada emissão precisa expirar |
| `authority_epoch` | inteiro sem sinal | Geração da autoridade para rejeitar grants de líder obsoleto |
| `signature` | JWS | Assinatura segundo chave e algoritmo permitidos pelo perfil implantado |

**Proposta:** usar JWS Compact Serialization, com tipo protegido fixo, `kid` resolvido apenas dentro do manifesto de confiança configurado e conjunto de algoritmos fixado por política local. Rejeitar `alg=none`, algoritmo não permitido, uso cruzado entre token API e credencial GMC, chave desconhecida e chave fora de validade. Não confiar em `jwk`, `jku` ou URI de chave fornecida pelo token sem validação contra a autoridade configurada. O perfil recomenda começar com `ES256` e `EdDSA` por chave, mas a lista final requer revisão de segurança e compatibilidade. Alternativa: COSE para mensagens CBOR mais compactas; isso exigiria outra codificação e negociação. JWS foi escolhido como proposta por coerência com o ecossistema OIDC/JSON já especificado, não como decisão aprovada nem prova de suporte de bibliotecas.

```json
{
  "typ": "gmp.callsign-credential.v1",
  "issuer": "https://issuer.example.invalid",
  "credential_id": "019a0000-0000-7000-8000-000000000002",
  "callsign": "GM-7K4P-9R2Q-X",
  "key_id": "issuer-key-2026-01",
  "holder_key_thumbprint": "jwk-thumbprint-example",
  "audience": "https://node.example.invalid/gmp",
  "scope": ["identity.callsign.present", "ptt.transmit"],
  "issued_at": "2026-11-15T20:00:00Z",
  "not_before": "2026-11-15T20:00:00Z",
  "expires_at": "2026-11-15T20:15:00Z",
  "authority_epoch": 12,
  "signature": "<JWS da autoridade, exemplo não válido>"
}
```

O horário de 15 minutos é **apenas exemplo**, não TTL aprovado. A política precisa definir duração máxima por classe de credencial antes da prontidão.

### Prova de posse

Para HTTP, a proposta é usar token sender-constrained com DPoP em clientes públicos e avaliar mTLS para autenticação serviço-a-serviço. DPoP demonstra posse de chave vinculada ao token, mas não autentica a pessoa nem decide ACL. Cada prova deve vincular método, URI, token e identificador único; o receptor mantém janela de replay limitada. Essa extensão HTTP não se aplica diretamente a frames de áudio.

Para `TalkStart`, o controlador de canal verifica a credencial GMC, o vínculo da chave do emissor, o `channel_id`, `session_id`, `grant_id`, `epoch`, `stream_id` e a prova de posse. O `TalkStart` autenticado leva apenas GMC ou alias autorizado e referências efêmeras. Nunca envia `gmi_id` em frames públicos. O controlador rejeita indicação textual sem prova, grant expirado, canal diferente e epoch antigo.

### Chaves, rotação e revogação

- A autoridade publica `kid`, algoritmo, chave pública, finalidade, validade e geração em manifesto autenticado e versionado.
- A chave privada nunca aparece em manifesto, token, log ou exportação pública. Armazenamento protegido, HSM, custódia e controles operacionais dependem de política/implantação e ainda não foram escolhidos.
- Rotação publica a nova chave antes de seu uso. Chaves antiga e nova podem coexistir apenas durante janela aprovada. Expirar a chave antiga impede nova emissão, mas não prolonga credenciais emitidas.
- A autoridade mantém um estado de credencial `active`, `suspended` ou `revoked` e um epoch monotônico por escopo. Revogar invalida novos grants, novas emissões e renovações. Invalida sessões/conexões conforme contrato do serviço e política de retenção.
- O RFC 7009 descreve revogação OAuth e reconhece que a propagação pode atrasar entre servidores. O contrato local não presume propagação instantânea.
- Uma credencial com status `unknown` ou `stale` não autoriza ingresso, emissão de grant PTT, acesso a localização precisa, recuperação/migração ou replicação de dados privados.
- Para ações locais já autorizadas, uma proposta de operação offline permite somente grant assinado ainda válido, com escopo mínimo e epoch conhecido. Após expiração ou limite de frescor, o serviço falha fechado para ação federada. Sem limite configurado para a classe, a prontidão de produção fica bloqueada.

Os limites `max_credential_lifetime`, `max_revocation_staleness` e `max_offline_grant_lifetime` são parâmetros obrigatórios por classe. Nenhum valor numérico está aprovado. Alternativa estrita consulta a autoridade em toda ação, com menor janela de estado obsoleto e pior disponibilidade em partição. Alternativa de grant offline assinado preserva operações restritas, com exposição limitada pela duração aprovada. Recomenda-se a alternativa limitada por grant e epoch para ações locais reversíveis; decisões de federação, alteração de ACL, migração, localização precisa e PTT externo exigem estado atual e autoridade consistente. Usuário e autoridades de produto/segurança devem aprovar os limites e a lista final de operações.

## Identidade por canal e aliases contextuais

ADR-035 mantém GMC estável como identificador público padrão nas comunicações. Como proposta adicional, um canal pode selecionar `identity_mode=stable_callsign` ou `identity_mode=contextual_alias` antes de admitir membros. A primeira opção permanece o padrão enquanto política de produto não aprovar outro valor.

| Ator | `stable_callsign` | `contextual_alias` |
|---|---|---|
| Membros do canal | Veem o GMC validado | Veem alias único naquele vínculo de membro |
| Usuário titular | Vê seu GMC e o alias apresentado | Vê o alias ativo e a audiência que o recebe |
| Administrador do canal | Vê GMC e estado de validação conforme ACL | Vê alias; não recebe o mapa para GMI por padrão |
| Serviço de identidade | Mantém vínculo privado para abuso e revogação | Mantém vínculo protegido alias↔GMI, inacessível à consulta comum |
| Outros canais e usuários públicos | Não recebem dados de canal privado | Não recebem alias, GMC privado nem vínculo |

O alias é aleatório e separado do `gmi_id`/GMC. Ele permanece estável apenas dentro de um vínculo de canal ativo, para manter atribuição conversacional; novo vínculo em outro canal recebe novo alias. Esta duração é uma **proposta**, pois ADR-035 apenas pede alias temporário. O canal informa o modo antes de entrada; um canal contextual não faz fallback para GMC sem nova confirmação do titular. Nenhum modo promete anonimato contra voz, conteúdo, horários ou correlação física.

Resolver alias para titular exige scope `identity.alias.resolve`, caso de abuso/incidente, papel separado de administrador comum, justificativa e auditoria. O resultado limita-se ao incidente e destinatários autorizados. O registro dessa resolução segue a retenção ainda pendente. Sem autoridade e finalidade aprovadas, o recurso de alias contextual não fica pronto para produção.

## Decisão de autorização

O autorizador aplica as verificações nesta ordem e nega se uma delas falhar:

1. Validar versão, formato, tamanho e audience da requisição.
2. Autenticar principal, prova de posse e cadeia do emissor. Recusar credencial expirada, revogada, desconhecida ou fora do escopo.
3. Resolver principal tipado e vínculo de identidade sem converter dados do cliente em confiança.
4. Derivar tenant e recurso do contexto autorizado no servidor. Validar vínculo de membership, proprietário e ACL.
5. Exigir scope mínimo para ação e escopo do recurso; ignorar scope que não veio de emissor confiável.
6. Verificar consentimento e finalidade específicos, incluindo janela temporal e destinatário para localização/Guardian/SOS.
7. Verificar status atual, epoch e regra de partição para a classe de ação.
8. Para bridge/federação, verificar política de saída na origem e política de entrada no destino; transformar apenas atributos suportados pelo destino, com proveniência UDNL.
9. Emitir resultado mínimo, registrar auditoria sem token, chave, coordenadas ou conteúdo protegido.

`tenant_id` do body, URL ou header é apenas seletor de recurso. O servidor compara-o com o principal e com a membership. Nenhuma igualdade textual autoriza leitura. RBAC pode agrupar permissões; ABAC ainda verifica finalidade, região, escopo de dados e atributos do recurso. PostgreSQL RLS é defesa complementar, não a única verificação de autorização.

### Scopes propostos

São nomes candidatos de contrato, ainda não registrados nem aprovados. O contrato GMP v1 deve fixar o vocabulário final e a semântica:

| Scope candidato | Permissão limitada |
|---|---|
| `identity.self.read` | Ler a projeção da própria sessão |
| `identity.alias.resolve` | Resolver um alias para o titular em um caso autorizado e auditado |
| `identity.handle.write` | Criar ou alterar alias próprio sob autoridade do domínio |
| `identity.callsign.claim` | Solicitar GMC para identidade autenticada; não garante concessão |
| `identity.migrate` | Iniciar/confirmar migração em fluxo de prova adicional |
| `federation.join.request` | Pedir ingresso para o `node_id` autenticado |
| `federation.node.attest` | Emitir certificado de ingresso, restrito à autoridade federativa |
| `federation.peer.read` | Ler a lista de peers visível a esse nó/escopo |
| `ptt.transmit` | Solicitar grant, ainda condicionado ao canal e estado vigente |
| `guardian.beacon.emit` | Obter token de emissão para sessão e consentimento ativos |
| `crowdfind.observe` | Enviar observação como observador autenticado e participante opt-in |
| `ops.health.read` | Ler readiness detalhada permitida a operador |
| `route.region.discover` | Descobrir regiões elegíveis para o principal/tenant |
| `asset.read`, `asset.write` | Ler/criar ou alterar ativos do tenant autenticado |
| `device.read`, `device.enroll`, `device.command` | Ler, associar por prova de posse ou comandar dispositivo vinculado |
| `location.read`, `location.history.read` | Ler localização conforme precisão, grant, finalidade e retenção |
| `route.resolve` | Pedir resolução de rota; não força o destino a aceitar tráfego |
| `channel.read`, `channel.manage`, `media.session.create` | Ler ACL, administrar canal como delegado e abrir sessão permitida |
| `bridge.read`, `bridge.manage`, `external.service.read` | Ler ou configurar somente integrações do tenant e do operador |
| `events.read` | Consumir stream e replay apenas para scopes autorizados |

Scopes não têm wildcards implícitos. `admin` ou `identity.*` não são aceitos como substitutos do scope específico.

## Matriz de autorização dos endpoints atuais

Esta matriz cobre apenas operações propostas nos documentos atuais. Graydon Hoare mantém a proposta de contrato de wire e paths em [GMP API v1](gmp-api-v1.md). Os caminhos, erros e nomes de scope ainda devem ser reconciliados entre os contratos; a existência do arquivo não comprova essa integração ou aprovação. Esta matriz não cria outra definição de transporte.

| Operação/path lógico | Principal e scope | Checagens adicionais e autoridade | Erros de autorização |
|---|---|---|---|
| `GET /.well-known/webfinger` | Público | Domínio serve apenas aliases publicados opt-in; limita enumeração; resposta não comprova identidade | `400 invalid_request`, `404 not_found`, `429 rate_limited`, `503 authority_unavailable` |
| `GET /.well-known/godmesh` | Público | Documento de domínio assinado; consumidor valida autoridade pelo catálogo de confiança, não pelo nome | `404 not_found`, `503 trust_state_unavailable` |
| `GET /v1/identity/me` | Pessoa/service autenticado; `identity.self.read` | Retorna somente a projeção permitida ao próprio sujeito; deriva tenant no servidor | `401 invalid_token`, `403 insufficient_scope`, `503 authority_unavailable` |
| `GET /v1/identity/resolve` | Público para `acct:` opt-in | Escopo limitado ao alias público; jamais retorna perfil privado, GMI, dispositivos ou tenant oculto | `400 invalid_request`, `404 not_found_or_not_visible`, `429 rate_limited` |
| `POST /v1/identity/handles` | Titular; `identity.handle.write` | Prova de posse; normalização e unicidade sob autoridade do domínio; confirmação antes de trocar handle | `401 invalid_token`, `403 not_owner`, `409 alias_unavailable`, `422 invalid_handle` |
| `POST /v1/identity/callsigns/claim` | Titular; `identity.callsign.claim` | Prova de posse GMI/chave; autoridade de emissão, unicidade e estado do registro; não declarar global enquanto indisponível | `401 invalid_token`, `403 not_owner`, `409 callsign_conflict`, `503 registry_unavailable` |
| `GET /v1/callsigns/{callsign}/credential` | Público para credencial publicada | Só campos públicos GMC; sem GMI ou chave privada; limite anti-enumeração | `404 not_found_or_not_visible`, `429 rate_limited`, `503 issuer_unavailable` |
| `POST /v1/identity/migration/initiate` | Titular; `identity.migrate` | Prova da credencial atual e do novo emissor; não confiar em nome semelhante; cria solicitação não efetiva | `401 invalid_token`, `403 not_owner`, `409 migration_pending`, `503 authority_unavailable` |
| `POST /v1/identity/migration/confirm` | Titular antigo e novo emissor, com prova adicional | Verifica nonce, prazo configurado, epoch, disputa e revogação; recuperação excepcional segue decisão pendente | `401 invalid_proof`, `403 migration_not_authorized`, `409 contested`, `503 authority_unavailable` |
| `POST /v1/federation/join-requests` | Principal `node`; `federation.join.request` | Prova de chave persistente, convite/atestado válido, tenant scopes explícitos e política do destino; nó novo não recebe dados privados antes da aprovação | `401 invalid_proof`, `403 invitation_invalid`, `409 node_conflict`, `503 trust_state_unavailable` |
| `POST /v1/federation/attest` | Autoridade `service`; `federation.node.attest` | Emissor federativo autorizado, lease/epoch atual, scopes mínimos; não aceita autoaprovação do nó | `401 invalid_token`, `403 issuer_not_authorized`, `409 stale_epoch`, `503 authority_unavailable` |
| `GET /v1/federation/peers` | Nó autorizado; `federation.peer.read` | Lista filtrada por região, cluster, tenant e relações de trust do solicitante | `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible` |
| `GET /v1/federation/manifests/current` | Público só se manifesto não contiver dados privados; caso contrário, nó confiável com `federation.peer.read` | Assinatura, versão, audience e escopo; manifesto nunca concede membership por si só | `404 not_found_or_not_visible`, `409 unsupported_version`, `503 trust_state_unavailable` |
| `POST /v1/federation/leave` | Nó titular com prova de posse; `federation.join.request` ou scope específico a definir | Revoga somente seu próprio vínculo, remove novas sessões e emite evento sob autoridade C10/C06; não revoga outros peers | `401 invalid_proof`, `403 not_owner`, `409 stale_epoch`, `503 authority_unavailable` |
| `TalkStart`/solicitação de grant PTT | Pessoa/device autenticado; `ptt.transmit` | Credencial GMC e PoP, membership/ACL do canal, grant vigente, `epoch`, sem alias/GMI inventado | `unauthenticated`, `identity_unverified`, `grant_expired`, `channel_denied`, `stale_epoch` |

Os códigos são semântica de autorização proposta. O contrato GMP decide o mapeamento para HTTP, envelope GMP, redaction e compatibilidade de versões. Para recursos privados, `404 not_found_or_not_visible` evita revelar existência a quem não pode ler. Um conflito de unicidade não informa quem detém o identificador.

### Outros endpoints da API inicial

O contrato GMP v1 deve consolidar estas linhas junto ao método/path final. Os scopes são candidatos coerentes com a tabela acima; não os considere nomes publicados até Graydon os integrar ao vocabulário de API.

| Método/path atual | Principal e scope candidato | Tenant, ACL e prova adicional | Autoridade que decide | Negação esperada |
|---|---|---|---|---|
| `GET /v1/health/live` | Público, resposta mínima de liveness | Sem hostname, versão sensível, dependências ou segredo | Edge do serviço | `503 unavailable` sem detalhe interno |
| `GET /v1/health/ready` | Operador autenticado; `ops.health.read` | Operador restrito ao cluster/região | Operação do serviço | `401`, `403`, `503` |
| `GET /v1/regions` | Principal autenticado; `route.region.discover` | Só regiões elegíveis à conta/tenant, política e capacidades atuais | Serviço de roteamento | `401`, `403`, `404 not_visible` |
| `GET /v1/assets` | Membro; `asset.read` | `tenant_id` da sessão; membership e ACL do ativo | Serviço de ativos | `401`, `403`, `404 not_visible` |
| `POST /v1/assets` | Membro delegado; `asset.write` | Tenant do servidor; tenant body não concede acesso | Autoridade do tenant | `401`, `403`, `409`, `422` |
| `GET /v1/devices` | Membro; `device.read` | Dispositivo ligado ao tenant/ativo; ocultar outros tenants | Serviço de dispositivos | `401`, `403`, `404 not_visible` |
| `POST /v1/devices` | Membro/delegado; `device.enroll` | Prova de posse da chave; convite/delegação do tenant quando não é autoinscrição | Autoridade de associação | `401`, `403`, `409`, `422` |
| `GET /v1/devices/{id}/capabilities` | Membro; `device.read` | Verificar vínculo do dispositivo e visibilidade da capability | Serviço de dispositivos | `401`, `403`, `404 not_visible` |
| `GET /v1/devices/{id}/location` | Destinatário autorizado; `location.read` | Grant de localização ativo; coarse/precise conforme consentimento, finalidade e audiência | Serviço de localização | `401`, `403`, `404 not_visible`, `503 stale` |
| `GET /v1/devices/{id}/observations` | Destinatário autorizado; `location.history.read` | Grant inclui histórico e período; política de retenção/exclusão aplicada | Serviço de localização/dados | `401`, `403`, `404 not_visible`, `503 authority_unavailable` |
| `POST /v1/devices/{id}/commands` | Membro/delegado; `device.command` | ACL do device, prova de posse quando exigida, expiração e chave de idempotência; o ACK não prova execução final | Serviço de comandos e destino valida outra vez | `401`, `403`, `404 not_visible`, `409`, `422` |
| `POST /v1/routes/resolve` | Principal; `route.resolve` | Validar capabilities, ACL, consentimento e elegibilidade de origem/destino; não aceita rota escolhida pelo cliente como autorização | Origem avalia saída e destino pode negar entrada | `401`, `403`, `404 no_route`, `503` |
| `GET /v1/channels` | Membro; `channel.read` | Só listar canais visíveis à membership | Serviço de canais | `401`, `403`, `404 not_visible` |
| `POST /v1/channels` | Membro delegado; `channel.manage` | Papel de criação no tenant; política de canal e modo de identidade aprovados | Autoridade de tenant/canal | `401`, `403`, `409`, `422` |
| `POST /v1/channels/{id}/sessions` | Membro; `media.session.create` | ACL do canal, consentimento de participantes conforme política e região/capacidade atual | Controlador de canal/mídia | `401`, `403`, `404 not_visible`, `409`, `503` |
| `POST /v1/channels/{id}/floor` | Participante; `ptt.transmit` | GMC/alias validado, grant, estado da sessão, epoch e ACL; não herda permissão de ouvir | Controlador do canal | `401`, `403`, `404 not_visible`, `409 stale_epoch` |
| `GET /v1/bridges` | Operador do tenant; `bridge.read` | Retorna somente bridges autorizadas àquele tenant | Administração de bridge | `401`, `403`, `404 not_visible` |
| `POST /v1/bridges` | Operador delegado; `bridge.manage` | Verificar políticas RF/externas, ambos os endpoints e credenciais isoladas; habilitar TX exige grant separado | Autoridade do tenant e bridge/destino | `401`, `403`, `409`, `422`, `503` |
| `GET /v1/radio/services` | Operador autorizado; `external.service.read` | Lista serviços externos explicitamente autorizados, sem expor segredos ou serviços de outro tenant | Serviço de integração | `401`, `403`, `404 not_visible` |
| `GET /v1/events/stream` | Principal; `events.read` | Filtro de tenant/stream/cursor aplicado no servidor; não aceita filtro como ACL; replay segue C10 | Serviço de eventos e destino | `401`, `403`, `409 cursor_invalid`, `503` |

Scopes definidos pelo usuário, `tenant_id` selecionado pelo cliente e prova de posse não substituem ACL do recurso. A autorização de saída e a autorização de entrada são distintas para rota federada ou bridge. Retenção/expurgo da resposta de localização depende da política C12; até C12 estabelecer regras por cópia, a API não pode prometer exclusão geral.

## Falhas, recuperação e migração

### Estado de validação

Os estados `active`, `suspended` e `revoked` pertencem à credencial. O resultado da verificação pode ser `valid`, `expired`, `revoked`, `unknown`, `stale` ou `key_unknown`. `unknown` significa que a autoridade não confirmou o estado; `stale` significa que a prova local ultrapassou a idade máxima configurada. São estados de validação, não uma máquina de estados de eventos concorrente.

| Resultado | Nova leitura não sensível | Novo grant PTT | Ingresso/sync de dado privado | Localização precisa/migração |
|---|---:|---:|---:|---:|
| `valid` | Avalia ACL | Avalia grant/epoch | Avalia autorização de federação | Avalia consentimento e papel |
| `expired` ou `revoked` | Nega, exceto recurso público | Nega | Nega e aplica isolamento | Nega |
| `unknown` ou `stale` | Nega se privado; pode servir cache público sem alegar freshness | Nega | Nega | Nega |
| `key_unknown` | Nega | Nega | Nega | Nega |

Operações locais em cache ficam limitadas aos grants assinados, ainda válidos, com epoch conhecido. A política de dados define se conteúdo já recebido fica acessível. Isso não permite obter nova réplica, ampliar ACL ou alegar revogação propagada. Uma reconexão consulta a autoridade, aplica eventos de revogação idempotentemente, atualiza epoch e mantém fencing; eventos/cursors/replay seguem o contrato C10 de sincronização.

### Proposta de recuperação

Migração normal exige prova de uma chave atual e confirmação do novo emissor antes de transferir autoridade. O endpoint inicia uma solicitação; não muda `home_authority` por si só. A mudança efetiva requer autoridade de escrita consistente, epoch crescente, emissão do novo vínculo e revogação do anterior.

Para emissor indisponível, recomenda-se recuperação com ao menos duas classes independentes de prova previamente registradas, notificação aos canais de recuperação e um período configurável de contestação. Durante `recovery_pending`, bloquear troca de chave, handles e operações administrativas sensíveis. Uma prova apenas por nome, e-mail semelhante, domínio novo ou acesso ao canal não basta. Esta regra é proposta; quantidade de provas, duração, equipe autorizada, contestação, impacto de privacidade e recuperação de pessoas sem fator alternativo aguardam decisão do usuário. Sem o procedimento aprovado, não habilitar recuperação excepcional em produção.

## Erros e resposta

| Código lógico | Condição | Efeito |
|---|---|---|
| `invalid_token` | Token ausente, expirado, assinatura inválida, issuer/audience incompatível | Não executa ação; cliente pode autenticar novamente |
| `invalid_proof` | PoP, nonce, vínculo de chave ou assinatura inválida | Nega; não revela se outro sujeito existe |
| `insufficient_scope` | Scope válido não cobre ação | Nega sem ampliar token |
| `not_owner` | Principal não tem vínculo exigido com sujeito/recurso | Nega; não revela titular |
| `not_found_or_not_visible` | Recurso ausente ou oculto ao principal | Mesma resposta para impedir enumeração |
| `conflict` | Handle/GMC já reservado, migração disputada ou epoch concorrente | Não transfere posse; informa apenas o conflito mínimo |
| `invalid_request` | Campo ausente, tipo inválido, versão não suportada | Rejeita antes de resolver identidade |
| `rate_limited` | Limite configurado excedido | Resposta recuperável com `Retry-After` quando aplicável |
| `authority_unavailable` | Trust, revogação, membership ou chave não pode ser validada | Falha fechada em operação privada/sensível; não faz downgrade |

O contrato de API GMP define status de transporte, `request_id`, `retry_after`, limites de tamanho e envelope final. Erros não incluem token, chave, mapeamento de alias, coordenada nem causa privada.

## Exemplos de decisão

**Permitido.** Pessoa com sessão válida e `identity.self.read` lê sua própria projeção GMI. A resposta não contém claims do tenant que não sejam necessários.

**Negado.** Um cliente envia `tenant_id` de outra organização no body, embora o token tenha scope de leitura. O backend compara a membership autenticada e nega.

**Negado.** Um usuário conhece o GMC de outra pessoa e o apresenta como identidade em `TalkStart`, sem credencial válida e PoP vinculado ao canal. O controlador não concede o grant.

**Negado.** Um nó recém-descoberto apresenta peer ID e endereço IP, mas não possui convite/atestado autorizado. O nó permanece isolado e não recebe snapshot privado.

**Negado.** Uma identidade com credencial revogada continua no cache de um nó sem informação atualizada. O validador classifica estado `unknown` ou `stale`, impede novas operações federadas e não declara que o nó está em dia.

**Proposta, sujeito a aprovação.** Um canal definido como `contextual_alias` apresenta alias só aos membros. Uma consulta de alias por moderador comum retorna `403`; uma equipe antiabuso autorizada registra finalidade e caso antes de resolver.

## Casos de conformidade documental

Estes casos são critérios para testes futuros. Nenhum foi executado nesta tarefa.

1. Mesmo `(iss, sub)` autenticado volta ao mesmo `gmi_id`; o mesmo `sub` sob outro issuer não herda o vínculo.
2. E-mail, handle, GMC, `tenant_id` do body e ID UUID informado não autenticam uma pessoa.
3. ID Token com audiência de cliente não funciona como token de acesso GMP para outro audience.
4. Credencial GMC adulterada, `alg=none`, `kid` desconhecido, issuer fora do catálogo, assinatura inválida, claims duplicados, expiração passada ou scope fora da política são rejeitados.
5. Prova DPoP repetida, método/URI divergente, token hash divergente ou chave diferente do vínculo do token são rejeitados.
6. Operações cross-tenant, recurso revogado, dispositivo sem vínculo e principal workload usando scope humano são negados.
7. Uma credencial válida com scope correto ainda falha se membership, finalidade, consentimento ou ACL do recurso não permitir a ação.
8. Nó revogado ou estado de confiança desconhecido não recebe nova replicação, certificado ou capacidade pronta.
9. Partição impede ações sensíveis sem authority freshness; grant local ainda válido não aumenta escopo e expira no limite configurado.
10. GMC estável permanece o padrão; alias contextual não atravessa canal, não cai para GMC sem confirmação e não é resolvido por administrador não autorizado.
11. Uma bridge sem autorização de saída ou sem autorização de entrada nega a projeção e preserva identidade/proveniência UDNL.
12. A recuperação excepcional rejeita nome semelhante como única prova e produz estado de contestação auditável quando houver prova conflitante.

## Decisões e bloqueios de prontidão

| Decisão | Recomendação | Estado/impacto |
|---|---|---|
| JWS ou COSE para credencial GMC | JWS como proposta por coerência com OIDC/JSON; avaliar tamanho e suporte em cada cliente | Usuário/segurança/API precisam aprovar; interop não validada |
| Algoritmos de assinatura | Perfil allowlist com ES256 e EdDSA por chave; sem `none` ou seleção livre pelo token | Revisar compatibilidade de bibliotecas, chaves e clientes; sem homologação |
| Sender constraint | DPoP para clientes públicos; avaliar mTLS para workloads | API e segurança precisam fechar transporte e key binding |
| Cache/revogação offline | Grant assinado com epoch e limites por classe; negar funções sensíveis sem freshness | Valores máximos, SLO e modo de partição aguardam decisão |
| Recuperação sem emissor antigo | Duas classes independentes de prova registrada, aviso, contestação e auditoria | Política/autoridade/duração aguardam decisão |
| Alias contextual | GMC estável por padrão; alias aleatório por vínculo de canal para canais opt-in | Visibilidade, retenção do mapa e acesso excepcional aguardam aprovação |
| Registro global do GMC | Sem declarar unicidade durante indisponibilidade; manter ID local provisório | Autoridade, quorum e fencing dependem de `autoridade-escrita.md` |

Produção fica bloqueada até que a autoridade de escrita, a API GMP, os eventos/sync e a política de retenção definam seus contratos e valores. Nenhum país, licença, mecanismo legal, SLO, RPO, RTO, prazo de retenção, presença de HSM ou suporte de hardware está aprovado neste documento.

## Referências oficiais consultadas

Consulta em 15/11/2026. Os padrões definem peças interoperáveis; não aprovam automaticamente o perfil GodMesh proposto.

- OpenID Foundation, [OpenID Connect Core 1.0, errata set 2](https://openid.net/specs/openid-connect-core-1_0.html), especialmente `iss`, `sub`, `aud` e expiração do ID Token. Limite: define autenticação entre RP e OP, não autorização de recursos GodMesh.
- IETF, [RFC 9700 — Best Current Practice for OAuth 2.0 Security](https://www.rfc-editor.org/rfc/rfc9700.html), janeiro de 2025. Recomenda sender-constrained e audience-restricted access tokens. Limite: a escolha entre DPoP e mTLS depende da implantação.
- IETF, [RFC 9449 — OAuth 2.0 Demonstrating Proof of Possession](https://www.rfc-editor.org/rfc/rfc9449.html), setembro de 2023. DPoP vincula token a chave e requisição e exige verificação de URI/método; o RFC declara que PoP não é autenticação nem controle de acesso por si só.
- IETF, [RFC 9068 — JWT Profile for OAuth 2.0 Access Tokens](https://www.rfc-editor.org/rfc/rfc9068.html), outubro de 2021. Define perfil de access token JWT. Limite: o contrato GMP ainda pode escolher token opaco/introspecção.
- IETF, [RFC 7009 — OAuth 2.0 Token Revocation](https://www.rfc-editor.org/rfc/rfc7009.html), agosto de 2013. Define endpoint de revogação e alerta que a propagação pode atrasar entre servidores.
- IETF, [RFC 7515 — JSON Web Signature](https://www.rfc-editor.org/rfc/rfc7515.html), maio de 2015; [RFC 7518 — JSON Web Algorithms](https://www.rfc-editor.org/rfc/rfc7518.html), maio de 2015; [RFC 7638 — JSON Web Key Thumbprint](https://www.rfc-editor.org/rfc/rfc7638.html), setembro de 2015; [RFC 8725 — JWT Best Current Practices](https://www.rfc-editor.org/rfc/rfc8725.html), fevereiro de 2020; [RFC 8037 — EdDSA for JOSE](https://www.rfc-editor.org/rfc/rfc8037.html), janeiro de 2017; [RFC 6979 — Deterministic DSA/ECDSA](https://www.rfc-editor.org/rfc/rfc6979.html), agosto de 2013. Base das propostas JWS, ES256, EdDSA, thumbprint e validação restrita de algoritmo. Limite: algoritmo final e chave não foram aprovados/testados.
- IETF, [RFC 9562 — UUID](https://www.rfc-editor.org/rfc/rfc9562.html), maio de 2024; [RFC 7033 — WebFinger](https://www.rfc-editor.org/rfc/rfc7033.html), setembro de 2013; [RFC 7565 — `acct:` URI Scheme](https://www.rfc-editor.org/rfc/rfc7565.html), dezembro de 2014. Referências para identificadores e descoberta já presentes nas decisões GodMesh.
