# Eventos e sincronização — contrato transversal proposto

> **Status:** proposta documental v0.2, preparada em 2026-11-15 para revisão; revisão de alinhamento a ADR-057 em 2026-11-17. Não é ADR aprovada, política de retenção, formato wire final, OpenAPI/Protobuf compilado, nem evidência de implementação. As propostas de sequência por stream, epoch no cursor e UUIDv7 para chave idempotente exigem aprovação e integração. Não há países, licença, hardware, volume, SLO/RPO/RTO ou prazos de retenção aprovados.

## 1. Autoridade, escopo e termos

Este é o contrato canônico para identidade de evento, ordem, checkpoint, replay, idempotência de comando e recebimento durável. `02-dominio/eventos.md` mantém os tipos e significados de domínio; `01-arquitetura/replicacao.md` descreve classes e fluxo. [GMP/API v1](gmp-api-v1.md) mapeia o contrato ao wire e aos erros; [identidade/autorização](identidade-autorizacao.md) define principal e grants; [autoridade de escrita](../01-arquitetura/autoridade-escrita.md) define epoch e prova de autoridade; [retenção/revogação](../04-seguranca/retencao-revogacao.md) define política por classe e cópia; [UDNL](../01-arquitetura/normalizacao-dados.md) define proveniência e dedupe de origem externa. Fronteiras de módulo pertencem ao documento de fronteiras. Esses documentos mantêm seus próprios domínios; este contrato não os substitui.

`event` é fato imutável ocorrido; `command` é pedido de efeito; `delivery attempt` é tentativa de transportar um comando; `projection` é estado materializado derivado; `checkpoint` é o maior prefixo contíguo duravelmente recebido; `cursor` é token opaco para retomar uma consulta autorizada. Não há garantia de exactly-once entre processos ou redes.

## 2. Estados de comando e evidências

### 2.1 Dimensões independentes

| Registro | Campo / tipo | Valores propostos | Autoridade e significado |
|---|---|---|---|
| `Command` | `lifecycle: enum` | `created`, `persisted`, `routed`, `completed`, `failed`, `cancelled`, `expired` | Serviço de comando registra o ciclo de vida lógico. Terminais: `completed`, `failed`, `cancelled`, `expired`. |
| `DeliveryAttempt` | `state: enum` | `queued`, `submitted_to_adapter`, `accepted_by_gateway`, `accepted_by_network`, `confirmed_by_device`, `outcome_unknown`, `confirmation_unavailable`, `rejected`, `failed`, `expired` | Adaptador/gateway registra evidência de sua etapa, com `attempt_id`, horário, protocolo e recibo opcional. Estado de tentativa não muda o lifecycle sozinho sem regra do comando. |
| `HumanAcknowledgment` | `state: enum` | `not_requested`, `pending`, `acknowledged`, `unavailable` | Serviço de domínio registra reconhecimento de pessoa autenticada, com `principal_ref` e prova/auditoria conforme política. Nunca inferido do ACK de rede. |

Proposta de campos comuns: `command_id` identificador estável; `operation_id` chave lógica idempotente; `tenant_id` e `resource_ref` escopados; `created_at` UTC; `expires_at` UTC ou nulo somente se a operação admitir duração ilimitada; `lifecycle`; `attempts[]`; `human_acknowledgment`; `version` inteiro monotônico da revisão de estado; `evidence_refs[]`. Campos wire e códigos finais são responsabilidade de C07.

### 2.2 Transições e crosswalk

O lifecycle segue `created → persisted → routed → terminal`. `created` pode terminar em `failed`, `cancelled` ou `expired` antes de persistir. `persisted` pode terminar em falha/cancelamento/expiração sem rota. `routed` só pode se tornar `completed` se o predicado de sucesso próprio da operação for provado; caso contrário termina em `failed`, `cancelled` ou `expired`. Um estado terminal não regride nem muda para outro terminal; correção é novo comando/evento relacionado. `version` aumenta em cada mudança persistida.

O estado por tentativa é append-only como histórico de fatos. Uma nova tentativa recebe novo `attempt_id` e mantém o mesmo `command_id`/`operation_id` apenas se representa o mesmo efeito lógico e a mesma requisição. Perda de resposta após envio vira `outcome_unknown`; consulta de status/recibo precede qualquer decisão de criar novo efeito. Se o protocolo não fornece confirmação final, registrar `confirmation_unavailable`; não fabricar `confirmed_by_device`.

| Vocabulário já publicado | Mapeamento nesta proposta | Prova mínima / ressalva |
|---|---|---|
| `created`, `persisted`, `routed` (API) | `Command.lifecycle` | Commit/registro do serviço na etapa respectiva. |
| `queued`, `submitted_to_adapter` (eventos) | `DeliveryAttempt.state` | Registro durável da fila/tentativa e da submissão ao adaptador. |
| `accepted_by_gateway` | tentativa aceita pelo gateway | Recibo autenticado do gateway; não prova recepção pelo dispositivo. |
| `accepted_by_network` | tentativa aceita pela rede | Confirmação definida pelo protocolo da rede; não prova recepção/apresentação ao dispositivo. |
| `confirmed_by_device` | tentativa confirmada pelo dispositivo | Recibo autenticado e vinculado ao comando/dispositivo; quando não suportado, usar `confirmation_unavailable`. |
| `acknowledged_by_person` | `HumanAcknowledgment.acknowledged` | Ação autenticada de pessoa identificada e autorizada, ligada ao incidente/comando. |
| `completed` | terminal do comando | Predicado de sucesso próprio do comando e evidência registrada; não equivale universalmente a entrega. |
| `resolved` | estado do incidente SOS | Mantido em máquina de incidente separada; não é alias de `completed`. |
| `delivered` | projeção derivada específica de domínio | Só publicar se definição e prova forem aprovadas pelo domínio; publicação MQTT ou ACK de gateway não bastam. |

