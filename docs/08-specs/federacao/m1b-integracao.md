# SPEC M1B/I v1.3 — integração da decisão de ingresso assinada

**Estado:** proposta para revisão independente em 18/11/2026. Cada caso abaixo aguarda execução sobre a integração.

## 1. Objetivo e escopo

Integrar `room/m1b-aprovacao-assinada-por-no-szuq` em `main` sobre `51e192c`, com S-sec-2. O aceite HTTP local libera a integração como etapa intermediária. O aceite de ponta a ponta da federação exige a evidência de pelo menos três servidores do §6 após M1C. A mudança do contrato de autorização do Inbox pertence à M4.

## 2. Governança e contrato de decisão

Aprovar ou rejeitar exige grant humano `federation:admin` e prova Ed25519 do aprovador vinculada ao pedido e ao cluster. A variável `GODMESH_FEDERATION_GOVERNANCE=single_approver:<cluster>` é a declaração explícita do operador de que o cluster nomeado opera sem votantes, conforme a ADR-071. A variável ausente ou vazia deixa a decisão desabilitada. Valor malformado ou modo não suportado impede o boot de serviço com `78`. Com o serviço no ar, decisões para cluster diferente ou modo desabilitado retornam `422 federation_governance_disabled`, sem transição. A configuração vale para a decisão local de ingresso. Bundles de confiança da ADR-072 continuam sujeitos ao quórum e ficam na M3.

Validar a assinatura contra a chave do par `authorized` do mesmo cluster ou, na ausência de registro para o aprovador, contra o genesis declarado. O registro prevalece sobre o genesis, inclusive quando revogado. Exigir `not_before <= now < expires_at` e epoch igual ou superior ao maior de uma aprovação ou rejeição confirmada por aquele nó. O piso é durável por aprovador e avança na mesma transação que confirma uma nova decisão. Um replay ou uma tentativa recusada preserva o piso.

