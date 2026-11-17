# GMP / API v1 — contrato documental proposto

> **Status:** proposta documental versionada; não é ADR aprovada, implementação, OpenAPI validado, arquivo `.proto` compilado ou prova de interoperabilidade. Data: 2026-11-15. Aplica-se à interface HTTP pública `/v1` e aos envelopes de controle GMP v1. As versões de transporte (QUIC, HTTP) são negociadas separadamente.
>
> **Autoridade:** este arquivo detalha o wire/API de `gmp.control.v1`, `gmp.event.v1` e `gmp.federation.v1`; `gmp.media.v1` mantém o payload de áudio fora da fila/log de eventos. `eventos-sincronizacao.md` é fonte de IDs, lifecycle/attempt/ACK, cursor/replay, inbox e política de idempotência; `identidade-autorizacao.md` é fonte de principal, credenciais, scopes candidatos e revogação; `../01-arquitetura/autoridade-escrita.md` propõe autoridade/epoch; `../04-seguranca/retencao-revogacao.md` propõe política por classe/cópia; `../01-arquitetura/normalizacao-dados.md` define proveniência/ProjectionReport UDNL, e `../03-integracoes/gateway-universal.md` aplica esses contratos a bridges. As fontes atuais continuam propostas; o mapeamento deste arquivo não as ratifica nem autoriza implementação presumida.

## 1. Decisões propostas e limites

1. **Wire GMP:** Protocol Buffers v3, com namespace/package versionado (`godmesh.gmp.control.v1`, `godmesh.gmp.event.v1`, `godmesh.gmp.federation.v1`). HTTP usa JSON. A mesma operação lógica pode ter mapeamento nos dois meios, mas o envelope de cada transporte não é intercambiável.
2. **Contrato HTTP canônico:** publicar um documento OpenAPI, proposta inicial `3.2.1`, separado deste resumo. A versão OpenAPI do artefato não muda o prefixo de API `/v1`; alteração incompatível exige novo major de API. O contrato abaixo é fonte de revisão até que o documento OpenAPI seja criado, validado e indicado como normativo.
3. **Handshake:** a proposta usa negociação explícita de major/minor e features antes de aceitar comandos. O nome ALPN `godmesh-gmp/1` é apenas valor candidato; requer revisão de arquitetura, colisões de nome e compatibilidade antes de adoção.
4. **Identidade/autorização:** sujeito, prova, scopes, credenciais e revogação referenciam `identidade-autorizacao.md`. Os nomes de scopes abaixo são uma proposta de vocabulário para tornar as operações revisáveis, não scopes aprovados nem substitutos de ACL.
5. **Semântica distribuída:** `event_id`, `command_id`, `operation_id`, `attempt_id`, lifecycle/attempt/ACK, cursor/replay e inbox são regidos por `eventos-sincronizacao.md`. A fingerprint do pedido é proposta por C07 em §5.1 para satisfazer a integração exigida por essa fonte. Não se afirma exactly-once, entrega garantida nem duração de retenção.
6. **Limites:** todos os limites de bytes, concorrência, batch, paginação e tempo são parâmetros obrigatórios do perfil publicado do serviço. Nenhum valor numérico de produção, SLO, retenção, RPO/RTO ou carga foi escolhido ou medido. Serviço sem perfil de limites explícito não anuncia readiness para a operação afetada.

### Compatibilidade e alternativas

| Tema | Proposta C07 | Alternativa | Motivo e efeito | Estado |
|---|---|---|---|---|
| API HTTP | OpenAPI 3.2.1 + JSON Schema | OpenAPI 3.1.2 ou contrato equivalente | 3.2.1 é a versão corrente listada pela especificação oficial consultada; generators, SDKs, validadores e cliente mínimo ainda precisam ser avaliados. Se a compatibilidade de ferramentas falhar, selecionar e fixar 3.1.2 antes de publicar o artefato. | Proposta; decisão de ferramenta pendente |
| GMP controle/eventos | Protobuf v3 | JSON/CBOR ou outro schema evolutivo | Mantém o padrão já citado nos documentos e oferece schema com tags numéricas; exige disciplina de compatibilidade, presence e reserva de tags. | Direção consistente com ADR-021/029; wire final pendente |
| QUIC 0-RTT | Sem dados de aplicação por padrão em v1; futura autorização só por perfil de operações replay-safe | Permitir operações de leitura específicas com análise de replay | RFC 9001 exige perfil da aplicação para 0-RTT e medidas contra replay. Bloquear até existir esse perfil evita assumir idempotência no transporte. Efeito: pode aumentar latência inicial; não compromete a semântica de retry no nível da aplicação. | Proposta de segurança; validação de implementação pendente |

## 2. Tipos e convenções de schema

| Tipo lógico | Representação GMP / HTTP | Presença e validação |
|---|---|---|
| UUID | `string` RFC 9562 em JSON; `string` canônica em Protobuf enquanto interoperabilidade multi-linguagem não for validada | Deve ser UUID válido; identidade interna permanece distinta de alias/GMC. |
| Timestamp | `google.protobuf.Timestamp` em Protobuf; string RFC 3339 UTC (`date-time`) em JSON | Presença explícita para datas opcionais. Rejeitar data inválida ou fora do prazo da operação. |
| OpaqueCursor | `bytes` no GMP; string base64url opaca no HTTP | Cliente não interpreta nem constrói; vinculado ao vetor de checkpoints contíguos, `partition` e `authority_epoch` obrigatórios (nenhum cursor sem partição ou de epoch incomparável é aceito, conforme ADR-057 e `eventos-sincronizacao.md` v0.2), filtros, principal/escopo, schema e política de retenção efetiva conforme eventos/sync. |
| IdempotencyKey / `operation_id` | UUIDv7 proposto; HTTP: forma textual UUID canônica no header `Idempotency-Key`; GMP: 16 bytes com a mesma identidade lógica | O cliente trata a chave como opaca. Namespace do servidor: `(tenant_id, principal_ref, operation_kind, resource_ref, operation_id)`; autenticação/autorização precedem lookup. Política de idade, retenção e replay seguem eventos/sync. |
| `request_fingerprint` | digest `sha256:` + hex minúsculo de 32 bytes | Proposta versionada conforme §5.1; autenticação, tokens, proof, trace e headers de transporte não entram no digest. |
| Scope | string no contexto autenticado | Servidor deriva `principal` e tenant do token/prova validada; `tenant_id` no payload nunca concede autorização. |
| Nullable/ausente | `optional` em scalar proto3 quando ausência difere de zero/vazio; propriedade ausente em JSON quando opcional | `null` só é aceito quando schema do campo o declarar; ausência não deve virar default silencioso em patch. |
| Enum | enumeração fechada do schema; zero = `UNSPECIFIED`/`UNKNOWN` | Valor crítico desconhecido: rejeitar com erro de versão/enum; nunca inferir permissão. Valores aditivos desconhecidos só são toleráveis em campo não crítico e quando feature compatível foi negociada. |

Protobuf: não renumerar nem reutilizar campo removido; marcar tag/nome removido como `reserved`; adicionar campos somente quando leitores antigos possam ignorá-los com segurança. Campo scalar com presença semântica usa `optional`; updates parciais precisam máscara/campo explícito para distinguir “não enviado” de “zerar”. Os exemplos abaixo são schema de referência a formalizar, não artefato compilado.

### 2.1 Envelope de evento e ACK de Inbox (proposta C07)

O envelope de evento é separado do envelope de controle. Campos e autoridades: `event_id` (UUID lógico estável, C10); `origin_node_id`, `tenant_scope`, `stream`, `origin_sequence` (namespace de ordenação C10); `authority_epoch` (opcional apenas quando o perfil declarar que não se aplica, C06/C10); `schema_profile` (versão de schema); `content_digest` (proposta C07 `sha256:` + 64 hex minúsculos sobre representação canônica versionada, nunca `request_fingerprint`); `occurred_at` (metadado, não ordenação); `retention_policy_ref` (referência interna à regra C08 efetiva por cópia, não prova de autorização); payload tipado por domínio e `SourceProvenance`/`ProjectionReport` quando houver projeção UDNL. O servidor receptor deriva principal/tenant autenticados e valida escopo, assinatura, autorização, frescor de revogação, epoch/fencing e política antes de persistir. Cliente não escolhe a autoridade ou política efetiva.

ACK de sync identifica `origin_node_id`, `tenant_scope`, `stream`, `authority_epoch` (quando aplicável), `last_contiguous_sequence` e o resultado `durable_inbox_commit`. Só pode ser emitido depois de validar e persistir duravelmente no Inbox; sequência com lacuna não avança o checkpoint. ACK confirma recebimento durável nesta cópia, não materialização, entrega a dispositivo, conclusão de comando, reconhecimento humano, réplica global nem descarte. O perfil wire definitivo e as formas de erro permanecem sujeitos à ratificação C10/C06/C08.

Schema ilustrativo em `godmesh.gmp.event.v1` (não compilado; IDs string preservam portabilidade até fixar o tipo UUID wire):

```protobuf
syntax = "proto3";
package godmesh.gmp.event.v1;

import "google/protobuf/timestamp.proto";

message EventEnvelope {
  optional string event_id = 1;
  optional string origin_node_id = 2;
  optional string tenant_scope = 3;
  optional string stream = 4;
  optional uint64 origin_sequence = 5;
  optional uint64 authority_epoch = 6;
  optional string schema_profile = 7;
  optional string content_digest = 8;
  optional google.protobuf.Timestamp occurred_at = 9;
  optional string retention_policy_ref = 10;
  optional bytes typed_payload = 11;
  optional bytes source_provenance = 12;
  optional bytes projection_report = 13;
}

enum InboxAckResult {
  INBOX_ACK_RESULT_UNSPECIFIED = 0;
  DURABLE_INBOX_COMMIT = 1;
  SEQUENCE_GAP_PENDING = 2;
}

message InboxAck {
  optional string origin_node_id = 1;
  optional string tenant_scope = 2;
  optional string stream = 3;
  optional uint64 authority_epoch = 4;
  optional uint64 last_contiguous_sequence = 5;
  optional InboxAckResult result = 6;
}
```