### 2.3 Casos positivos e negativos

- **Positivo:** a rede aceita uma mensagem e emite recibo: tentativa `accepted_by_network`; comando continua `routed` até o sucesso de domínio ser demonstrado.
- **Positivo:** dispositivo fornece recibo autenticado vinculado ao `command_id`: tentativa `confirmed_by_device`; isso ainda não cria reconhecimento humano.
- **Negativo:** gateway responde sucesso de publicação MQTT: não declarar recepção no terminal, `delivered`, `acknowledged_by_person` ou `completed` sem predicado adicional.
- **Negativo:** SOS recebe claim humano: atualizar `HumanAcknowledgment`; não mudar incidente para `resolved` sem fluxo próprio.
- **Negativo:** timeout sem recibo: registrar `outcome_unknown`, consultar estado com a mesma operação idempotente; nunca reemitir como novo comando por simples timeout.

## 3. Idempotência de comandos e deduplicação de eventos

### 3.1 Chave idempotente de operação

| Campo lógico | Tipo/presença | Regra proposta |
|---|---|---|
| `operation_id` | UUIDv7; obrigatório em comando mutável | Chave opaca ao cliente; autenticação e autorização continuam separadas. C07 mapeia ao `Idempotency-Key` HTTP e ao campo GMP. |
| `tenant_id` | identificador de tenant derivado do principal | Namespace do servidor; valor enviado pelo chamador não concede acesso. |
| `principal_ref` | referência opaca ao principal autenticado, obrigatória | Derivada do contexto validado; não aceitar como prova dentro do body. |
| `operation_kind` | enum/string namespaced obrigatório | Define classe semântica do comando, não endpoint incidental. |
| `resource_ref` | referência tipada/escopada, obrigatória quando comando age sobre recurso | Evita colisão entre recursos; nulo apenas em operação global expressamente definida. |
| `request_fingerprint` | digest de representação canônica e versionada, obrigatório | Compara o pedido lógico. Algoritmo, canonicalização e forma wire devem ser fixados por C07; até isso ocorrer, não há interoperabilidade alegada. |
| `policy_ref` | ID/versão de política de retenção, obrigatório para readiness | Define quanto tempo a chave, resultado/tombstone e resposta podem ser consultados; não contém duração escolhida aqui. |

Namespace conceitual: `(tenant_id, principal_ref, operation_kind, resource_ref, operation_id)`. O servidor primeiro autentica e autoriza a operação atual, depois consulta a chave dentro desse namespace. Não revelar resultado de outro principal/tenant. Resultado terminal ou em andamento é a mesma operação, não autorização permanente.

**Proposta UUIDv7:** recomendada porque codifica timestamp Unix nos 48 bits iniciais, permitindo rejeitar uma chave *desconhecida* cuja idade exceda limite configurado mesmo após descarte do registro. Alternativa A: chave opaca sem timestamp e tombstone de dedupe retido indefinidamente (custo de armazenamento e política). Alternativa B: UUIDv7 e registro/tombstone com janela finita (exige tolerância de relógio e decisão de prazo; após expirar, chave antiga desconhecida é recusada, nunca reexecutada). UUIDv7 não autentica solicitante nem prova horário confiável. RFC 9562, §5.7, “Universally Unique IDentifiers (UUIDs)”, maio de 2024, consultada em 2026-11-15: https://www.rfc-editor.org/rfc/rfc9562.html. Limite da fonte: define formato UUIDv7, não semântica idempotente, segurança da chave, skew, retenção ou política GodMesh.

`max_key_age`, `max_future_skew` e `idempotency_record_retention` são parâmetros obrigatórios por operação/tenant. Nenhum valor foi aprovado. Ausência de política eficaz bloqueia prontidão para aceitar efeito mutável federado.

### 3.2 Decisão determinística para retry

1. Chave conhecida + mesmo namespace + fingerprint igual: retornar a mesma operação/recibo/estado conhecido; não criar segundo efeito. Revalidar autorização antes de revelar resposta.
2. Chave conhecida + fingerprint diferente: `IDEMPOTENCY_CONFLICT`; nenhum efeito adicional.
3. Chave desconhecida dentro da política de idade e autorização válida: criar uma operação e reservar a chave atomicamente com persistência do comando/outbox.
4. Chave desconhecida além de `max_key_age`: `IDEMPOTENCY_KEY_EXPIRED`; não executar, mesmo que o registro anterior tenha sido removido.
5. Timestamp além de `max_future_skew`, UUID inválido ou política ausente: `IDEMPOTENCY_KEY_TIME_INVALID` ou `IDEMPOTENCY_POLICY_UNAVAILABLE`; negar sem efeito.
6. Timeout após submissão: cliente consulta com a mesma chave; repetir mesma requisição retorna recibo/estado existente. Não interpretar timeout como falha terminal.
7. Resposta original indisponível por retenção: retornar `IDEMPOTENCY_RESULT_EXPIRED` com referência segura ao estado mínimo, se policy permitir; nunca executar a chave antiga novamente.

### 3.3 Identidade e replay de eventos