Recusar epoch acima de `i64::MAX` e expiração fora da faixa de segundos Unix persistível em `timestamptz` com `422 invalid_request`. Para PostgreSQL 16, a faixa em segundos inteiros é `-210866803200..=9224318015999`. Preservar o segundo UTC assinado na gravação e na leitura, inclusive no limite inclusivo de armazenamento. A janela de autorização continua semiaberta. [Tipos temporais do PostgreSQL 16](https://www.postgresql.org/docs/16/datatype-datetime.html) · [Limites internos](https://github.com/postgres/postgres/blob/REL_16_STABLE/src/include/datatype/timestamp.h)

Preservar o JSON, os bytes canônicos e o domínio de aprovação `godmesh.federation.approval.v1`. A rejeição usa os mesmos campos JSON e o domínio exclusivo `godmesh.federation.rejection.v1`. A rota determina o domínio exigido. Clientes de aprovação mantêm a assinatura existente. Clientes de rejeição precisam assinar no novo domínio, e assinaturas anteriores no domínio de aprovação são recusadas nessa rota. Aprovar persiste a prova e autoriza o par atomicamente. Repetir a aprovação de pedido já aprovado com prova ainda válida e autorizada responde `200`, mantém a prova e a configuração do par gravadas e rejeita par revogado. Repetir uma rejeição válida de pedido já rejeitado responde `409 join_request_state_conflict`. Rejeitar encerra o pedido sem autorizar o par nem preencher campos de prova de aprovação.

## 3. Migração e implantação

Aplicar `0009_federation_approval.sql` pendente também em banco com `0010_event_signed_ingest.sql` aplicada. Preservar numeração, conteúdo e checksums aplicados, histórico do migrador e dados do ingest. Verificar banco vazio, atualização e reexecução sem reaplicação.

A migração `0011_federation_decision_epoch.sql` cria o piso durável por aprovador e o inicializa com o maior epoch das aprovações assinadas já registradas por nó. A migração preserva o histórico de pedidos e a exigência de campos de prova de aprovação vazios em pedidos rejeitados. Rejeições históricas sem epoch persistido permanecem sem esse dado.

Antes de atualizar uma instalação com `0008` aplicada e `0009` pendente, executar `SELECT count(*) FROM federation_join_request WHERE state = 'approved'` no banco de destino. Usar a janela de manutenção para impedir decisões da versão anterior entre essa leitura e a migração. Em uma instalação nova, o migrador cria o esquema em ordem. A pré-verificação é somente leitura. Encontrar pedido legado `approved` sem prova bloqueia a implantação. A guarda de `0009` também deve abortar transacionalmente, preservando dados e histórico, sem DDL parcial nem registro de aplicação. O procedimento futuro para esses dados requer decisão separada antes de qualquer intervenção.

## 4. Invariantes transacionais

- **Decisões.** Serializar por aprovador, inclusive genesis sem linha de par. Verificar autoridade, estado do pedido e piso de epoch na transação da decisão, mantendo a proteção até o commit.
- **Ingest.** Serializar por `(origem, tenant, stream, partição)`, inclusive na primeira escrita. O Inbox participa do mesmo bloqueio ao gravar em `event_position`. Seu contrato de autorização e sua própria política de epoch ficam para a M4. Ler e proteger autoridade e escopo na transação que verifica epoch, retenção e sequência e grava a posição. Autoridade e escopo devem refletir alterações já concluídas antes da autorização, e alterações concorrentes devem aguardar o commit ou rollback da operação autorizada. O mesmo vale para a autoridade da decisão de ingresso. Respostas de sucesso de escrita só saem após commit.

## 5. Aceite HTTP local

| Caso | Verificação |
|---|---|
| Ingresso e escopo | Genesis aprova B (`200`, prova persistida). B pode decidir no cluster habilitado. Com escopo explícito, B grava evento (`201`). Os `scopes` do certificado não concedem escopo de ingest. Rejeição válida encerra o pedido sem autorização. |
| Recusas | Verificar grant ou prova ausente, ação ou pedido divergente, nó desconhecido, pendente ou revogado, cluster sem habilitação e epoch regressivo. No ingest, verificar corpo legado, assinatura inválida, vínculo de identidade, tenant e escopo. Comparar estado durável antes e depois. |
| Concorrência e recuperação | Com ordenação controlada, comprovar que operações observam o estado confirmado e que decisões de ingresso e chamadas a `POST /v1/events` de epoch inferior não escrevem após commit de epoch superior no namespace aplicável. Mudança de autoridade ou escopo anterior à autorização deve ser observada. Falha transacional deixa estado íntegro e permite nova tentativa ainda válida. |
| Piso e faixa persistíveis | Após uma rejeição confirmada, negar aprovação e rejeição do mesmo nó com epoch inferior, preservando pedido, par e piso. Aceitar epoch igual com prova válida. Confirmar que `0011` preserva o maior epoch das aprovações anteriores. Aceitar a última expiração UTC persistível com igualdade exata ao consultar o banco. Recusar o segundo seguinte com `422 invalid_request`, sem efeito. |
| Escritas entre Inbox e ingest | Após o Inbox confirmar epoch superior no namespace, `POST /v1/events` com epoch inferior retorna `422 event_epoch_stale` e não grava posição. Se o ingest confirmar o epoch superior primeiro, o Inbox de epoch inferior segue seu contrato legado sob o mesmo bloqueio. Conferir o prefixo retornado, o checkpoint próprio do epoch do lote e a preservação da posição já registrada no epoch superior. |
| Replay, retenção e lacuna | Replay ainda autorizado, fresco e elegível retorna `200` com a classe persistida e uma única posição. Jurisdição ou política de retenção ausente recusa a escrita. Lacuna retorna o erro contratual com sequência esperada e recebida. Preencher a lacuna permite reenviar a posição seguinte. |
| Frescor e contrato | Aceitar os limites inclusivos de frescor do evento, passado e futuro, e recusar um segundo além. Preservar a janela semiaberta da decisão. Conferir corpos, status e erros em `/v1/meta`, OpenAPI e HTTP. |

Erros estáveis do aceite incluem `401 unauthenticated` para ausência de credencial, `403 insufficient_scope` para grant sem o escopo da rota, `422 invalid_request` para corpo sem prova, `422 approval_signature_invalid` para pedido ou sentido de decisão divergente, `422 approval_not_yet_valid` e `422 approval_expired` para limites da janela, `422 approval_epoch_regressed`, `422 approver_not_authorized`, `422 no_authorized_approver` e `422 federation_governance_disabled`. O ingest preserva `422 event_signature_invalid`, `422 event_node_unauthorized`, `422 event_scope_denied`, `422 event_not_fresh`, `422 event_epoch_stale`, `409 sequence_gap`, `409 retention_policy_missing` e `409 retention_scope_unresolved`. Falha transitória de armazenamento preserva `503 storage_unavailable` e a política de `effect: unknown` do timeout HTTP.

## 6. Aceite futuro e rastreabilidade

**Após M1C.** Iniciar A, B e C com bancos e identidades distintos. A, genesis declarado, aprova B. B recebe, verifica e consome a prova pelo caminho oficial, depois aprova C. C permanece isolado até consumir sua prova válida. Conferir ingest de B com escopo explícito e rejeitar prova inválida ou expirada. Confirmar os enlaces pelo protocolo. Os registros de confiança devem vir do fluxo de ingresso e distribuição.

**Rastreabilidade.** RF-015 e RNF-002, parcialmente, → ADR-071 → testes de federação e `0009`. Política do ingest §5 → testes `event_signature` e `0010`. ADR-072 rege a sequência. O modo local segue a exceção explícita para clusters sem votantes da ADR-071. O domínio de rejeição faz parte desta versão submetida à revisão.

As evidências existentes cobrem as fatias separadas. O aceite integrado permanece pendente. A suíte de base executada antes da integração está registrada separadamente.