Regras de presença: `event_id`, origem, scope, stream, sequence, schema, digest e payload são obrigatórios; epoch ausente só em perfil que declare que não se aplica. `DURABLE_INBOX_COMMIT` exige persistência validada; `SEQUENCE_GAP_PENDING` nunca confirma sequência acima da lacuna. `typed_payload`, proveniência e relatório devem virar mensagens/tipos definidos nos schemas proprietários antes de interoperabilidade; não aceitam bytes arbitrários como bypass de validação.

## 3. Negociação de protocolo e handshake

### 3.1 Camadas independentes

1. **Transporte:** HTTPS/TLS, QUIC/TLS ou outro transporte autorizado pela tabela de transportes. Negociação de versão de QUIC/HTTP não negocia versão GMP.
2. **Protocolo de aplicação:** após canal protegido e autenticação de servidor, peers negociam major/minor GMP e features suportadas.
3. **Identidade/autorização:** validar credencial, DPoP para cliente público ou mTLS quando perfil serviço-a-serviço, subject/tenant-context, status/frescor de revogação e scopes candidatos conforme `identidade-autorizacao.md`; no federation peer, validar identidade de nó e admissão. Token JWT versus opaco segue pendente no C09.
4. **Operação:** somente após conclusão do handshake e autorização, aceitar envelopes de controle/evento. Uma credencial válida/scope não substitui membership, ACL, consentimento/finalidade nem autorização de saída e de entrada. Mídia usa sessão e autorização próprias.

### 3.2 Mensagens de negociação

| Mensagem/campo | Tipo | Presença | Regra |
|---|---|---|---|
| `ClientHello.protocol_major` | `uint32` | obrigatório | Major de wire compatível que o cliente oferece. |
| `ClientHello.min_minor`, `max_minor` | `uint32` | obrigatório | Intervalo inclusivo; `min_minor <= max_minor`. |
| `ClientHello.features[]` | `FeatureOffer[]` | obrigatório, pode ser vazio | Cada entrada tem `feature_id:string`, `required:bool`; IDs únicos. |
| `ClientHello.client_instance_id` | UUID | obrigatório após autenticação de cliente | Identifica instância, não pessoa, device owner nem credencial. |
| `ClientHello.nonce` | bytes | obrigatório | Valor aleatório fresco para vincular confirmação da sessão; tamanho máximo anunciado pelo perfil. Não é credencial. |
| `ServerHello.protocol_major`, `selected_minor` | `uint32` | obrigatório | Seleção deve pertencer ao intervalo anunciado; major exato. |
| `ServerHello.features[]` | `string[]` | obrigatório, pode ser vazio | Subconjunto de features oferecidas e autorizadas; nenhuma feature desconhecida/obrigatória é silenciosamente descartada. |
| `ServerHello.server_node_id` | UUID | obrigatório em sessão node-to-node; ausente para edge se perfil permitir | Validado contra identidade autenticada; endereço de rede não é identidade. |
| `ServerHello.session_id` | UUID | obrigatório | Novo identificador de sessão; não reutilizado como `message_id` ou key idempotente. |
| `ServerHello.echoed_nonce`, `server_nonce` | bytes | obrigatório | Confirma ligação ao hello atual e evita aceitar resposta de sessão anterior. |
| `ServerHello.profile_id` | string | obrigatório | Identifica o perfil de limites/transportes/recursos divulgado pelo serviço. |

`FeatureOffer` e respostas são autenticadas dentro do canal protegido. Antes da seleção de versão, não transmitir PII, localização, credenciais reutilizáveis ou comandos de efeito persistente.

### 3.3 Resultado da negociação

- Sucesso somente quando há interseção do major e intervalo minor, todos os requisitos da feature obrigatória são satisfeitos, o perfil é conhecido e a identidade pode ser autorizada.
- Sem interseção: encerrar com `UNSUPPORTED_VERSION`; não fazer downgrade silencioso nem tentar bridge como fallback implícito.
- Feature obrigatória desconhecida ou não permitida: encerrar com `UNSUPPORTED_REQUIRED_FEATURE`.
- Feature opcional desconhecida pode ser ignorada apenas se não alterar autorização, confidencialidade, semântica de estado, escopo ou interpretação de dados.
- Atualização do `v1` só pode adicionar campo/feature compatível. Mudança de significado, requisito de presença, autorização ou comportamento de enum exige `v2` ou nova feature explicitamente negociada.
- Negociação bem-sucedida não equivale a autorização para qualquer operação, não valida capacidade de hardware e não significa readiness de um subsistema.

## 4. Envelope GMP de controle

### 4.1 Estrutura lógica proposta

| Campo | Tipo | Presença | Significado |
|---|---|---|---|
| `message_id` | UUID | obrigatório | Identifica a transmissão lógica desta mensagem GMP; não substitui a chave idempotente da operação. |
| `message_type` | enum/string namespaced | obrigatório | Tipo estável (`CONTROL_REQUEST`, `CONTROL_RESULT`, `ERROR`); corpo discriminado por `oneof`. |
| `protocol_major`, `protocol_minor` | `uint32` | obrigatório | Deve coincidir com handshake corrente. |
| `tenant_id` | UUID | obrigatório em operação tenant-scoped | Contexto alegado, conferido contra o sujeito e grant verificados; ausência ou mismatch falha fechado. Não define tenant autorizado. |
| `principal_ref` | string opaque | derivado do contexto autenticado; nunca aceito como prova do chamador | Referência para auditoria/redação. O valor verificável e o formato são definidos pelo contrato de identidade. |
| `issued_at`, `expires_at` | Timestamp | obrigatórios em comando mutável; opcionais somente para health/read sem efeito | `issued_at < expires_at`; relógio/tolerância dependem de perfil operacional publicado; fora da janela → `DEADLINE_EXCEEDED`. |
| `operation_id` | UUIDv7 em forma textual/16 bytes GMP (proposta C10) | obrigatório em operação mutável | Um-para-um com HTTP `Idempotency-Key`; opaco ao cliente; não é `message_id`, `command_id`, `attempt_id` nem `event_id`. |
| `request_fingerprint` | string ASCII `sha256:` + 64 hex minúsculos | obrigatório em operação mutável depois da canonicalização; derivado/verificado por servidor | Identifica a representação lógica do pedido para idempotência; nunca é fornecido como prova pelo cliente. Detalhe em §5.1. |
| `authority_epoch` | `uint64` | obrigatório em mutação que exige autoridade forte, caso contrário ausente | Deve corresponder à atribuição committed para a partição; segue C06. Não é o `expected_epoch` do grant PTT/canal (domínio separado), não transforma epoch em autorização e não participa da fingerprint lógica; cada tentativa usa autoridade vigente. |
| `partition_id` | string opaca | obrigatório junto a `authority_epoch` em mutação forte | Partição C06 sobre a qual o escritor exerce autoridade; servidor determina o escopo e o cliente não escolhe autoridade por proximity/health. |
| `authority_node_id` | UUID | obrigatório junto a `authority_epoch` em mutação forte | Nó escritor da atribuição committed; verificado no destino. `home_region` não é encaminhamento nem prova e não é repetido no comando como autoridade. |
| `authority_proof_ref` | string opaca | obrigatório quando o perfil de autoridade exigir prova verificável no envelope; ausente em operações sem autoridade forte | Referência verificável conforme contrato de autoridade/identidade; hora/health não substituem prova nem fencing no destino. |
| `command_id` | UUID | obrigatório em comando que tem ciclo de vida assíncrono; ausente em leitura ou evento sem comando | ID estável do ciclo de vida de comando, distinto de operation_id e message_id. |
| `attempt_id` | UUID | obrigatório somente em cada DeliveryAttempt; ausente fora de uma tentativa | Novo em cada tentativa de entrega; não cria nova operação/comando. |
| `dispatch_scope_ref`, `dispatch_sequence` | `string` opaca + `uint64` | ambos obrigatórios somente em comando de bridge `per_target_serial`; ambos ausentes nos demais envelopes | Identificam o escopo local de ordenação (bridge/alvo/epoch) e a posição do comando lógico nesse escopo; proposta C07/C15 abaixo. |
| `trace_id` | bytes/UUID opaco | opcional | Correlação; nunca transportar segredo em trace. |
| `body` | `oneof` por domínio | obrigatório | Payload específico validado por feature, tamanho e autorização. |

Schema ilustrativo para tornar wire tags e presence revisáveis; **não compilado nem aprovado**. Regras de tenant, identidade e idempotência acima continuam aplicáveis:

```protobuf
syntax = "proto3";
package godmesh.gmp.control.v1;

import "google/protobuf/timestamp.proto";

enum MessageKind {
  MESSAGE_KIND_UNSPECIFIED = 0;
  MESSAGE_KIND_REQUEST = 1;
  MESSAGE_KIND_RESULT = 2;
  MESSAGE_KIND_ERROR = 3;
}

message FeatureOffer {
  string feature_id = 1;
  bool required = 2;
}

message ClientHello {
  optional uint32 protocol_major = 1;
  optional uint32 min_minor = 2;
  optional uint32 max_minor = 3;
  repeated FeatureOffer features = 4;
  optional string client_instance_id = 5;
  optional bytes nonce = 6;
}

message ServerHello {
  optional uint32 protocol_major = 1;
  optional uint32 selected_minor = 2;
  repeated string features = 3;
  optional string server_node_id = 4;
  optional string session_id = 5;
  optional bytes echoed_nonce = 6;
  optional bytes server_nonce = 7;
  optional string profile_id = 8;
}

message ControlRequest {
  optional string operation_kind = 1;
  optional string resource_id = 2;
  optional bytes payload = 3;
}

message GmpError {
  optional string code = 1;
  optional string request_id = 2;
  optional bool retryable = 3;
  optional uint32 retry_after_ms = 4;
  optional string safe_message = 5;
}

message Envelope {
  optional string message_id = 1;
  optional MessageKind message_kind = 2;
  optional uint32 protocol_major = 3;
  optional uint32 protocol_minor = 4;
  optional string tenant_id = 5;
  optional google.protobuf.Timestamp issued_at = 6;
  optional google.protobuf.Timestamp expires_at = 7;
  optional bytes operation_id = 8;
  optional bytes trace_id = 9;
  optional string request_fingerprint = 13;
  optional string command_id = 14;
  optional string attempt_id = 15;
  optional uint64 authority_epoch = 16;
  optional string authority_proof_ref = 17;
  optional string partition_id = 18;
  optional string authority_node_id = 19;
  optional string dispatch_scope_ref = 20;
  optional uint64 dispatch_sequence = 21;
  oneof body {
    ControlRequest control_request = 10;
    bytes domain_result = 11;
    GmpError error = 12;
  }
}
```