`event_id` identifica o evento lógico e sua redelivery; não é `operation_id`. Evento de origem externa usa também chave de origem escopada conforme UDNL (`tenant_scope`, rede/instância de origem, `origin_event_key`; `tenant_scope` do UDNL equivale a `tenant_id` na posição canônica da §4.1); a chave só é estável se a fonte garantir essa propriedade. Dedupe de evento usa a identidade completa + hash do conteúdo canônico:

- mesma identidade + mesmo hash: duplicata; registrar/confirmar sem reaplicar efeito;
- mesma identidade ou sequência + hash diferente: `EVENT_IDENTITY_CONFLICT`; quarentena/alerta, sem sobrescrever;
- identidade nova, sequência esperada: aceitar para Inbox;
- sequência futura com lacuna: persistir como pendente, solicitar intervalo ausente e não avançar checkpoint;
- evento expirado, epoch inválido, origem não autorizada ou assinatura inválida: rejeitar/quarentenar conforme política, nunca aplicar.

Replay reproduz evento imutável e não executa novamente o comando que o originou. Mensagens e mídia podem exigir políticas de projeção próprias; áudio em tempo real não passa a ser log durável genérico.

## 4. Sequência, checkpoint e cursor

### 4.1 Namespace de sequência — proposta

**Posição canônica (ADR-057):** a tupla ordenada `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`, com `partition` e `authority_epoch` obrigatórios (`decisoes.md:133`). Esta é a única posição válida para evento, checkpoint, snapshot, cursor e Inbox neste contrato.

**Mapeamento de vocabulário (v0.1 → v0.2):** onde a v0.1 grafava `tenant_scope` referindo-se ao namespace de tenant, lê-se `tenant_id`; onde grafava `stream`, lê-se `stream_id`; `origin_sequence` corresponde a `seq`. São renomeações do mesmo conceito, não dimensões novas. A v0.1 não tinha componente de partição — ausência que a ADR-057 elimina. `tenant_scope` permanece apenas onde nomeia a chave de proveniência do UDNL (§3.3), e ali mapeia para `tenant_id` na posição canônica.

Recomendação proposta: sequência `seq` inteira positiva e estritamente crescente por `(origin_node_id, tenant_id, stream_id, partition, authority_epoch)`. Ela é contígua dentro desse namespace. Não há ordenação total entre streams; `occurred_at` é metadado de evento e nunca ordenação autoritativa. Isso preserva mensagens com ordem própria por conversa sem impor uma sequência global de origem. Alternativa: contador global por origem; simplifica ordenação entre streams, mas serializa fluxos independentes e amplia o domínio de bloqueio em caso de perda. A recomendação requer revisão de compatibilidade de C11 (modelo/persistência), C07 (API/cursor) e C06 (epoch/handoff).

`(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` é a chave de unicidade proposta. Repetição da mesma chave com mesmo `event_id`/hash é duplicata; com conteúdo diferente é conflito. `partition` e `authority_epoch` são obrigatórios (ADR-057, `decisoes.md:133`); um perfil que não possa fornecê-los não produz posição válida e falha fechado até decisão de C06/C11 — não há namespace “sem epoch” nem mistura tolerada entre chaves com e sem epoch.

#### 4.1.1 Conciliação documental com a unicidade do modelo

O SQL ilustrativo atual de [modelo de dados](../02-dominio/modelo-dados.md) apresenta índices por origem, tenant, stream, epoch quando aplicável e sequência, coerentes com a recomendação desta seção. O exemplo anterior com `UNIQUE(origin_node_id, origin_sequence)` pressupõe sequência global por nó e permanece referência histórica e alternativa de projeto. A ilustração atual não comprova migração, esquema implantado nem aprovação do perfil. Para a proposta por stream, a posição inclui `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`; `event_id` e hash canônico ajudam a identificar a repetição, mas não substituem os componentes do namespace.

Exemplo ilustrativo (partição incluída, conforme ADR-057): nó N, tenant T1, stream `chat/room-7`, partição p1, epoch 4, seq 12; mesmo nó/tenant/stream/epoch, partição p2, seq 12; nó N, tenant T1, stream `presence`, p1, epoch 4, seq 12; nó N, tenant T2, `chat/room-7`, p1, epoch 4, seq 12; nó N, tenant T1, `chat/room-7`, p1, epoch 5, seq 12. São cinco chaves distintas e válidas sob esta proposta. Repetir a primeira tupla com mesmo `event_id`/hash é duplicata idempotente; mesma tupla com outro `event_id` ou hash divergente é conflito/quarentena, nunca sobrescrita.

As ilustrações anteriores desta seção, grafadas sem `partition` e com `tenant_scope`/`stream`, são **históricas e não normativas**; nenhuma delas define posição aceitável. Um cursor ou checkpoint derivado delas — sem partição — é **rejeitado**, nunca traduzido (§4.2).

**Decisão e integração pendentes:** o modelo atual ilustra a opção (B), sequência por stream/tenant/epoch com checkpoint namespaceado; a opção (A), sequência global por origem, permanece alternativa com domínio de serialização mais amplo. Sua compatibilidade com a regra de [replicação](../01-arquitetura/replicacao.md) de sequências independentes por stream precisa ser conciliada: alocação física e ordem lógica por stream não são aqui consideradas automaticamente equivalentes. A escolha final, vínculo de stream à partição de autoridade, semântica de epoch ausente, compatibilidade, versão e eventual migração requerem decisão e revisão conjuntas. Os índices ilustrativos, isoladamente, não provam contiguidade, handoff ou exclusão de mistura de perfis. Os exemplos não comprovam banco alterado nem migração aplicada.