Validation profile requires `protocol_major`, `message_id`, `message_kind` and the matching body even though proto3 encodes them as optional/presence-aware scalars; it also validates required `tenant_id`, deadlines and `operation_id` for mutating operations. `payload`/`domain_result` are placeholders for message-specific typed definitions in the eventual `.proto`; they do not authorize arbitrary opaque payload in production. Replaced/removed tags must be reserved. The final design should split handshake messages from established-session envelopes into the appropriate package(s).

### 4.2 Framing e limites

Proposta para GMP em QUIC stream: cada mensagem é um Protobuf delimitado por comprimento unsigned-varint, seguido exatamente pelo número declarado de bytes. DATAGRAM não transporta envelopes mutáveis de controle. Para transporte que já delimita mensagens, não adicionar prefixo redundante. Antes de alocar ou decodificar, o parser verifica prefixo, comprimento, `max_envelope_bytes`, `max_body_bytes` e limite de campos/repetições do perfil. Comprimento truncado, bytes excedentes, prefixo malformado ou limite excedido produz erro tipado e fecha/reset o stream conforme o perfil; nunca tenta ressincronizar lendo payload arbitrário.

Os números efetivos para `max_envelope_bytes`, `max_body_bytes`, `max_repeated_items`, `max_inflight_requests`, deadlines e concorrência regional ficam em perfil publicado e versionado. Enquanto escala/carga/capacidade não foram definidas, valores não são aprovados; ausência desses parâmetros bloqueia readiness e divulgação de compatibilidade operacional. Métrica de tamanho e saturação não deve registrar payload privado.

### 4.3 Exemplo positivo

O exemplo demonstra shape, não identidade real nem dado autorizado. `issued_at`, expiração, tenant, ACL e operação precisam passar validação do servidor.

```json
{
  "message_id": "019a0000-0000-7000-8000-000000000021",
  "message_type": "CONTROL_REQUEST",
  "protocol_major": 1,
  "protocol_minor": 0,
  "tenant_id": "019a0000-0000-7000-8000-000000000011",
  "issued_at": "2026-11-15T15:00:00Z",
  "expires_at": "2026-11-15T15:00:30Z",
  "operation_id": "019a0000-0000-7000-8000-000000000041",
  "body": {
    "kind": "channel.floor.request",
    "channel_id": "019a0000-0000-7000-8000-000000000031"
  }
}
```

### 4.4 Casos negativos

- `tenant_id` diverge do contexto autenticado → `TENANT_SCOPE_MISMATCH`; não procurar o recurso em outro tenant.
- Operação usa campo `principal_ref` fornecido pelo cliente para elevar autoridade → ignorar como prova e negar `PERMISSION_DENIED`.
- Expiração ausente em comando mutável, `issued_at >= expires_at` ou prazo fora do perfil → `INVALID_ARGUMENT`/`DEADLINE_EXCEEDED`, sem enfileirar.
- `operation_id` repetido com payload diferente → conflito de idempotência definido por eventos/sync; nunca substituir o resultado anterior.
- Mensagem `message_id` repetida não autoriza reaplicação; regras e janela de deduplicação pertencem ao contrato de eventos/sync.
- Feature crítica ausente, enum desconhecido crítico, versão major incorreta ou envelope acima do limite → rejeitar antes de executar efeito.

### 4.5 Ordenação local de dispatch de bridge (proposta C07)

Para comando de bridge serializado por alvo, `dispatch_scope_ref` é uma referência opaca à geração local persistida do dispatcher, vinculada a bridge e alvo autorizados; `dispatch_sequence:uint64` é positiva e crescente dentro desse escopo, atribuída uma vez por comando lógico. Os dois campos aparecem juntos somente na tentativa de executar um `command_id`; não substituem `operation_id`, `attempt_id`, `authority_epoch`, `origin_sequence` nem cursor/checkpoint C10. Operações `independent` sem requisito de ordem não carregam esses campos. A referência wire não é o registro durável: o serviço que possui o dispatch mantém geração, próxima posição, checkpoint contíguo, pendências e associações a `command_id`/`operation_id`, persistindo escopo, posição e comando/outbox atomicamente antes de confirmar `queued`. Uma alocação confirmada não é desfeita nem reutilizada após reinício.

**Separação do fingerprint:** ambos são metadados de controle/ordenação, gerados pelo dispatcher, e ficam fora de `request_fingerprint`. A ordem local não altera o pedido semântico nem o efeito solicitado; reencaminhar a mesma operação dentro do mesmo `dispatch_scope_ref` conserva sequência e fingerprint, enquanto cada nova `DeliveryAttempt` conserva `command_id`/`operation_id`/fingerprint/`dispatch_sequence` e recebe novo `attempt_id`. Uma mudança de `target_ref` que altere o destino semântico continua dentro do fingerprint conforme §5.1; `dispatch_scope_ref` não é substituto desse alvo. Assim, recálculo do digest não confunde retry, seleção local de fila ou autoridade com a identidade do pedido. Se uma futura operação definir que posição/ordem muda o efeito de domínio, isso não pode ser codificado por esta sequência local: o campo semântico correspondente deverá integrar uma nova versão do schema/fingerprint.

**Retry, recovery e reordenação propostos:** (1) Crash antes do commit atômico não confirma `queued` nem envia; uma repetição só pode alocar após consultar idempotência e confirmar que não houve commit anterior. (2) Crash depois do commit e antes do envio reenvia mesmos IDs, escopo e sequência com novo `attempt_id`, sem alocar outra posição. (3) Após envio sem resultado conclusivo, `DeliveryAttempt.state=outcome_unknown` é estado, não erro: consultar recibo/estado remoto com a mesma identidade; sem prova conclusiva ou deduplicação remota, bloquear qualquer novo efeito. (4) Mesmo escopo e sequência registrados com o mesmo `command_id`, `operation_id` e fingerprint são duplicata: devolver estado conhecido, sem novo efeito; identidade/fingerprint divergente na mesma posição falha fechado. (5) Sequência acima do checkpoint contíguo fica pendente, não produz efeito, não avança watermark e exige recuperação da lacuna; após resolvê-la, revalidar autorização, expiração, autoridade e fencing. (6) Restore antigo ou perda do registro isola o dispatcher; sem fonte mais recente/reconciliação que prove a maior posição e resolva efeitos incertos, manter rota bloqueada, sem reiniciar posição. (7) Expiração impede iniciar ou renovar efeito; consulta autorizada pode retornar resultado já conhecido sem reabrir comando. (8) Recuperação ou destino indisponível mantém comando pendente/`outcome_unknown`, sem avançar checkpoint nem contornar lacuna; eventual retry usa os mesmos IDs e posição apenas enquanto prazo, autorização, retenção e deduplicação permitirem. (9) Rotação não ativa a nova geração para o alvo enquanto houver posições/efeitos anteriores sem resolução suficiente para impedir duplicação. O destino serial não aplica `n+1` antes de resolver `n`; comandos `independent` só podem reordenar quando o perfil versionado os declarar comutativos e seguros fora de ordem.

Estas regras são proposta documental para mapear o ciclo de vida local descrito em C15, não uma alegação de protocolo ou persistência implementados. A tabela de erros §6 mantém `DISPATCH_SEQUENCE_GAP`, `DISPATCH_SEQUENCE_CONFLICT` e `DISPATCH_SCOPE_STALE` como nomes candidatos; `outcome_unknown` continua estado de DeliveryAttempt. `DEADLINE_EXCEEDED` representa prazo vencido e impede efeito novo; `UNAVAILABLE` representa dependência indisponível e permite somente consulta ou retry da mesma identidade/posição sob prazo, autorização, retenção e dedupe válidos. Nenhum desses erros autoriza repetir efeito externo incerto. C10 deve incorporar/revisar lifecycle e recovery sem criar sequência concorrente de sync; C08 precisa definir retenção/tombstone suficientes para consulta e deduplicação. Até isso ocorrer, recovery durable/readiness não pode ser declarado.

**Crosswalk canônico de lifecycle:** `eventos-sincronizacao.md` §4.1.3, “Propriedade e transições entre os estados”, é a fonte da proposta de estados do registro local e das transições. Este GMP só mapeia o resultado para campos/status/error wire; não cria enum nem lifecycle paralelo. `CommandReceipt.status` continua exclusivamente `Command.lifecycle` C10 (§7.2); o registro de dispatch e `DeliveryAttempt.state` são eixos separados. Estados locais `pending_gap`, `resolved` e `scope_stale` não são valores de `Command.lifecycle` nem enums wire.