A `dispatch_sequence` de C15 é outro domínio: escopo local de alvo/bridge/epoch, serve à ordenação de comandos externos por destino, e não identifica evento nem alimenta cursor/checkpoint de sync. Exemplo: dispatch 8 no alvo A e dispatch 8 no alvo B não colidem; nenhum deles ocupa a posição 8 de stream de eventos. Integração wire e erro de lacuna permanecem pendentes em C07.

### 4.1.2 Ordenação local de dispatch para bridges (integração C07/C15; proposta)

C07 GMP/API v1 §4.5 propõe `dispatch_scope_ref` opaco e `dispatch_sequence:uint64` positiva para comando de bridge `per_target_serial`; os dois campos são atribuídos uma vez por comando lógico, ficam fora de `request_fingerprint`, e redelivery conserva sequência e recebe novo `attempt_id` (GMP §4.5; fingerprint §5.1). Esta seção registra o alinhamento semântico de sync proposto por C10, sem ratificar wire/GMP nem implantação.

`dispatch_scope_ref` identifica um escopo local vinculado a bridge, alvo e geração/epoch local de dispatch. `dispatch_epoch` nomeia aqui essa dimensão semântica; ainda não é estabelecido como campo wire separado de `dispatch_scope_ref`. Essa geração local pertence ao dispatcher/alvo e não é `authority_epoch`, que representa autoridade de escrita C06. O dispatcher não pode derivar nem substituir um pelo outro. Uma operação pode carregar ambos quando necessária: o destino valida autoridade de escrita para a mutação e valida o escopo/epoch local para a ordem de envio. Streams e checkpoints de evento não usam `dispatch_sequence`.

| Regra proposta | Comportamento |
|---|---|
| Atribuição | Para comando `per_target_serial`, atribuir uma posição positiva uma única vez por `command_id` lógico dentro de `dispatch_scope_ref`. Ordem independente sem requisito serial omite ambos os campos. |
| Redelivery | Preservar `command_id`, `operation_id`, `request_fingerprint`, `dispatch_scope_ref` e `dispatch_sequence`; criar `attempt_id` novo. Não repetir efeito de resultado desconhecido sem consulta/recuperação do estado. |
| Fingerprint | `dispatch_scope_ref` e `dispatch_sequence` não entram no fingerprint C07 §5.1. Alvo semântico, operação, deadline e payload que alteram o efeito continuam compondo o pedido canônico conforme C07. |
| Lacuna | Se chega `n+1` antes de `n`, persistir/observar como pendente e não executar `n+1`, avançar watermark local nem declarar dispatch concluído. Solicitar recuperação/reentrega de `n`; depois revalidar autorização, deadline, `authority_epoch`, lease/fencing e dedupe antes de qualquer efeito. |
| Duplicata/conflito | Mesma posição e mesma identidade lógica/fingerprint retorna estado conhecido sem novo efeito. Posição já vinculada a identidade/fingerprint diferente falha fechado; preservar registro anterior. |
| Escopo obsoleto | `dispatch_scope_ref` de geração/alvo/bridge não vigente falha fechado; não migrar sequência antiga silenciosamente. Novo escopo/posição só após consultar resultado anterior, confirmar que não haverá duplicação e revalidar autorização. Detalhes de roteamento permanecem com C15/C07. |

`DISPATCH_SEQUENCE_GAP`, `DISPATCH_SEQUENCE_CONFLICT` e `DISPATCH_SCOPE_STALE` são nomes candidatos em C07 §6, não erros wire ratificados. C10 propõe mapear gap para recuperação da posição sem efeito, conflito para não retry automático e escopo obsoleto para reavaliação/roteamento seguro; C07/C15 devem confirmar códigos e semântica de retry. São distintos de `SEQUENCE_GAP` (ordem de evento) e `authority_epoch_stale`/`authority_unavailable` (autoridade de escrita).

Exemplos de conformidade: (a) comando lógico 8 reentregue mantém posição 8 e fingerprint, mas ganha novo `attempt_id`; (b) posição 9 recebida antes da 8 fica pendente e não produz efeito; ao recuperar 8, revalidar todas as autorizações antes de liberar 9; (c) posição 8 repetida com outro `command_id` falha como conflito; (d) escopo de epoch local antigo não é aceito mesmo quando o `authority_epoch` de dados continua válido; (e) `authority_epoch` obsoleto falha mesmo se a sequência local de dispatch é atual. Estes são critérios documentais, não testes executados.

**Status:** proposta de alinhamento semântico C10. O ciclo de vida local está detalhado em §4.1.3 e cruzado com `DeliveryAttempt.state` e `Command.lifecycle`; continuam pendentes a aprovação do vocabulário/códigos e o mapeamento wire/retry de C07. A proposta não afirma interoperabilidade nem execução.

## 4.2 Checkpoint e cursor

Checkpoint interno: tupla lógica `(authorized_scope_ref, origin_node_id, tenant_id, stream_id, partition, authority_epoch, last_contiguous_seq)`, além de `schema_profile` e `updated_at`. `partition` e `authority_epoch` são obrigatórios; checkpoint sem partição é inválido e não substitui o valor anterior. Checkpoint só avança depois que cada evento anterior nesse namespace está duravelmente no Inbox. Vector de streams/origens é múltiplos checkpoints, não um máximo escalar.