| Estado local C10 §4.1.3 | Evidência/gatilho que o GMP pode reportar | Efeito no `DeliveryAttempt` / `Command.lifecycle` | Resposta/erro proposto e proibição |
|---|---|---|---|
| `queued` | Commit atômico de escopo, posição e comando/outbox antes da confirmação local | Pode reportar `persisted`; tentativa enfileirada não prova submissão, `routed` ou conclusão | Recibo de persistência; não reportar aceitação externa nem `completed` |
| `pending_gap` | Posição posterior observada sem prefixo anterior contíguo | Registrar pendência/lacuna; sem avanço automático do lifecycle | `DISPATCH_SEQUENCE_GAP` candidato; reter sem efeito e sem avançar checkpoint |
| `outcome_unknown` | Envio possível sem recibo/resultado conclusivo para o mesmo comando e posição | `DeliveryAttempt.state=outcome_unknown`; lifecycle não muda sem evidência e regra C10 | Estado/consulta, não erro de resultado; dependência de consulta indisponível pode retornar `UNAVAILABLE`; não inferir sucesso/falha nem criar posição/efeito novo |
| `resolved` | Recibo autenticado ou reconciliação conclusiva ligada ao mesmo comando/posição | Registrar evidência; mudar `Command.lifecycle` apenas se o predicado de sucesso/falha C10 estiver provado | Reportar o recibo/resultado observado; resolver checkpoint não equivale por si só a `completed` |
| `expired` | Serviço dono verifica `expires_at` sob relógio/perfil aplicável | Pode tornar `Command.lifecycle=expired` conforme C10 e encerrar novas tentativas de efeito | `DEADLINE_EXCEEDED` candidato para início/renovação de efeito. Se uma tentativa anterior continua `outcome_unknown`, preservar essa incerteza e permitir apenas reconciliação/consulta autorizada; expiração não fabrica resultado remoto, não apaga dedupe/recibo nem reabre comando |
| `scope_stale` | Dispatcher rejeita referência à geração local retirada/incompatível | Registrar recusa/encerramento da tentativa antiga; lifecycle não muda por si | `DISPATCH_SCOPE_STALE` candidato; não reescrever escopo/posição nem confundir com `authority_epoch_stale`; nova geração requer reconciliação |

`UNAVAILABLE` não é transição de lifecycle: indica dependência indisponível. Antes de `expires_at`, retry só pode reutilizar a mesma identidade lógica, escopo e posição se autorização, política de retenção e dedupe forem atuais; após expiração, apenas consulta/reconciliação, sem novo efeito. Os três códigos `DISPATCH_*`, `DEADLINE_EXCEEDED` e este perfil de retry são propostas técnicas de mapeamento C07 para revisão, não ratificações de estado/erro wire nem aprovação de política. A aprovação da norma pertence ao usuário.

## 5. Replay, idempotência e prazo

1. QUIC/TLS protege tráfego de sessão, mas não é a deduplicação de operação. `event_id`, `message_id`, `command_id`, `operation_id` e `attempt_id` têm funções distintas conforme eventos/sync; não os reutilizar como sinônimos.
2. **Perfil v1 proposto:** não aceitar dados de aplicação GMP ou requisições HTTP mutáveis em QUIC/TLS 0-RTT. Até aprovação de perfil específico, primeira mensagem de aplicação depende de handshake concluído. Somente uma decisão futura poderá liberar leitura replay-safe, com lista de operações e análise explícita de privacidade.
3. Comando mutável carrega `operation_id` e o serviço calcula/verifica `request_fingerprint`; o cliente HTTP não escolhe nem envia o digest. GMP inclui-o somente em envelope autenticado de serviço após canonicalização. HTTP `Idempotency-Key` é a forma textual proposta do mesmo UUIDv7 lógico; GMP usa os mesmos 16 bytes. A chave é opaca para o cliente. O servidor deriva `tenant_id` e `principal_ref` do contexto autenticado e consulta a operação somente após revalidar autorização atual, dentro do namespace `(tenant_id, principal_ref, operation_kind, resource_ref, operation_id)` de eventos/sync.
4. Mesmo namespace + mesma chave + mesma fingerprint devolve recibo/estado existente sem novo efeito, desde que policy efetiva autorize revelar o resultado. Fingerprint divergente retorna `IDEMPOTENCY_CONFLICT`; nunca altera a operação anterior. UUIDv7, limite de idade/skew, política de retenção e resultado após expiração permanecem propostas/parâmetros sem valor aprovado; chave antiga desconhecida não deve ser reinterpretada como operação nova.
5. Requisição expirada é rejeitada sem novo efeito. Para comando já persistido, o cliente consulta status com autorização; não inventa resultado no timeout e não renova implicitamente o prazo.
6. Revogação e epoch/authority são verificadas na execução e em cada replay autorizado. Contrato de identidade, autoridade de escrita e retenção regem os detalhes; falta desses contratos bloqueia operação mutável federada.

### 5.1 Fingerprint canônica do pedido (proposta C07)

Para tornar a regra de C10 e do perfil Gateway verificável em HTTP e GMP, `request_fingerprint` é proposta como `sha256:` seguido de 64 dígitos hexadecimais minúsculos. A entrada TLV tem campos semânticos tipados em IDs fixos e ordem crescente: (1) `fingerprint_version:uint16=1`; (2) `operation_kind` UTF-8; (3) `resource_ref` tipo + ID; (4) `target_ref` tipo + ID, ou marcador explícito de ausência se o alvo coincidir com o recurso/não se aplicar; (5) `command_schema_version` UTF-8; (6) presença de `expires_at` (deadline), seconds `int64`, nanos `uint32`; (7) presença/bytes de `expected_version`; (8) corpo já validado em ordem dos field IDs estáveis do schema, com todos defaults aplicados. Cada item codifica field ID, marcador de presença, comprimento em bytes e valor. UUID é 16 bytes; strings são UTF-8 exatas após normalização definida pelo campo; integers são big-endian de largura do tipo; enums usam IDs estáveis, não nomes localizados; arrays preservam a ordem definida pelo schema. Cada comando declara sua tabela de field IDs e quais campos afetam o efeito; se um alvo distinto afetar o efeito, sua inclusão é obrigatória.

O mesmo pedido lógico HTTP e Protobuf deve produzir a mesma sequência canônica antes do SHA-256; serialização JSON bruta, serialização Protobuf genérica, header `Authorization`, token, nonce de prova, `principal_ref`, `tenant_id`, trace, tentativa, `message_id`, `authority_epoch`, referência/política de retenção e campos de transporte não entram no digest. O namespace C10 liga o hash ao tenant/principal/operação/recurso/chave. Mudança de qualquer campo semanticamente efetivo exige nova `command_schema_version`/fingerprint version. Consumidor recalcula a fingerprint a partir do pedido autenticado e compara com o registro; não confia em digest apresentado pelo cliente.

Esta escolha de algoritmo e codificação é decisão técnica **proposta**, não ratificada nem testada entre linguagens. Alternativa: canonicalização definida por schema/formato que as toolchains efetivamente suportem, desde que HTTP/GMP produzam o mesmo digest. Antes de interoperação, requer vetores documentais: mesmas propriedades JSON em ordem diferente geram mesmo digest; payload, expiração ou versão que muda o efeito gera digest diferente; token, trace e tentativa diferentes não mudam o digest. Até seleção/ratificação e vetores multi-linguagem, fingerprint persistida não é interoperabilidade declarada.

## 6. Erros de protocolo

Envelope lógico `GmpError` proposto:

| Campo | Tipo | Presença | Regra |
|---|---|---|---|
| `code` | string com token canônico estável | obrigatório | Preserva exatamente códigos wire/domain listados neste documento ou pelo contrato proprietário; casing não é normalizado. Cliente não depende do texto livre. |
| `request_id` | UUID | obrigatório quando request foi decodificado | Correlaciona requisição/resposta sem expor credencial. |
| `retryable` | bool | obrigatório | `true` é indicação, nunca promessa; respeitar `retry_after_ms` quando presente. |
| `retry_after_ms` | `optional uint32` | opcional | Só para backoff orientativo; limites definidos pelo perfil. |
| `safe_message` | string | opcional | Texto legível sem detalhes internos, PII, token ou segredo. |
| `details` | mapa tipado de erro | opcional | Somente campos permitidos por código; nunca echo de conteúdo sensível. |