Cursor de API/wire é opaco e autenticado pelo servidor; contém ou referencia a versão do cursor e a tupla/vector de checkpoints na posição canônica da §4.1, filtros, escopo de autorização, versão de schema e política de retenção. Cliente não constrói, decodifica, amplia escopo nem usa cursor em outro principal, stream, partição ou filtro. **Cursor sem partição ou de epoch incomparável é rejeitado** (`decisoes.md:133`), nunca traduzido, completado por inferência nem aceito como cursor legado válido. Não existe caminho de compatibilidade que preencha a partição ausente: a resposta é rejeição explícita, e a retomada exige cursor novo ou snapshot autorizado (§5). C07 define codificação GMP/HTTP e o erro de protocolo. Mudança de autorização não permite replay de dados sem autorização atual.

| Entrada | Resultado normativo proposto |
|---|---|
| Evento seq 10 recebido, checkpoint 9 | Persistir inbox e avançar para 10. |
| Evento seq 12 recebido antes do 11 | Persistir pendente, manter checkpoint 10, pedir 11. |
| Mesmo evento 12 redeliverido com mesmo hash | Deduplicar sem reaplicar; cursor permanece 10 até preencher 11. |
| Evento 12 com identidade igual e hash diferente | `EVENT_IDENTITY_CONFLICT`; não substituir nem avançar. |
| Snapshot até 50 e replay | Validar manifest/digest/escopo; só aplicar seq > 50 no namespace correspondente. |
| Cursor sem `partition` ou sem versão reconhecida | Rejeitar o cursor; nenhum checkpoint é aceito por inferência e nenhum evento é pulado. Exigir cursor novo ou snapshot autorizado (§5). |
| Cursor de epoch incomparável (epoch anterior, posterior ou não comparável) | Rejeitar; nunca traduzir para o epoch vigente nem inferir continuação. A retomada depende do handoff da linha seguinte. |
| Autoridade troca epoch | Nunca comparar `seq` do epoch novo como continuação implícita do antigo. Exigir handoff autenticado que declare o último prefixo de cada stream/partição do epoch anterior; até C06/C07/C11 fixarem wire e persistência, transição fica em prontidão bloqueada. |

Snapshot manifest conceitual obrigatório: `snapshot_id`, `tenant_id`, `origin_node_id`, `stream_id`, `partition`, `authority_epoch` obrigatório e declarado, `last_contiguous_seq`, `schema_profile`, `chunk_count`, `content_digest`, `authority_proof_ref`, `retention_policy_ref`. Snapshot é estado consistente até aquele checkpoint; carregar snapshot parcial ou sem prova/digest não avança checkpoint. A especificação criptográfica/assinatura permanece no GMP/identidade.

## 5. Outbox, replay e retenção

Origem persiste mutação local, alocação de sequência e evento Outbox na mesma transação local. Publicação é pelo menos uma vez; retentativas mantêm evento/seq/hash idênticos. Falha após enviar mas antes do ACK pode duplicar transporte; dedupe no receptor torna repetição segura, sem prometer exactly-once de rede.

Retenção usa referência a `data_class`, `policy_id/version`, `copy_class`, `retention_seconds` e escopo/residência do contrato de C08. Classes de cópia abrangem, conforme perfil aplicável, log, Outbox, Inbox, lacunas pendentes, snapshots, caches, backups e exportações. `null`/ausente significa política não configurada, não infinito: cópia/replay persistente fica bloqueada até política eficaz. Este documento não escolhe jurisdição, base legal, prazo, retenção de tombstone ou garantia offline.

Cursor além do intervalo retido retorna erro de cursor expirado (`CURSOR_EXPIRED` ou mapeamento GMP equivalente) com `snapshot_required=true` e limite inicial seguro, se autorizado. Cursor sem partição ou de epoch incomparável é rejeitado, não mapeado para `CURSOR_EXPIRED`: são causas distintas e nenhuma delas autoriza avanço de checkpoint. Não pular eventos sem declarar perda. Recuperação exige snapshot íntegro + replay posterior; se snapshots/intervalo não existem ou autorização está revogada, falhar fechado e expor estado de recuperação, não fingir sincronização completa. Nó offline por período além da retenção precisa rebootstrap; qual disponibilidade desse caminho é requisito de produto ainda pendente.

Tombstones e eventos de revogação preservam ordem de propagação segundo C08. ACK de sincronização confirma cópia durável do evento/controle, não comprova eliminação física de cópias. Réplica offline cuja autorização/revogação ficou desconhecida não serve dados protegidos até revalidar estado. Backup restaurado precisa reaplicar controles/revogações antes de voltar a servir. Política efetiva de retenção e requisitos legais são bloqueios de prontidão.

## 6. Inbox, materialização e recuperação

### 6.1 Campos lógicos de Inbox

| Campo | Tipo/presença | Regra |
|---|---|---|
| `inbox_id` | identificador opaco, obrigatório | Identifica registro durável local. |
| `event_id` | UUID/ID lógico do evento, obrigatório | Identidade estável entre redeliveries. |
| `origin_node_id`, `tenant_id`, `stream_id`, `partition`, `authority_epoch`, `seq` | identificadores/inteiro; `partition` e `authority_epoch` obrigatórios (ADR-057) | Chave do namespace e ordenação; unicidade definida na §4. |
| `content_digest` | digest canônico versionado, obrigatório | Detecta colisão/conteúdo alterado. Algoritmo final pertence ao perfil GMP. |
| `validation_state` | enum obrigatório | `received`, `validated`, `rejected`, `quarantined`. Só `validated` pode compor checkpoint. |
| `materialization_state` | enum obrigatório | `pending`, `applied`, `failed_retryable`, `failed_terminal`, `not_required`. |
| `received_at`, `validated_at`, `materialized_at` | timestamps UTC | Campos de auditoria temporal; não substituem sequência. `validated_at`/`materialized_at` nulos até ocorrer. |
| `retention_policy_ref` | referência versionada obrigatória para persistir | Política C08 aplicável à cópia Inbox/projeção. |
| `error_code` | enum/string seguro, nulo quando sem erro | Sem payload privado ou segredo em log. |