| Código | Condição | Retry automático |
|---|---|---|
| `INVALID_ARGUMENT` | Campo, enum, formato ou combinação inválida | Não; corrigir entrada |
| `UNAUTHENTICATED` | Credencial/prova ausente, inválida ou expirada | Só após nova autenticação autorizada |
| `invalid_token` / `invalid_proof` | C09: token ou proof of possession/cadeia inválido | Reautenticar/reprovar; não repetir operação sem chave e request válidos |
| `insufficient_scope` / `not_owner` | C09: permissão/principal não cobre recurso | Não; sem revelar titular ou ampliar token |
| `not_found_or_not_visible` | C09: recurso ausente ou oculto ao principal | Resposta anti-enumeração uniforme |
| `AUTHORIZATION_STALE` / `authorization_revoked` | C09/C06/C08: grant/revogação não pode ser confirmada ou foi revogada | Não; falhar fechado em acesso sensível |
| `PERMISSION_DENIED` | Sujeito não autorizado pela ACL/policy | Não sem alteração válida de autorização |
| `TENANT_SCOPE_MISMATCH` | Tenant do pedido não coincide com contexto autenticado | Não |
| `UNSUPPORTED_VERSION` | Major/minor não negociável | Não; upgrade compatível explícito |
| `UNSUPPORTED_REQUIRED_FEATURE` | Feature requerida sem suporte/autoridade | Não |
| `SCHEMA_VIOLATION` | Corpo inválido ou campo crítico incompatível | Não |
| `MESSAGE_TOO_LARGE` | Excede limite de perfil | Não; reduzir payload |
| `REPLAY_DETECTED` | Repetição fora da regra/escopo autorizado | Não reaplicar |
| `IDEMPOTENCY_CONFLICT` | C10: mesmo namespace/chave com fingerprint diferente | Não; manter registro original |
| `IDEMPOTENCY_KEY_EXPIRED` | C10: chave desconhecida além da idade máxima configurada | Não; nunca executar como chave nova |
| `IDEMPOTENCY_KEY_TIME_INVALID` | C10: UUIDv7 inválido ou timestamp fora da tolerância configurada | Não |
| `IDEMPOTENCY_POLICY_UNAVAILABLE` | Política idempotente/retentiva não está efetiva | Não; falhar fechado |
| `IDEMPOTENCY_RESULT_EXPIRED` | Resultado completo expirou, mas consulta mínima ainda é permitida pela política | Não; não repetir efeito |
| `CURSOR_EXPIRED` | C10: cursor ultrapassou intervalo retido | Não; informar `snapshot_required=true` quando autorizado |
| `SNAPSHOT_REQUIRED` | C10: recuperação exige snapshot íntegro antes de replay | Após obter snapshot/prova elegível |
| `EVENT_IDENTITY_CONFLICT` | C10/UDNL: identidade de evento repetida com digest divergente | Não; quarentena, não sobrescrever |
| `EVENT_SCHEMA_UNSUPPORTED` | Schema de evento não suportado/negociado | Não; atualizar compatibilidade |
| `EVENT_AUTHORIZATION_DENIED` | Evento/replay fora de autorização vigente | Não |
| `EVENT_SIGNATURE_INVALID` | Assinatura/prova de evento inválida | Não |
| `EVENT_EPOCH_STALE` | Evento usa epoch de autoridade obsoleto | Não |
| `SEQUENCE_GAP` | Falta sequência anterior no namespace do stream | Retry/recovery do intervalo, sem avançar checkpoint |
| `DISPATCH_SEQUENCE_GAP` | Falta posição anterior no escopo local serial de bridge | Resolver/reentregar a lacuna; não aplicar comandos posteriores |
| `DISPATCH_SEQUENCE_CONFLICT` | Mesma posição local já vinculada a outro comando/operação/fingerprint | Não; preservar registro e não aplicar efeito |
| `DISPATCH_SCOPE_STALE` | Referência opaca aponta para alvo/bridge/epoch local não vigente | Não; renovar roteamento/escopo com autorização atual |
| `INBOX_UNAVAILABLE` | Inbox não pode confirmar persistência durável | Não enviar ACK; backpressure/retry conforme perfil |
| `MATERIALIZATION_PENDING` | Inbox durável, projeção ainda não materializada | Consultar estado, não declarar projeção pronta |
| `MATERIALIZATION_FAILED` | Projeção falhou conforme estado C10 | Retry somente se erro/perfil permitir |
| `RETENTION_POLICY_MISSING` | C08: falta política efetiva/classe/prazo para gravação ou replay persistente | Não; sem persistência/replicação/exportação |
| `AUTHORIZATION_STALE` | C08/C09: atualidade da autorização não pode ser provada | Não; negar leitura/processamento sensível |
| `REVOCATION_PENDING` | Revogação sem confirmação em uma ou mais cópias | Não afirmar aplicação global |
| `authority_unavailable` | C06/C09: authority/quorum/trust indisponível | Não; falhar fechado em mutação forte/acesso privado |
| `authority_epoch_stale` | C06: epoch menor que o committed | Não; mapear para `CONFLICT` HTTP sem revelar dados fora do escopo |
| `authority_node_mismatch` | C06: nó não é escritor da atribuição committed | Não |
| `authority_proof_invalid` | C06: prova ausente/inválida/não committed/escopo incompatível | Não |
| `authority_transfer_in_progress` | C06: transferência ainda não committed | Não para mutação forte |
| `BRIDGE_LOOP_DETECTED` | UDNL: bridge atual já consta em `visited_bridge_ids` | Não encaminhar |
| `BRIDGE_HOP_LIMIT_EXCEEDED` | UDNL: saída excederia `hop_limit` | Não encaminhar |
| `RESOURCE_EXHAUSTED` | Quota ou limite de concorrência atingido | Após backoff, se indicado |
| `UNAVAILABLE` | Serviço/autoridade temporariamente indisponível | Retry apenas com a mesma key e dentro do prazo |
| `DEADLINE_EXCEEDED` | Prazo vencido ou resposta não obtida a tempo | Consultar status antes de nova operação |
| `CONFLICT` | Estado/epoch/versão do recurso mudou | Ler estado atual e reconciliar |
| `INTERNAL` | Falha inesperada | Retry apenas conforme perfil; sem detalhes internos |

Erro de transporte pode não conter `request_id`; timeout de cliente não prova que servidor não persistiu. Antes de repetir comando, usar a mesma `Idempotency-Key` e consultar estado conforme contrato de eventos/sync. Para lifecycle de dispatch, use o crosswalk §4.5 e C10 §4.1.3: timeout/`UNAVAILABLE` não apagam `outcome_unknown`, e `DEADLINE_EXCEEDED` encerra nova tentativa de efeito após expiração, sem inventar resultado remoto. Erros de autenticação/autorização não devem revelar existência de identidade, dispositivo, canal ou localização cross-tenant.

## 7. Contrato HTTP `/v1`

### 7.1 Regras gerais

- HTTPS obrigatório para API externa; validar certificado/host e identidade de emissor conforme política. QUIC não é requisito para HTTP/JSON.
- `Authorization: Bearer <access-token>` é forma HTTP proposta; para cliente público, DPoP sender-constrained conforme perfil C09, e mTLS continua alternativa para serviço-a-serviço. JWT (RFC 9068) versus token opaco/introspecção ainda requer escolha. `tenant_id` e `principal_ref` são resolvidos/validados pelo servidor; claims, body/path/query jamais substituem membership, ACL, propósito ou autorização atual.
- Todas as respostas JSON usam UTF-8. Datas em RFC 3339 UTC. IDs são UUID strings salvo quando marcados opaque. Campos desconhecidos são ignorados apenas quando não críticos e não alteram segurança/semântica.
- Mutação exige `Idempotency-Key` textual no formato UUID canônico UUIDv7 proposto; o valor é opaco para cliente e mapeia um-para-um ao `operation_id` GMP/C10. Namespace, fingerprint e regras de retry estão em §§2 e 5; idade/skew/retenção são parâmetros sem valor aprovado.
- Paginação usa `limit` com máximo publicado por perfil e cursor opaco. Cursores são vinculados ao vetor de checkpoints contíguos/epoch de C10, filtros, principal, tenant, scopes, schema e política efetiva. Cursor malformado ou de contexto incompatível retorna `400 INVALID_CURSOR`; cursor vencido pelo intervalo retido retorna `410 CURSOR_EXPIRED`, com `snapshot_required=true` quando a recuperação estiver autorizada. Revogação impede replay e não amplia acesso.
- Erros usam `application/problem+json`? **Proposta adiada**: a representação canônica é JSON `ApiError` abaixo para manter compatibilidade de cliente e mapeamento simples com `GmpError`. O documento OpenAPI deverá fixar media type e códigos HTTP antes de publicação.
- `trace_id` pode ser retornado; nunca incluir payload de localização privada, segredo, token, proof ou corpo de mensagem em logs padrão.
- Antes de persistência, replay, replicação ou exportação, o serviço resolve política C08 `effective` por `data_class`, tenant/finalidade/jurisdição e classe de cópia; `policy_ref` enviado pelo cliente é apenas referência/seletor, nunca prova de política efetiva. Se política/classe/prazo/escopo estiver ausente, falhar com `RETENTION_POLICY_MISSING` sem persistir. `policy_id/version`, `copy_class`, `retention_seconds` e autorizações são associados pelo serviço proprietário; valores seguem pendentes.

### 7.2 Schemas HTTP comuns

| Schema | Campos |
|---|---|
| `ApiError` | `code:string` obrigatório; `message:string` sanitizada; `request_id:string(UUID)` nullable se falhou antes de parse; `retry_after_ms:integer` opcional; `details:object` opcional e allow-listed. |
| `Page<T>` | `items:T[]` obrigatório; `next_cursor:string` nullable. O cursor é opaque e só serve à mesma consulta/autorização. |
| `ResourceVersion` | `resource_id:string(UUID)`; `version:string` opaque/ETag; `updated_at:string(date-time)`. |
| `CommandReceipt` | `command_id:string(UUID)` estável; `operation_id:string(UUIDv7)`; `request_fingerprint:string`; `accepted_at:string(date-time)`; `expires_at:string(date-time)`; `status:string` = lifecycle C10 (`created,persisted,routed,completed,failed,cancelled,expired`); `latest_attempt:DeliveryAttemptSummary?` com `attempt_id`, `state`, `observed_at?`, `receipt_ref?`; `authority_epoch:uint64` quando aplicável; `status_url:string` autorizada. Aceitação HTTP não significa entrega. |
| `HealthLive` | `status:enum{live}` apenas; sem dependências ou dados internos. |
| `HealthReady` | `status:enum{ready,degraded,not_ready}`; `services:object[]` com `service:string`, `state:enum{ready,degraded,not_ready}`, `reason_code:string` opcional. Acesso restrito ao operador; não contém hostnames/segredos. |

`CommandReceipt.status` contém exclusivamente `Command.lifecycle` C10. `accepted_by_gateway`, `accepted_by_network`, `confirmed_by_device`, `outcome_unknown` e `confirmation_unavailable` são estados/evidências de `DeliveryAttempt`, nunca aliases de lifecycle; `acknowledged_by_person` é `HumanAcknowledgment`; SOS `resolved` é estado de incidente. Esses campos não autorizam inferência de entrega nem reconhecimento. Um HTTP `202` só garante o estado que o recibo efetivamente reporta; não implica `persisted` se o perfil futuro não exigir commit durável antes da resposta.

### 7.3 Modelo cumulativo de capacidades (proposta)

As coleções `supported[]`, `verified[]` e `operational[]` da resposta `DeviceCapabilities` são **cumulativas**, não estados disjuntos. Cada item é um identificador namespaced de capability (por exemplo `voice.tx`), sem significado de autorização:

| Coleção | Semântica | Invariante proposta |
|---|---|---|
| `supported[]` | Capability declarada pelo perfil do dispositivo/adaptador; declaração não é evidência de funcionamento. | Pode conter capabilities ainda não verificadas. |
| `verified[]` | Subconjunto de `supported[]` cuja evidência atribuível passou validação segundo o método aplicável. | `verified ⊆ supported`; ausência de evidência válida exclui da lista. |
| `operational[]` | Subconjunto de `verified[]` observado disponível para a capability e suas dependências no contexto de avaliação indicado. | `operational ⊆ verified ⊆ supported`; observação ausente, inválida ou vencida exclui da lista operacional. |

O contrato de capacidades pode expressar o mesmo resultado por capability nos níveis ordenados `supported → verified → operational`; GMP projeta esses níveis em conjuntos cumulativos para compatibilidade com o formato já proposto. O cliente pode derivar o nível mais alto presente (operational, senão verified, senão supported), mas não pode inferir nível algum da ausência nas três listas. A ordenação descreve evidência/disponibilidade, não uma máquina persistente de transições: a avaliação pode regredir ou expirar sem remover a declaração `supported`.

`observed_at` é o instante UTC da avaliação que produziu a resposta; `conditions[]` contém códigos e contexto legíveis por máquina para limitações observadas. Um perfil versionado precisa definir janela de frescor, método/fonte de evidência e como indicar observação desconhecida. Nenhum TTL, threshold ou janela está aprovado aqui. Até a política de frescor estar configurada, ausência de observação atual verificável impede classificar a capability como operational ou anunciar prontidão com base nela.

**Separação obrigatória:** operational significa somente disponibilidade contextual observada da capability e das dependências incluídas na avaliação. Não significa identidade autenticada, autorização, rota elegível, saúde global de recurso, nem readiness de serviço/produto. Autorização continua uma decisão independente da policy/ACL; elegibilidade de rota e readiness são avaliações próprias e precisam verificar, além de capability operacional, autorização vigente, rota e dependências requeridas. Falha/ausência de qualquer condição exclui elegibilidade, mas não altera a declaração `supported` nem prova revogação.

Casos de conformidade documental:

- `supported=[voice.tx]`, `verified=[]`, `operational=[]`: declaração sem evidência validada; não elegível.
- `supported=[voice.tx]`, `verified=[voice.tx]`, `operational=[]`: capability verificada sem disponibilidade contextual atual demonstrada.
- `supported=[voice.tx]`, `verified=[voice.tx]`, `operational=[voice.tx]`: disponibilidade contextual observada; autorização e avaliação de rota continuam obrigatórias antes de um comando.
- `verified=[voice.tx]` sem `supported=[voice.tx]`, ou `operational=[voice.tx]` sem `verified=[voice.tx]`: violação de invariante; consumidor conformante rejeita como `SCHEMA_VIOLATION`.
- Evidência mais antiga que a janela publicada: a capability não permanece em `operational[]` por cache sem revalidação. Sem janela publicada, estado operacional não sustenta anúncio de prontidão.

### 7.4 Operações e schemas mínimos

Scope names são **propostos**, a ratificar pelo contrato de identidade. `tenant-context` significa contexto derivado da prova validada; nenhum endpoint aceita tenant body como autoridade. Para respostas paginadas, `Page<T>` é o schema de saída. `200` lê/resolve; `201` cria recurso sincronamente; `202` aceita processamento assíncrono sem declarar entrega; `204` só encerra ação sem corpo; `400/401/403/404/409/410/413/415/422/429/503` mapeiam aos códigos ApiError aplicáveis. A lista não substitui a resposta específica detalhada no OpenAPI.