### 6.2 Requisito transacional e alternativa recuperável

Se Inbox e projeção usam o mesmo recurso transacional, commit de `Inbox(validated)` + dedupe identity + materialização + checkpoint contíguo deve ser atômico. O ACK ao emissor só pode ser enviado após esse commit; projeções não podem ficar sem Inbox correspondente.

Se não compartilham transação, alternativa recomendada é Inbox durável como ledger de recebimento, seguida por projetor que consome `materialization_state=pending`, aplica efeito idempotente usando `(projection_version, event identity)`, e marca `applied`. Se projeção e marcador não puderem ser escritos atomicamente, o projetor precisa detectar repetição pelo próprio estado materializado ou manter uma Outbox local para a confirmação; reconstrução desde Inbox/snapshot é obrigatória. Somente persistir Inbox não autoriza declarar materialização pronta. Até modelo, banco e fronteira transacional serem escolhidos, não se afirma garantia de atomicidade implantada.

| Ponto de falha | Resultado/recovery requerido |
|---|---|
| Crash antes do commit Inbox | Nenhum ACK; remetente redelivera mesmo evento/seq; receptor tenta de novo. |
| Crash depois do commit Inbox, antes do ACK | Redelivery deduplicada por identidade/hash; retorna ACK do checkpoint realmente durável. |
| Commit Inbox e projeção na mesma transação, crash antes do ACK | Ambos sobrevivem ou nenhum sobrevive; redelivery não duplica projeção. |
| Inbox commitado em store separado, crash antes de projetar | Registro fica `pending`; worker retoma. Checkpoint só pode avançar segundo regra de durabilidade do Inbox, e estado da projeção fica explícito. |
| Projeção aplicada, crash antes de marcar `applied` | Retry é idempotente ou reconcilia estado de projeção; não duplica efeito. Se isso não puder ser garantido, bloquear essa projeção para eventos com efeito. |
| Projeção falha terminalmente | Manter Inbox/auditoria e erro; não ocultar lacuna da projeção; exigir reparo/reconstrução, sem apagar evento aceito. |
| Evento futuro chega antes de lacuna | Guardar como pendente; não confirmar cursor além do prefixo contíguo. |

## 7. Erros, falhas e segurança de replay

Códigos conceituais para mapeamento C07: `IDEMPOTENCY_CONFLICT`, `IDEMPOTENCY_KEY_EXPIRED`, `IDEMPOTENCY_KEY_TIME_INVALID`, `IDEMPOTENCY_POLICY_UNAVAILABLE`, `IDEMPOTENCY_RESULT_EXPIRED`, `EVENT_IDENTITY_CONFLICT`, `EVENT_SCHEMA_UNSUPPORTED`, `EVENT_AUTHORIZATION_DENIED`, `EVENT_SIGNATURE_INVALID`, `EVENT_EPOCH_STALE`, `SEQUENCE_GAP`, `CURSOR_EXPIRED`, `SNAPSHOT_REQUIRED`, `INBOX_UNAVAILABLE`, `MATERIALIZATION_PENDING`, `MATERIALIZATION_FAILED`. São rótulos de proposta, não códigos wire aprovados. C07 GMP/API v1 §6 também propõe `DISPATCH_SEQUENCE_GAP`, `DISPATCH_SEQUENCE_CONFLICT` e `DISPATCH_SCOPE_STALE` para ordenação local de bridge; os nomes são candidatos a integração C10/C15, não códigos ratificados. `DISPATCH_SEQUENCE_GAP` não é `SEQUENCE_GAP`; `DISPATCH_SCOPE_STALE` não é epoch de autoridade obsoleto.

Falhas de autorização, assinatura, epoch, fingerprint conflitante e política ausente são fail-closed, sem avanço de checkpoint. Indisponibilidade temporária de Inbox impede ACK e gera retry/backpressure conforme perfil; não confirmar por ter recebido bytes. DLQ/quarentena deve preservar referências mínimas, motivo e escopo com retenção C08; não copiar credenciais ou payload sensível sem autorização. Limites de tamanho, lote, concorrência e timeout são parâmetros obrigatórios ainda não dimensionados.

## 8. Casos de conformidade documental

São critérios para implementação futura, não testes executados:

1. Comando repetido na mesma chave/escopo/fingerprint retorna uma operação; mesma chave com outro fingerprint falha sem segundo efeito.
2. Timeout após envio seguido de retry consulta/retorna a mesma operação; chave desconhecida além da janela é recusada, não reexecutada.
3. Dois streams intercalados têm cursores independentes; sequência 8 ausente em um stream não bloqueia o outro.
4. Evento 12 antes do 11 não eleva checkpoint; chegada do 11 permite avançar somente até o último prefixo recebido e persistido.
5. Mesmo event identity/hash é deduplicado; mesma identidade com conteúdo diferente é conflito e não sobrescreve.
6. Snapshot até 50 seguido de replay aplica somente >50, com digest, escopo e prova de posição canônica (incluindo `partition`) validados; cursor expirado demanda snapshot, nunca skip silencioso.
7. Cursor sem `partition` ou de epoch incomparável é rejeitado, nunca traduzido nem completado por inferência; nenhum checkpoint avança e nenhum evento é pulado. Mudança de epoch exige handoff que declare fronteira anterior por stream e partição; evento de epoch antigo depois do fence é rejeitado/quarentenado segundo autoridade.
8. Crash antes do commit não gera ACK; crash após commit antes de ACK gera redelivery deduplicada; Inbox e projeção atômicos persistem juntos ou a projeção separada se recupera idempotentemente.
9. Sem política de retenção/escopo eficaz, cópia e replay persistente não ficam prontos; restauração requer recuperar controles de revogação.
10. Nenhum caso declara implementação validada sem evidência de execução autorizada.
11. Redelivery de comando serial no mesmo escopo conserva `dispatch_sequence` e fingerprint, mas usa novo `attempt_id`; resultado desconhecido exige consulta antes de efeito novo.
12. Posição posterior a uma lacuna de dispatch fica sem efeito; conflito na mesma posição falha fechado; escopo local obsoleto não migra silenciosamente. `dispatch_epoch` local e `authority_epoch` são verificados separadamente.
13. Alterar o registro local para `pending_gap`, `resolved` ou `scope_stale`, ou marcar uma tentativa `outcome_unknown`, não altera `Command.lifecycle` sem a evidência e a transição permitida em §2.2; estado `expired` exige prazo vencido confirmado e não reabre terminal.
14. Se o comando expira enquanto uma tentativa permanece `outcome_unknown`, impedir novo efeito e manter permitida somente a consulta/reconciliação autorizada da mesma identidade; não inventar resultado, apagar recibo/dedupe nem reabrir o comando.

## 9. Decisões e integrações pendentes

| Dono | Integração necessária |
|---|---|
| Alan / integração transversal | Registrar esta fonte canônica e resolver dependências que envolvem mais de um dono; não converter proposta em aprovação. |
| Graydon (C07) | Mapear chaves, estados, cursor opaco, erros e recibos em GMP/API; fixar canonicalização/fingerprint, compatibilidade e códigos wire. |
| Leslie (C06) | Aprovar semântica de epoch/handoff e prova do último checkpoint por stream/partição na troca de autoridade. |
| Michael / persistência (C11) | Alinhar unicidade, Inbox/Outbox, checkpoints, tombstones, retenção e fronteira de transação sem tratar SQL ilustrativo como implantação. |
| Kim (C09) | Confirmar representação do principal/tenant autenticado e revalidação de autorização para retry/replay. |
| Bruce (C08) | Definir política eficaz por classe/cópia, duração, expiração e sequência de revogação/restauração. |
| Gregor (C15) | Confirmar mapeamento `event_id`/`origin_event_key`, hash/proveniência e versionamento UDNL sem dedupe global de identidade externa. |
| Graydon (C07) | Mapear códigos candidatos e regras de retry de dispatch para o contrato GMP; formalizar compatibilidade e transições entre registro local, `DeliveryAttempt.state` e `Command.lifecycle` conforme §4.1.3. |
| Gregor (C15) | Manter lifecycle local de dispatch, `dispatch_scope_ref`/`dispatch_sequence`, recuperação e evidências de bridge alinhados a §4.1.3; não confundir com sequência de eventos ou `authority_epoch`. |
| Produto/usuário | Aprovar vocabulário/compatibilidade de estados; janela idempotente, skew, retenção, disponibilidade de nó offline, RPO/RTO/SLO e políticas/jurisdições aplicáveis. |

**Readiness:** até essas decisões e a validação independente ocorrerem, este texto serve como proposta revisável e casos de conformidade, não como promessa interoperável, política aprovada ou resultado de produção.


## 4.1.3 Integração do ciclo de vida persistente C15

Esta subseção integra documentalmente o ciclo de vida local descrito em `03-integracoes/gateway-universal.md`, “Ciclo de vida local de dispatch”, linhas 109–130 (fonte consultada em 2026-11-16). Ela não altera o contrato GMP de C07 nem declara política aprovada. O registro durável de escopo/posição/comando e o checkpoint contíguo local pertencem ao dispatcher; não são o Inbox, o cursor ou o checkpoint de eventos desta fonte. `dispatch_epoch` é geração do escopo local e permanece distinto de `authority_epoch`, que cerca autoridade de escrita.

**Estados propostos do registro local.** `queued` significa persistência confirmada antes do envio; `pending_gap` significa que uma posição anterior falta e nenhum efeito posterior pode ocorrer; `outcome_unknown` significa envio possível sem resultado conclusivo e bloqueia novo efeito; `resolved` significa resultado conhecido por recibo ou reconciliação; `expired` impede iniciar ou renovar efeito após `expires_at`; `scope_stale` impede uso da geração retirada. Esses nomes são vocabulário de integração proposto, não enum ratificado. O dispatcher persiste escopo, sequência e comando atomicamente antes de confirmar `queued`. Só move o checkpoint local pelo prefixo contíguo de posições resolvidas. Restore antigo, perda do registro ou rotação com pendências mantém o alvo bloqueado até reconciliar o maior número usado e os efeitos incertos. A rotação não reinicia nem reutiliza posições.

| Condição | Ação proposta | Relação com erros GMP |
|---|---|---|
| Lacuna de dispatch | Manter a posição posterior pendente; recuperar a ausente; não produzir efeito nem avançar checkpoint. | `DISPATCH_SEQUENCE_GAP` é candidato local, distinto de `SEQUENCE_GAP` de eventos. |
| Mesma posição com outra identidade lógica | Preservar vínculo anterior e falhar fechado, sem retry automático. | `DISPATCH_SEQUENCE_CONFLICT` é candidato, não código wire aprovado. |
| Referência de escopo/geração retirada | Quarentenar e reconciliar antes de rotear para geração nova. | `DISPATCH_SCOPE_STALE` é candidato, não `authority_epoch_stale`. |
| Prazo expirado | Não iniciar nem renovar efeito; consulta só pode retornar resultado já conhecido. | `DEADLINE_EXCEEDED` é candidato GMP geral; a semântica vem de C07. |
| Serviço de recuperação indisponível | Manter pendente ou `outcome_unknown`; permitir consulta/retry da mesma identidade apenas se prazo, autorização, retenção e dedupe vigentes permitirem. | `UNAVAILABLE` não libera nova posição nem novo efeito. |

Após crash depois do envio sem recibo, o dispatcher consulta o mesmo `command_id`/`operation_id` e mantém `outcome_unknown` até reconciliação conclusiva. Nunca converte incerteza em sucesso ou falha presumidos; sem deduplicação remota ou evidência de estado, bloqueia o alvo. O comportamento não promete exactly-once. Rotação só pode ativar nova geração depois da reconciliação das posições antigas. Isso atende ao requisito documental de recuperação de C15, sujeito à política de retenção C08: sem prazo eficaz para registro, recibo e dedupe, readiness de recuperação permanece bloqueada.

### Propriedade e transições entre os estados

A tabela cruza o registro local de dispatch C15 com os tipos C07/C10 sem criar lifecycle de comando concorrente. `Command.lifecycle` conserva os valores e transições de §2.2 e GMP §7.2; `DeliveryAttempt.state` registra uma tentativa específica. Os estados `pending_gap`, `resolved` e `scope_stale` qualificam registro/recuperação local, não `Command.lifecycle` nem enum wire aprovada.

| Estado/registro | Proprietário e gatilho observado | Efeito permitido em `DeliveryAttempt.state` | Efeito permitido em `Command.lifecycle` | Transição vedada |
|---|---|---|---|---|
| `queued` | Dispatcher; commit atômico de escopo, posição e comando/outbox antes de confirmar a fila. | Criar tentativa e registrar sua submissão; `queued` não prova envio. | Pode registrar `persisted`; `routed` só após evidência de roteamento conforme §2.2. | Não marcar tentativa aceita nem comando `completed` pela persistência local. |
| `pending_gap` | Dispatcher; existe posição posterior sem prefixo anterior contíguo recuperado. | Registrar tentativa pendente/retida e evidência da lacuna. | Sem avanço por si; conservar lifecycle atual, salvo evidência independente que satisfaça transição definida. | Não enviar efeito posterior, avançar checkpoint, nem inferir `failed`/`completed`. |
| `outcome_unknown` | Adaptador/dispatcher; envio pode ter ocorrido, mas recibo/resultado não foi confirmado. É estado de `DeliveryAttempt` conforme GMP §4.5. | Marcar a tentativa como `outcome_unknown`; consultar recibo/estado com a mesma identidade. | Nenhuma mudança automática; lifecycle só muda com evidência e regra de §2.2. | Não classificar como sucesso/falha, criar efeito/posição nova ou marcar terminal só pelo estado desconhecido. |
| `resolved` | Dispatcher; recibo autenticado ou reconciliação conclusiva vincula resultado ao mesmo comando/posição. | Registrar estado/recibo observado pela tentativa correspondente. | Atualizar apenas quando o predicado do comando for provado; `completed` requer predicado de sucesso, e falha terminal exige evidência/regra aplicável. | Não tratar resolução do checkpoint como `Command.completed` por si só. |
| `expired` | Serviço dono do comando confirma `expires_at` vencido sob relógio/perfil aplicável. | Impedir novas tentativas de efeito. Se tentativa anterior permanece `outcome_unknown`, preservar esse estado e permitir apenas consulta/reconciliação autorizada da mesma identidade. | Pode mover o comando para `expired` segundo lifecycle C10 e evidência de prazo; isso não determina o resultado da tentativa. | Não iniciar/renovar efeito, presumir o resultado remoto, apagar recibo/dedupe ou reabrir lifecycle terminal por retry. |
| `scope_stale` | Dispatcher rejeita referência de geração local retirada/incompatível. | Rejeitar/encerrar tentativa no escopo antigo e registrar motivo; eventual nova geração exige reconciliação. | Não muda por si; manter lifecycle até evidência independente de transição. | Não reescrever escopo/posição da tentativa anterior nem converter em `authority_epoch_stale`. |

Toda transição de lifecycle requer o gatilho e a evidência definidos em §2.2; a observação de um estado local ou de tentativa nunca basta. Estados terminais de comando não regridem nem mudam para outro terminal. C07 revisa tecnicamente e mapeia estes vínculos para wire; a aprovação normativa cabe ao usuário. Os nomes e códigos continuam propostos até essa aprovação.

Os códigos acima e o uso de `DEADLINE_EXCEEDED`/`UNAVAILABLE` precisam ser mapeados e revisados tecnicamente por C07; a aprovação normativa cabe ao usuário. `dispatch_scope_ref` pode vincular a geração opaca, sem exigir `dispatch_epoch` como campo wire adicional. C07 deve mapear `dispatch_sequence`, escopo, checkpoint, códigos/retry e a tabela de transições acima. C06 continua responsável por semântica de autoridade/handoff. C08 precisa definir retenção efetiva de registros/recibos; C15 mantém o lifecycle local; C10 separa a sincronização de eventos. Não há decisão de retenção ou implementação validada nesta atualização.