| Método e rota | Entrada validada | Sucesso e significado | Autorização proposta / erros específicos |
|---|---|---|---|
| `GET /v1/health/live` | Sem body | `200 HealthLive`; processo responde | Pública, sem campos sensíveis; `503` quando não vivo. |
| `GET /v1/health/ready` | Sem body | `200 HealthReady`; read-only readiness | `ops.health.read`; restrita a monitor confiável; `403`. Não é health check global de todos os produtos. |
| `GET /v1/regions` | `cursor?`, `limit?` | `200 Page<Region>`: `region_id`, `eligibility`, `capabilities[]`, `reason_code?`; não divulga endpoint secreto | `route.region.discover` (C09); `400 INVALID_CURSOR`, `403`, `404` anti-enumeração. |
| `GET /v1/identity/me` | Sem body | `200 IdentitySelf`: projeção própria permitida; `gmi_id` apenas no contexto interno autorizado; memberships minimizadas | `identity.self.read`; `401 invalid_token`, `403 insufficient_scope`, `503 authority_unavailable`. |
| `GET /v1/identity/resolve?address=` | `address` `acct:` opt-in; entrada não pode conter segredo | `200 IdentityResolution`: somente projeção pública explicitamente publicada; nunca `gmi_id`, membership/tenant, dispositivos ou perfil privado; `404 not_found_or_not_visible` uniforme | Público conforme C09; rate limit/anti-enumeração; `400 invalid_request`, `404 not_found_or_not_visible`, `429 rate_limited`. Resolver público não autentica. |
| `POST /v1/identity/migration/initiate` | `Idempotency-Key`; body `target_authority:string`, `transfer_proof_ref:string`, `expected_version:string`; timestamps server-authoritative | `202 MigrationReceipt`: `migration_id`, state de migração, `expires_at`, `status_url`; não confirma migração/troca de autoridade | `identity.migrate`; prova da credencial atual e novo emissor; `401 invalid_token/invalid_proof`, `403 not_owner`, `409 migration_pending/authority_epoch_stale`, `503 authority_unavailable`. |
| `GET /v1/assets` | `cursor?`, `limit?`, `filter?` allow-listed | `200 Page<Asset>`: `asset_id`, `kind`, `display_label?`, `version`, `updated_at` | `asset.read`; filtro não amplia tenant; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`. |
| `POST /v1/assets` | `Idempotency-Key`; body `kind:string`, `display_label?`, `attributes:object` allow-listed | `201 Asset` com `asset_id`, version e timestamps | `asset.write`; validar tipos/campos permitidos; `400 invalid_request`, `401 invalid_token`, `403 insufficient_scope/not_owner`, `409 IDEMPOTENCY_CONFLICT`, `413 MESSAGE_TOO_LARGE`, `422`. |
| `GET /v1/devices` | `cursor?`, `limit?`, `asset_id?` | `200 Page<Device>`: `device_id`, `asset_id?`, `state`, `capability_summary[]`, version | `device.read`; owner/ACL; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`. |
| `POST /v1/devices` | `Idempotency-Key`; body `device_kind`, `asset_id?`, `claimed_capabilities[]`, `proof_ref?` | `201 Device` com `device_id`, `binding_state`, version; declarado ≠ verificado/operacional | `device.enroll`; prova de posse e delegação conforme C09; `400 invalid_request`, `401 invalid_proof`, `403 not_owner`, `409 IDEMPOTENCY_CONFLICT`, `422`. |
| `GET /v1/devices/{device_id}/capabilities` | UUID válido | `200 DeviceCapabilities`: coleções cumulativas §7.3, `conditions[]`, `observed_at` | `device.read`; ACL do device; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`. |
| `GET /v1/devices/{device_id}/location` | UUID válido; sem aceitar precisão via query para elevar acesso | `200 LocationObservation`: `observed_at`, `source`, `quality`, `precision_m?`, `position` redigida conforme política, `version` | `location.read`; policy/grant vigente e autorização atual; `401 invalid_token`, `403 insufficient_scope`, `404 not_found_or_not_visible`, `410` se recurso retido/revogado. |
| `GET /v1/devices/{device_id}/observations` | `cursor?`, `limit?`, `from?`, `to?`, `type?` allow-listed | `200 Page<Observation>`; filtros no escopo autorizado e retenção aplicável | `location.history.read`; `401/403/404`; `410 CURSOR_EXPIRED` ou recurso expirado, com recuperação C10 somente se autorizada. |
| `POST /v1/devices/{device_id}/commands` | `Idempotency-Key`; body `command_type`, `payload`, `expires_at`, `expected_version?`; `authority_epoch`/prova se perfil de autoridade exigir | `202 CommandReceipt`; lifecycle e tentativa separados; aceitação só indica o estágio do recibo, sem ACK de entrega | `device.command`; ACL+capability operacional+rota+policy efetiva; erros incluem `IDEMPOTENCY_CONFLICT`, `IDEMPOTENCY_KEY_EXPIRED`, `RETENTION_POLICY_MISSING`, `authority_epoch_stale`, `authority_unavailable`. Falha fechado sem autorização/policy/autoridade. |
| `POST /v1/routes/resolve` | Body `service`, `source_ref`, `destination_ref`, `constraints[]`; read-only | `200 RouteDecision`: `eligible`, `route_ref?`, `reason_codes[]`, `capability_evidence[]`, `valid_until` | `route.resolve`; saída e entrada avaliadas independentemente; `401/403/404 no_route/503`. Resolver não concede permissão permanente. |
| `GET /v1/channels` | `cursor?`, `limit?`, `filter?` | `200 Page<ChannelSummary>`: `channel_id`, `scope`, `media_profile`, version | `channel.read`; ACL antes da paginação; `401/403/404 not_found_or_not_visible`. |
| `POST /v1/channels` | `Idempotency-Key`; body `display_name`, `channel_scope`, `policy_ref`, `media_profile` | `201 Channel` com `channel_id`, version | `channel.manage`; membership/policy validada; C08 policy efetiva é resolvida pelo servidor; `401/403/409/422/RETENTION_POLICY_MISSING`. Política de país/rádio não inferida. |
| `POST /v1/channels/{channel_id}/sessions` | `Idempotency-Key`; body `client_instance_id`, `requested_media_profile`, `resume_ref?` | `201 MediaSession`: `session_id`, `media_node_ref`, `expires_at`, `negotiated_profile`; não contém áudio | `media.session.create`; ACL+admissão+capacidade; `401/403/404/409/429/503`. |
| `POST /v1/channels/{channel_id}/floor` | `Idempotency-Key`; body `session_id`, `action:enum{request,release}`, `grant_id?`, `expected_epoch?` (epoch do grant PTT) | `202 FloorReceipt`: `command_id`, `operation_id`, lifecycle receipt/status reference; grant só existe após confirmação efetiva pelo controlador | `ptt.transmit`; canal/session/ACL/grant epoch; se autoridade forte também se aplicar, `authority_epoch` permanece campo separado conforme C06; `401/403/404 not_found_or_not_visible/409 authority_epoch_stale/429/503`. Não é caminho de áudio. |
| `GET /v1/bridges` | `cursor?`, `limit?` | `200 Page<BridgeSummary>`: `bridge_id`, `state`, `authorization_scope`, `direction`, `last_evidence_at?` | `bridge.read`; operador/tenant conforme governança; `401/403/404`. |
| `POST /v1/bridges` | `Idempotency-Key`; body `adapter_type`, `authorization_ref`, `direction`, `policy_ref`, `requested_capabilities[]` | `202 BridgeReceipt`: `command_id`, `operation_id`, estado próprio de admissão bridge, `status_url`; não declara conexão habilitada | `bridge.manage`; C09 autoriza; C08 policy efetiva é resolvida pelo servidor. Erros incluem `IDEMPOTENCY_CONFLICT`/`RETENTION_POLICY_MISSING`. TX fica desabilitado até grant explícito; preservar IDs/fingerprint C10/Gateway, sem fila/estado concorrente. |
| `GET /v1/radio/services` | `cursor?`, `limit?`, `region?` | `200 Page<RadioService>`: `service_ref`, `protocol`, `region`, `observed_capabilities[]`, `authorization_state` | `external.service.read`; catálogo autorizado; descoberta não autoriza TX. |
| `POST /v1/federation/join-requests` | `Idempotency-Key`; body `node_id`, `identity_proof_ref`, `requested_scope[]`, `software_protocols[]` | `202 JoinReceipt`: `request_id`, estado de admissão C09 (`awaiting_authorization`), `status_url` | `federation.join.request`; prova de nó/autoridade; `invalid_proof`, `invitation_invalid`, `node_conflict`, `trust_state_unavailable`. Não ingressa peer automaticamente. |
| `GET /v1/federation/peers` | `cursor?`, `limit?`, `state?` | `200 Page<FederationPeer>`: `node_id`, `state`, `granted_scope[]`, `authority_epoch?`, `last_verified_at` | `federation.peer.read`; operador/federação autorizada; `404 not_found_or_not_visible`. |
| `GET /v1/events/stream` | Cursor opcional opaco; filtros de stream allow-listed; transporte/upgrade e frame format dependem de perfil | Stream de envelopes/eventos com `event_id`, origem, sequência, epoch, schema e payload autorizado; ACK só após Inbox durável conforme C10 | `events.read`; filtros não são ACL; cursor preso a checkpoints, scope e retenção; `CURSOR_EXPIRED`, `SNAPSHOT_REQUIRED`, `EVENT_AUTHORIZATION_DENIED`, `INBOX_UNAVAILABLE`. WebSocket 101 versus outro stream continua pendente. |

### 7.4.1 Rotas de identidade e federação presentes em C09, ainda não catalogadas no índice inicial

Essas rotas já aparecem no contrato C09, mas não em `api-inicial.md`; incluí-las aqui registra a proposta existente, sem criar autorização nem alegar release. Antes de um OpenAPI final, Alan deve decidir se atualiza o índice público com os donos dos arquivos. Os erros lógicos abaixo mantêm a grafia C09; API HTTP preserva token/casing e os mapeia ao status HTTP somente no artefato OpenAPI.

| Método e rota proposta em C09 | Principal/scope e resposta/negação relevantes | Relação com GMP/C10 |
|---|---|---|
| `GET /.well-known/webfinger` | Público e somente aliases `acct:` opt-in; `400 invalid_request`, `404 not_found`, `429 rate_limited`, `503 authority_unavailable` | Fora do prefixo `/v1`; alias não autentica nem revela GMI/tenant oculto. |
| `GET /.well-known/godmesh` | Público apenas para documento não privado; `404 not_found`, `503 trust_state_unavailable` | Consumidor verifica assinatura/trust; documento não concede membership. |
| `GET /v1/callsigns/{callsign}/credential` | Público somente para campos GMC publicados; `404 not_found_or_not_visible`, `429`, `503 issuer_unavailable` | Não retorna GMI nem chave privada; descoberta não autentica titular. |
| `POST /v1/identity/handles` | `identity.handle.write`; request idempotente; `401 invalid_token`, `403 not_owner`, `409 alias_unavailable`, `422 invalid_handle` | Campos de prova ficam no C09; operação mutável usa operation_id/fingerprint C10/C07. |
| `POST /v1/identity/callsigns/claim` | `identity.callsign.claim`; `401 invalid_token`, `403 not_owner`, `409 callsign_conflict`, `503 registry_unavailable` | Não promete concessão; autoridade do registro/credencial conforme C09. |
| `POST /v1/identity/migration/confirm` | Prova adicional titular antigo + novo emissor; `401 invalid_proof`, `403 migration_not_authorized`, `409 contested`, `503 authority_unavailable` | Fluxo não altera autoridade/home region antes do commit e fencing C06; idempotência C10. |
| `POST /v1/federation/attest` | `federation.node.attest`; emissor service autorizado e lease/epoch vigente; `401 invalid_token`, `403 issuer_not_authorized`, `409 stale_epoch`, `503 trust_state_unavailable` | Node admission/attestation é domínio C09; epoch segue C06. |
| `GET /v1/federation/manifests/current` | Público só se manifesto sem dados privados; senão nó confiável com `federation.peer.read`; `404`, `409 unsupported_version`, `503 trust_state_unavailable` | Manifesto assinado não concede membership; schema/proof de snapshot e replay remetem a C10. |
| `POST /v1/federation/leave` | Prova de posse do nó; `federation.join.request` ou scope específico a definir; `401 invalid_proof`, `403 not_owner`, `409 stale_epoch`, `503 authority_unavailable` | Revoga somente vínculo do próprio nó; emite evento C10, respeita C06/C08; falta definir scope dedicado. |

### 7.4.2 Coerência de proveniência UDNL e Gateway

GMP transporta `SourceProvenance` e `ProjectionReport` como tipos/versionamentos definidos em `../01-arquitetura/normalizacao-dados.md`; esta API não cria perfil alternativo. `event_id` é a identidade do evento canônico GodMesh; `origin_event_key` identifica evento externo dentro de `(tenant_scope, root_network, root_instance_id, origin_event_key)`; `message_id` só pode ser mapeado a essa chave quando a fonte garante estabilidade. Para comandos, `request_fingerprint` é hash da requisição lógica C07/C10; não é `content_digest` do evento Inbox/UDNL nem identificador de deduplicação de evento. Um evento pode manter os identificadores e digests distintos.

Em bridge, `visited_bridge_ids` é a cadeia normativa, `hop_count` derivado, `hop_limit` configurado/monotônico e TTL/`expires_at` limite temporal diferente. Ciclo, limite excedido ou continuidade desconhecida bloqueiam encaminhamento externo encadeado; os erros `BRIDGE_LOOP_DETECTED`/`BRIDGE_HOP_LIMIT_EXCEEDED` não significam autorização negada. Payload projetado deve seguir saída/entrada autorizadas, minimização e relato de perda `ProjectionReport`; não enviar localização precisa ou credenciais em metadados propagados.

Tipos `Asset`, `Device`, `Observation`, `Channel`, `Bridge`, `Region`, `FederationPeer`, `IdentityResolution` e recibos devem ter JSON Schema fechado para campos críticos e política `additionalProperties` explícita no OpenAPI. `attributes`/`payload` são extensões namespaced com limite e allow-list, não bag arbitrária para contornar schema ou controles de tenant.

### 7.5 Semântica de status HTTP

| HTTP | Uso | Significado limitado |
|---|---|---|
| `200` | consulta ou resolução | resposta de leitura concluída na autoridade consultada; dados podem ser frescos até `observed_at`, nunca promete globalidade. |
| `201` | criação síncrona de recurso | recurso criado/persistido na autoridade local; não afirma replicação global. |
| `202` | comando, solicitação de migração/ingresso/bridge ou ação assíncrona | solicitação aceita para processamento; estado consultável; não é `delivered`, `completed` ou ACK humano. |
| `204` | operação idempotente sem representação, se posteriormente definido | sem payload; não utilizar para ocultar resultado assíncrono. |
| `400/422` | erro sintático/semântico | schema inválido ou regra de campo violada; sem efeito parcial. |
| `401/403` | autenticação/autorização | sem informação que permita enumeração cross-tenant. |
| `404` | não encontrado ou ocultado por política | mesma forma observável para recurso inacessível quando anti-enumeração exigir. |
| `409` | versão/epoch/idempotência conflituosa | retornar código estável; não aplicar efeito parcial. |
| `410` | cursor/recurso expirado ou revogado | requer consulta/fluxo renovado com autorização atual. |
| `413/415` | tamanho/media type inválido | rejeitar antes de processamento/armazenamento. |
| `429/503` | quota/serviço indisponível | `Retry-After` opcional de perfil; retry conserva key dentro do prazo. |

## 8. Falhas, concorrência e consistência

- **Timeout após envio:** resultado é desconhecido para cliente; repetir somente com a mesma chave e consultar status. Não criar novo command ID para “tentar de novo” sem decisão do cliente/contrato.
- **Falha antes da persistência:** serviço responde erro transitório e não emite recibo de aceitação. Prova de persistência e recuperação são definidas pelo contrato eventos/sync.
- **Falha após persistência:** responde/retorna status pendente quando possível; retry com chave não duplica dentro da janela compartilhada. Não forjar `confirmed_by_device`/entrega.
- **Partição/perda de autoridade:** operações que exigem autoridade consistente falham `UNAVAILABLE` ou `CONFLICT`; somente operações explicitamente locais e autorizadas podem continuar. C07 não escolhe home-region, epoch ou política de transferência.
- **Revogação concorrente:** revalidar credencial, grant, ACL e authority no ponto de aplicação/replay. Se estado de revogação não pode ser consultado conforme política, negar efeito protegido.
- **Backpressure:** perfis fixam limites e Retry-After; filas por tenant/serviço não podem consumir recurso reservado sem autorização de prioridade. Valores/SLO ficam pendentes de decisão e medição.
- **Compatibilidade de schema:** mixed-version rollout exige N/N-1 compatível conforme estratégia tecnológica e contract tests; versões sem interseção, field critical unknown ou downgrade não negociado falham fechado.
- **Ordenação:** API não promete ordem global. Evento, comando por recurso e cursor seguem contrato de eventos/sync; leitura retorna versão/cursor/proveniência quando pertinente.
- **Privacidade:** payloads/sigilos não aparecem em `ApiError`, traces, métricas ou logs padrão. Classes e retenção seguem contrato de retenção/revogação.

## 9. Exemplos HTTP

### Positivo: solicitação assíncrona de comando

```http
POST /v1/devices/019a0000-0000-7000-8000-000000000031/commands HTTP/1.1
Authorization: Bearer <credencial-de-exemplo>
Idempotency-Key: 019a0000-0000-7000-8000-000000000041
Content-Type: application/json

{"command_type":"device.ping","payload":{},"expires_at":"2026-11-15T15:00:30Z"}
```

Resposta ilustrativa `202 Accepted`: `CommandReceipt` com status inicial que eventos/sync define. O corpo acima é fictício; `device.ping` só pode ser aceito se capability, ACL, rota, prazo e política reais permitirem.

### Negativo: tenant enviado no body não eleva acesso

```http
POST /v1/devices/019a0000-0000-7000-8000-000000000031/commands HTTP/1.1
Idempotency-Key: 019a0000-0000-7000-8000-000000000042
Content-Type: application/json

{"tenant_id":"tenant-de-outra-pessoa","command_type":"unlock","payload":{},"expires_at":"2026-11-15T15:00:30Z"}
```

Se o token não tem vínculo/ACL para o recurso, responder `403 PERMISSION_DENIED` ou `404` anti-enumeração conforme política, sem procurar nem executar o comando. `tenant_id` não é campo autorizado no schema de criação.

### Negativo: versão ou feature crítica desconhecida

Um peer que não suporta o major ofertado ou uma feature obrigatória recebe erro de negociação e encerra antes de aceitar `body`. Não tentar usar `gmp.media.v1` ou bridge de terceiros como downgrade automático.

## 10. Casos de conformidade documental

Estes casos definem o que um futuro teste precisa provar; não foram executados neste trabalho.

1. Cliente e servidor selecionam somente major/minor comum; major incompatível falha explicitamente; recurso opcional desconhecido é ignorado apenas quando não muda semântica crítica.
2. Feature obrigatória não suportada e enum crítico desconhecido falham antes de qualquer efeito persistente.
3. Mensagem com prefixo truncado, length excessivo, campos excessivos, timestamp inválido ou versão divergente é rejeitada sem alocação/processamento ilimitado.
4. Tenant indicado no body/path que não corresponde ao token/ACL recebe negação e não cria side effect nem enumera recurso de outro tenant.
5. `message_id` repetido e `operation_id` repetido são testados separadamente; repetição legítima só devolve resultado estável conforme a regra compartilhada, e mesma key com hash diferente gera conflito.
6. Request mutável em 0-RTT é rejeitado/adiado até handshake completo; não há profile de 0-RTT aprovado nesta entrega.
7. Timeout após envio preserva a mesma key na repetição; o cliente consulta recibo/status e nunca infere entrega a partir de `202` ou publicação em rede.
8. Falha de autoridade, epoch desatualizado, grant revogado e partição geram erro/status explícito e nenhum efeito proibido.
9. Cursor com outro tenant, filtro ou versão é recusado; cursor reapresentado após revogação não amplia leitura.
10. Endpoints de localização não revelam precisão sem policy/grant; `ApiError`, logs e trace não contêm credencial, posição ou payload sensível.
11. Schema de cada operação valida required/optional, nulidade, enum, bounds do perfil, media type e resposta de sucesso/erro.
12. Clientes dos perfis de compatibilidade aprovados passam canary/rollback e comunicação com versão N/N-1 antes de qualquer anúncio interoperável.
13. Redelivery de comando serial no mesmo escopo conserva `dispatch_sequence` e fingerprint, mas usa novo `attempt_id`; não repete efeito com resultado anterior desconhecido sem consulta/dedupe.
14. `n+1` recebido antes de `n` fica sem efeito e não avança posição; a lacuna resolvida libera reavaliação sob autorização/expiração/epoch vigentes.
15. Repetição da mesma posição com mesma identidade retorna estado conhecido; posição reusada com outro comando/operação/fingerprint falha `DISPATCH_SEQUENCE_CONFLICT`; escopo obsoleto falha fechado.
16. Alterar apenas `dispatch_scope_ref`/`dispatch_sequence` não altera fingerprint; alterar o alvo semântico ou payload que muda o efeito altera digest conforme os vetores futuros de canonicalização.

## 11. Dependências e decisões não fechadas

| Item | Responsável principal | Bloqueio atual |
|---|---|---|
| Semântica única de IDs, status/ACK, idempotência, cursor, replay, inbox e retenção | Martin Kleppmann (`eventos-sincronizacao.md`) | Não definir estados ou janela paralela aqui; alinhar o mapa de campos/API com essa fonte antes da versão executável. |
| Sujeito, claims/scopes definitivos, prova e revogação | Kim Cameron (`identidade-autorizacao.md`) | Nomes de scopes e fluxo auth aqui são propostas, não autorização implementável. |
| Autoridade de escrita/home region/epoch/failover | Leslie Lamport (`../01-arquitetura/autoridade-escrita.md`) | Não ativar mutação federada até autoridade e comportamento em partição serem ratificados. |
| Retenção por classe, cópia, tombstone e conteúdo revogado | Bruce Schneier (`../04-seguranca/retencao-revogacao.md`) | Sem prazo inventado; exigir parâmetros antes de operação real. |
| Política e tradução UDNL/bridge | Gregor Hohpe (UDNL e contratos de bridge) | Preservar identidade/proveniência/confiança e perda declarada; não traduzir identidade não suportada. |
| Fronteiras de módulo e vocabulário | Erich Gamma (fronteiras/glossário) | `principal`, `tenant`, `operation`, `feature`, `profile`, `capability` devem usar vocabulário comum. |
| OAS, ALPN, profiles de interoperabilidade e publicação do contrato | Alan Turing coordena; decisão final é do usuário para escolhas de produto/operacionais | OAS 3.2.1, `godmesh-gmp/1`, caps, media types e rollout permanecem propostas sujeitas a revisão e validação. |
| Países, licenças, hardware, carga, SLO/RPO/RTO, retenção/atendimento | Usuário / owners pertinentes | Não há aprovação/evidência; impedir prontidão/uso operacional dependente desses parâmetros. |

## 12. Referências técnicas oficiais consultadas

Consulta em **2026-11-15**. Fontes sustentam apenas o comportamento normativo citado; não comprovam implementação, disponibilidade de biblioteca ou compatibilidade de SDK GodMesh.

| Fonte oficial | Versão/escopo e trecho de apoio | Limite para esta proposta |
|---|---|---|
| [OpenAPI Specification v3.2.1](https://spec.openapis.org/oas/v3.2.1.html) | Versão publicada em 2026-09-10; página oficial consultada em 2026-11-15 define OpenAPI como descrição de interface HTTP independente de linguagem e lista histórico com 3.1.2. | A especificação define formato, não escolha para GodMesh. OAS 3.2.1 é recomendação, aguardando aprovação e avaliação de ferramentas/SDKs. |
| [Protocol Buffers — proto3 Language Guide](https://protobuf.dev/programming-guides/proto3/) e [Field Presence](https://protobuf.dev/programming-guides/field_presence/) | Docs oficiais consultadas em 2026-11-15: `optional` é recomendado para presença explícita em scalar proto3; a presença implícita confunde valor default com campo ausente. | Guia descreve formato/runtime; não escolhe schema GodMesh nem comprova geração multi-linguagem. |
| [RFC 9001 — Using TLS to Secure QUIC](https://www.rfc-editor.org/rfc/rfc9001.html) | RFC de maio de 2021; seção 5.6 requer que protocolo de aplicação sobre QUIC defina uso aceitável de 0-RTT e medidas contra replay; também especifica TLS 1.3 para QUIC. | Sustenta a regra conservadora proposta de não enviar dados de aplicação em 0-RTT sem perfil; não prova replay protection de GMP. |
| [RFC 9308 — Applicability of the QUIC Transport Protocol](https://www.rfc-editor.org/rfc/rfc9308.html) | RFC de setembro de 2022; recomenda análise cuidadosa do que é enviado em 0-RTT e nota risco de múltiplas cópias/replay. | Não escolhe política de retries ou idempotência de domínio; isso permanece com eventos/sync. |
| [RFC 9000 — QUIC Transport](https://www.rfc-editor.org/rfc/rfc9000.html) | RFC de maio de 2021, QUIC v1; descreve negociação da versão de transporte e separação do transporte seguro. | Versão QUIC não substitui negociação/versionamento da aplicação GMP. |

