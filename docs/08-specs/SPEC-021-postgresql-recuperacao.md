# SPEC-021 — Operação PostgreSQL e recuperação

**Versão:** DRAFT-0.4 · **Data:** 2026-11-17  
**Estado:** DRAFT / OPEN; não aprovada e não pronta para implementação.  
**Autor:** Bruce Momjian.  
**Revisão independente designada:** Michael Stonebraker; solicitada após congelamento do hash ao final deste rascunho.  
**Requisitos:** cobertura operacional transversal ainda precisa de vínculo semântico confirmado por Ada. RNF-005, RNF-006 e RNF-010 são candidatos de operação/qualidade citados na pauta, não cobertura integral confirmada por esta SPEC. B09/B10 são frentes de planejamento, não IDs de requisito.  
**Escopo:** contrato documental para operação PostgreSQL, cópias de backup, restore, promoção, recuperação de persistência/sync e compatibilidade/rollback de schema. Não escolhe tecnologia de backup, política, parâmetros quantitativos nem arquitetura de disponibilidade.

## 1. Fontes e rastreabilidade

- `docs/README.md` §§ Regras de prevalência e Manutenção, linhas 97–128: arquitetura de referência e ADRs prevalecem; documentação não comprova implementação.
- `docs/01-arquitetura/arquitetura-referencia.md` e `docs/06-planejamento/decisoes.md`: normas e decisões vigentes devem prevalecer em conflitos; decisão específica de recuperação operacional ainda não identificada.
- `docs/00-produto/visao.md`: escopo integral Personal, Teams, Node e Platform/SDK; não atribui comportamento de recuperação já implementado.
- `docs/05-operacao/implantacao.md:3–17,59–79`: servidor Rust em contêiner Linux como alvo aprovado; topologia e bootstrap são documentais; readiness e backup são expectativas; limites, ferramentas e políticas não escolhidos.
- `docs/01-arquitetura/replicacao.md:3–15,23–40,42–55`: classes de dados e consistência recomendadas, snapshot/replay, retenção e autoridade por tenant/partição; cópias de backup devem ser separadas de réplicas.
- `docs/02-dominio/modelo-dados.md:122–159`: Outbox/Inbox, checkpoints namespaceados, replay e migration são contratos ilustrativos; unicidade, backfill, compatibilidade, rollback e fronteiras físicas seguem pendentes.
- `docs/07-contratos/eventos-sincronizacao.md:145–155,173–180`: snapshots, retenção, revogação após restore e alternativas transacionais Inbox/materialização.
- `docs/05-operacao/observabilidade.md` e `docs/06-planejamento/pendencias.md:80–98`: relação requisito/sinal/método/responsável, RPO/RTO, autoridade, retenção, restore e fronteiras de recuperação ainda sem decisões completas.
- `docs/06-planejamento/matriz-rastreabilidade.md:150–162,164–166`: casos devem explicitar pré-condição, esperado, falha/recuperação, ambiente e evidência; NOT_ASSIGNED e NOT_EXECUTED preservam o estado real.
- Contribuição de origem lida: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-bruce-momjian.md`, SHA-256 `1ed093625a080c746d06d0488818ee74d8635630cfd9688ac30ea478df85c294` (2026-11-16). Seu parecer focal de Michael no mesmo arquivo de contribuição concluiu sem objeção material aos recortes D2–D4 nos hashes avaliados; isso é revisão documental de contribuição, não aprovação desta SPEC. A resposta posterior de Michael ao parecer foi incorporada na mesma contribuição.

### Estado das afirmações

**Especificado:** regras normativas citadas nas fontes acima. **Proposto nesta SPEC:** contratos e critérios abaixo, até aprovação. **Aprovado:** apenas decisões marcadas como aprovadas nas fontes; não há aprovação dos parâmetros de recuperação aqui. **Implementado/validado:** não verificado. Nenhum banco, backup, restore, migration, serviço ou produto foi acessado ou executado.

### Posição, ACK e política normativa (ADRs-057/058/061/062)

Posição canônica: `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. `event_sequence`, `dispatch_sequence`, `dispatch_epoch` e `authority_epoch` são distintos e não intercambiáveis. Cursor opaco versionado vincula principal, escopo, filtros, perfil e retenção; cursor sem partição ou com epoch incomparável é rejeitado, nunca traduzido. Toda resposta exige `ack_class`: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device` ou `acknowledged_human`, cada qual com emissor, evidência e escopo próprios. ACK de Inbox tem limite `persisted_local`.

Outbox na mesma transação do domínio é o padrão. Inbox em ledger separado só quando recepção e projeção não podem compartilhar transação. Replay reconstrói projeções e nunca repete efeitos externos irreversíveis (bridge, RF, pagamento, notificação). Autoridade `stale/unknown` bloqueia mutação forte. Persistência exige política aprovada por `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)`; sem ela falha fechada e degrada explicitamente, retendo só o mínimo necessário ao efeito imediato autorizado. RPO/RTO e prazos permanecem `OPEN — decisão do usuário`.

## 2. Produtos, plataformas e atores

O servidor PostgreSQL pertence ao alvo servidor GodMesh em contêiner Linux. A SPEC descreve a semântica de recuperação dos dados compartilhados por Personal, Teams, Node e Platform/SDK; não presume que cada produto mantenha seu próprio banco ou cópia.

Clientes macOS arm64/x86_64, iOS, Windows, Linux desktop, Android e web consomem estados e contratos de recuperação quando expostos por serviços autorizados. Nenhum fluxo de operação direta ao banco nesses clientes é especificado. Aplicabilidade de cada cenário por produto/plataforma precisa de confirmação; `N/A` exige justificativa de domínio revisada, sem exclusão global.

Atores lógicos: solicitante autenticado; operador com autoridade aprovada para o escopo; serviço de banco/recuperação; autoridade de escrita/fencing por tenant/partição; verificador de integridade; projetor Inbox quando aplicável. Funções concretas, pessoas de plantão, credenciais e mecanismo de autorização permanecem OPEN. A autoria da SPEC não concede acesso operacional.

## 3. Contrato de recuperação

### 3.1 Intenção e elegibilidade

1. A operação declara objetivo, escopo (tenant/classe/cópia), destino lógico, ponto pretendido de recuperação e autoridade solicitante.
2. O operador identifica a modalidade e cópia candidatas conforme política aprovada. A cópia deve ter proveniência, escopo, versão de schema/perfil e evidência de integridade/completude suficientes para o método definido.
3. Política de retenção, autorização vigente, revogações e dependências de chaves/provas são revalidadas antes de disponibilizar dados protegidos. Política ausente/desconhecida ou cópia sem evidência impede elegibilidade.
4. A réplica em funcionamento não substitui backup independente contra corrupção, exclusão maliciosa ou propagação de erro.
5. Destino restaurado permanece isolado até validação. Carregamento da cópia não declara prontidão nem autoriza promoção.

### 3.2 Validação e promoção

A validação compara o escopo restaurado com o predicado aprovado: integridade estrutural/lógica aplicável, isolamento entre tenants, versão do schema/perfil, evidência da cópia, estado de retenção/revogação, Inbox/Outbox, checkpoints e projeções. Regras e ferramentas específicas de verificação são OPEN. Falha, divergência ou falta de evidência mantém o destino isolado e produz estado parcial/`unknown` com evidência sanitizada.

Promoção exige decisão de operador autorizado e contrato transversal de autoridade/fencing que prove o limite de escrita, estado do epoch e tratamento de escritores concorrentes. Sem isso, bloquear promoção. A autorização de promover não pode ser inferida da conclusão do restore. Rollback de schema não desfaz eventos ou efeitos externos aceitos; resultado incerto exige reconciliação explícita, sem replay cego.

No PostgreSQL, replicação lógica não replica DDL nem sequences: alterações de schema e avanço de sequences exigem coordenação explícita, compatibilidade e reconciliação na promoção/restore. RLS é defesa em profundidade e não constitui isolamento único: proprietário da tabela, superusuário ou papel `BYPASSRLS` pode contorná-la; combinar com autorização na aplicação, papéis mínimos e integridade referencial conforme contrato aprovado.

### 3.3 Sync após recuperação

- Preservar escopo, origem, stream, epoch quando aplicável e último prefixo contíguo validado. Lacuna não pode ser saltada.
- Preservar posição canônica `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. `event_sequence`, `dispatch_sequence`, `dispatch_epoch` e `authority_epoch` não se substituem. Cursor opaco versionado liga principal/escopo/filtros/perfil/retenção; sem partição ou epoch incomparável é rejeitado, nunca traduzido. Rebootstrap só ocorre se política, autorização, snapshot íntegro e retenção permitirem.
- Inbox durável não comprova materialização. Se Inbox e projeção usam mesma transação, exigir commit conjunto conforme contrato. Se forem separadas, escolher por serviço o ledger/projetor recuperável e idempotente antes de ACK/promover.
- ACK exige `ack_class` dentre as seis classes normativas. Inbox só pode emitir até `persisted_local`; não implica entrega global, projeção concluída ou atendimento humano.
- Após restore antigo, aplicar revogações e retenções vigentes antes de servir cópia, mesmo quando o backup era íntegro no instante capturado.

## 4. Estados e transições propostos

Rótulos seguintes são estados conceituais para descrever os resultados; nomes wire, enums e armazenamento não estão aprovados.

| Estado | Significado | Transição permitida |
|---|---|---|
| `requested` | Pedido recebido; autorização e escopo ainda não validados | `eligible`, `rejected` ou `unknown` |
| `eligible` | Cópia e política passaram os predicados de elegibilidade definidos | `restoring` ou `blocked` |
| `restoring` | Restore em andamento para destino isolado | `validating`, `partial` ou `failed` |
| `validating` | Restore terminou; predicados de consistência são avaliados | `ready_isolated`, `partial`, `blocked` |
| `ready_isolated` | Predicados aprovados no escopo declarado; ainda sem promoção | `promoted` somente após decisão/fencing ou `blocked` |
| `promoted` | Autoridade aprovada habilitou serviço para escopo específico | Estado do serviço é reportado separadamente; não prova SLO |
| `partial` | Somente parte do escopo satisfaz predicados, com cobertura explícita | `ready_isolated` após reconciliação ou `blocked` |
| `blocked` | Falta decisão, autoridade, evidência, política ou compatibilidade | Requer ação responsável documentada; sem retry cego |
| `failed` | Método reportou falha; sucesso não demonstrado | Retry idempotente ou procedimento de descarte/reconciliação aprovado |
| `unknown` | Estado ou evidência não pode ser determinado | Revalidar fonte ou manter indisponível; não inferir sucesso |

O rótulo de estado não substitui evidência, autorização ou decisão humana. Transições finais e contratos de erro precisam ser conciliados com APIs/eventos por seus responsáveis.

## 5. Idempotência, retry e falhas

Toda operação mutável precisa de identidade estável de operação e escopo. Repetição com a mesma identidade e parâmetros deve retornar o estado/evidência existente ou continuar de checkpoint seguro; reutilização com parâmetros incompatíveis é conflito explícito. Cada fronteira de commit tem resultado recuperável após crash. Não presumir transação atômica entre PostgreSQL, armazenamento de cópias, transporte, projeção ou serviço externo.

| Falha | Resultado requerido | Recuperação proposta |
|---|---|---|
| Cópia ausente, escopo ambíguo, digest/proveniência inválida | `rejected` ou `unknown`; sem alteração da fonte e sem promoção | Identificar cópia/evidência autorizada; não selecionar automaticamente outra classe |
| Crash no restore | `partial`/`failed`; destino não servido | Retomar por etapa idempotente ou descartar segundo procedimento aprovado; validar antes de retomar |
| Falha de validação ou integridade | `blocked`; destino isolado | Registrar predicado que falhou e reconciliar com responsável |
| Revogação ou política mudou após captura | Não servir dado protegido | Revalidar política atual; bloquear se fonte estiver indisponível/desatualizada |
| Lacuna em sequência/checkpoint ou epoch mudou | Não avançar cursor nem aceitar escritor antigo | Recuperar prefixo/handoff autenticado ou rebootstrap aprovado |
| Inbox persistiu, projeção falhou | Estado materialização pendente; não declarar pronto | Projetor idempotente/reconstruível selecionado e validado documentalmente |
| Evento duplicado, identidade e digest iguais | Sem efeito duplicado | Dedupe dentro da retenção aprovada |
| Chave/posição coincide, digest ou identidade diverge | Conflito/quarentena; sem sobrescrita nem avanço de checkpoint | Reconciliação por autoridade do domínio |
| Writer concorrente após restore | Sem promoção enquanto não houver fence/autoridade demonstrada | Bloquear writers conforme mecanismo aprovado; detalhes OPEN |
| Resultado de efeito externo desconhecido | Não repetir automaticamente efeito possivelmente irreversível | Consulta/reconciliação; decisão operacional autorizada |

Limite de tentativas, atrasos, expiração de idempotency key e política de quarentena são OPEN e não recebem valores nesta versão.

## 6. Migração de schema e rollback

Uma futura migration deve declarar versão anterior/alvo, leitores/escritores compatíveis, dados históricos mapeáveis, checks de pré-condição, checkpoints de etapa, integridade/contagens esperadas, janela/corte de escrita, estratégia de rollback e condição para parar. Não adivinhar tenant, stream, epoch, assinatura ou autoridade a partir de linha incompleta. Linha sem proveniência fica em quarentena ou impede o corte.

Para mudança do namespace de eventos, preservar envelope histórico e bytes assinados quando aplicável; qualquer posição derivada precisa de crosswalk versionado/autenticado e decisão de autoridade. Retirar chave de unicidade legada somente após leitores/escritores estarem em perfil compatível e corte cercado. Checkpoints escalares/cursors antigos sem tradução expiram e requerem rebootstrap autorizado; não presumir continuidade entre epochs.

Rollback de schema só é elegível quando envelope, eventos, provas, checkpoints e efeitos pós-corte podem ser preservados ou reconciliados. Efeito externo desconhecido/irreversível bloqueia rollback automático. Alternativa é interromper promoção e recuperar adiante sob decisão autorizada. O plano por etapa e a janela de compatibilidade permanecem OPEN; nenhum script, migration ou rollback foi executado.

## 7. RPO/RTO e evidência operacional

Não existem metas quantitativas escolhidas nesta SPEC. Para cada objetivo futuro, registrar requisito, classe/tenant, cópia/modalidade, ponto pretendido, último ponto consistente comprovado, fonte e versão de evidência, operador responsável, início/fim segundo relógio aprovado e cobertura de escopos recuperados/desconhecidos. RPO compara marcos de dados definidos e comparáveis; não pode ser inferido só da idade do arquivo. RTO termina apenas no predicado aprovado de serviço recuperado/autorizado para o escopo, com detecção, decisão, restore e promoção medidos separadamente.

Se faltar denominador, relógio, política, origem temporal ou evidência, resultado é `unknown`; restauração parcial reporta numerador/escopo conhecido e não se generaliza para o conjunto. Janelas, agregação, metas por classe e tolerância de indisponibilidade dependem de decisão de produto/usuário e análise de Brendan/Michael/Momjian. Não há SLO ou medição observada aqui.

Registro operacional futuro inclui identificador da operação/cópia, escopo minimizado, modalidade/método e versão, horários, predicados e resultado, checkpoints, falhas, decisões/aprovador e referências a evidências. Não inclui credenciais, payload privado, chaves nem prova reutilizável. Acesso, retenção, jurisdição, redaction e consulta ficam condicionados às decisões de segurança/privacidade.

## 8. Critérios de aceite e casos planejados

Casos concretos recebem IDs no índice de Ada quando atribuídos. Todos abaixo são **planejados e NOT_EXECUTED**; ambiente, método exato e oráculo necessitam aprovação/definição.

| Caso planejado | Pré-condição e estímulo | Esperado | Requisitos/plataformas |
|---|---|---|---|
| Restore íntegro para escopo elegível | Cópia com proveniência/política/autoridade válidas; restaurar destino isolado | Validar predicados definidos e ficar `ready_isolated`; não promover automaticamente | RF/RNF de operação ainda a mapear; servidor Linux; clientes recebem estado aplicável |
| Cópia corrompida ou incompleta | Digest/proveniência ou consistência falha | `blocked`/`unknown`, sem servir nem promover; evidência do predicado ausente | B09/B10; todas plataformas que consultam status |
| Restauração parcial | Falha em parte de tenant/classe/cópia | Delimitar escopo recuperado e desconhecido; não relatar sucesso global | Requisitos operacionais a atribuir |
| Revogação após backup | Backup precede revogação e é restaurado | Não servir cópia protegida até revalidar e aplicar política atual | RNF de segurança/retenção a mapear; servidor e clientes |
| Crash em cada fronteira de restore | Interromper antes/depois de checkpoints definidos | Retry converge sem duplicar efeito nem marcar etapa ausente como concluída | B09/B10; Linux servidor |
| Inbox e projeção na mesma transação | Falha antes e após commit | Sem ACK antes de commit comum; retry não duplica projeção | Interface B06/B12; servidor Linux |
| Inbox/ledger e projetor separados | Inbox durável; projetor falha e reinicia entre efeito/marcador | ACK limitado ao ledger; materialização permanece pendente até prova idempotente | Interface B06/B12; servidor Linux |
| Lacuna ou epoch diferente | Evento posterior disponível ou epoch troca sem handoff | Checkpoint não salta lacuna; handoff ausente bloqueia continuidade | Interface B06/B12; servidores/nós autorizados |
| Escritor obsoleto após restore | Instância antiga tenta escrever após promoção | Fencing/autorização barra escrita ou teste fica bloqueado se mecanismo não definido | C06/B06/B12; servidor Linux |
| Rollback com efeito externo incerto | Mudança de schema aceitou evento/efeito pós-corte | Bloquear rollback automático e exigir reconciliação ou recuperação adiante | B09/B10 e B06/B12; Owner de efeito externo OPEN |
| Cursor legado sem tradução | Snapshot/retenção suficiente e insuficiente em cenários distintos | Rebootstrap somente quando autorizado e possível; senão indicar recuperação incompleta | RF/RNF sync a cruzar; servidor e clientes |
| Restore sob retry duplicado | Mesma identidade/escopo repetidos; depois parâmetros divergentes com mesma chave | Mesmo resultado/continuação idempotente no caso igual; conflito no divergente | Requisitos de operação/API a mapear; interfaces aplicáveis |

**Aceite documental desta SPEC:** cada requisito operacional aplicável deve estar ligado a contrato, estado, método, dono e critério observável; alternativas e limites aprovados; casos negativos/degradação/recuperação revisados; compatibilidade com C06/C08/C10/C11 conciliada; revisão independente deste hash registrada. Aceite documental não equivale a teste executado nem aprovação final do lote.

## 9. Decisões OPEN

| Decisão | Alternativas e proposta provisória | Dono/dependência | Condição de fechamento |
|---|---|---|---|
| Requisitos RF/RNF cobertos e cenários por produto | Mapear requisitos operacionais a partir da matriz, sem inferir por semelhança | Ada/Margaret/Brendan; matriz central | Linhas atribuídas e lacunas identificadas com revisor |
| Modalidades/fontes e consistência das cópias | Definir após inventário aprovado; não escolher ferramenta nem confundir réplicas com backup | Brendan/Michael/Momjian | Para cada modalidade: escopo, fonte, consistência, verificação, autorização, limitação e método |
| RPO/RTO, janela, classe e risco | Metas por classe/cópia após cenário e custo versus sem garantia até estudo; sem números nesta versão | Produto/usuário; Brendan/Michael/Momjian | Metas aprovadas, relógio, denominador, fonte de verdade e resultado parcial/unknown definidos |
| Retenção, revogação, criptografia e cópias derivadas | Política por classe/cópia versus regra geral; não escolher prazos | Usuário, Schneier, Kim, Lawrence, Michael | Política/jurisdição aprovadas e aplicada ao backup, réplica, cursor, log, cache e exportação |
| Autoridade, fencing e promoção | Promoção manual condicionada a prova versus destino isolado até contrato completo | Leslie/Martin/Graydon/Michael | Autoridade única, corte, epoch, writers antigos, partição e recuperação definidos |
| Inbox/projeção/checkpoint | Commit atômico se mesma transação; ledger+projetor idempotente se recurso separado | Michael/Martin/Graydon; Momjian revisão operacional | Escolha por serviço/efeito, proprietário e cenários de crash/retry/restore aprovados |
| Migration/backfill/crosswalk e rollback | Preservar envelope e mapear por fonte autoritativa; envelope derivado só com identidade/prova própria | Michael/Fowler/Martin/Leslie | Campos assinados/canonicalização, compatibilidade, corte, quarentena, rollback e retenção definidos |
| Operador, evidência e auditoria | Papel mínimo com processo ainda por definir; sem nomes de plantão | Brendan e usuário | Função, aprovação, escalonamento, acesso/retention e fonte de evidência definidos |
| Erros, limites e retry | Especificar após API/contrato de operação; sem número de tentativas/timeout nesta versão | Graydon/Brendan/Momjian | Tabela de erros, limite, timeout/cancelamento, identidade/retry e reconciliação fechados |

## 10. Mudanças desta versão — DRAFT-0.4

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito candidato e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; casos de borda acrescentados em ADR-058, 059 e 060. Nenhum caso foi executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com negativo e falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`. Nenhum caso foi executado.

Mudanças herdadas de DRAFT-0.2:

- **ADR-057:** posição canônica com partição, tipos de sequência/epoch separados e cursor incompleto/incomparável rejeitado.
- **ADR-058:** seis classes ACK com `ack_class` obrigatório e limite Inbox em `persisted_local`.
- **ADR-061:** Outbox atômica padrão; Inbox separada apenas sem transação comum; replay não repete efeitos irreversíveis; autoridade `stale/unknown` bloqueia mutação forte. Registradas limitações de replicação lógica (DDL/sequences) e RLS como defesa em profundidade.
- **ADR-062:** política de retenção aprovada obrigatória por classe/cópia e falha fechada sem política.
- **G3-02:** cabeçalho com data 2026-11-17.

## 11. Dependências, revisão e estado

- C06: autoridade e fencing (Leslie/Alan); C08: retenção e política de cópias (Schneier/Kim); C10: namespace, epoch, checkpoints e replay (Kleppmann/Graydon); C11: esquema e migrações (Michael); B09/B10: sinais, métodos, restore por cópia e responsáveis (Brendan/Momjian/Michael/Grady).
- Revisão por Michael Stonebraker: **solicitada para este arquivo e hash de finalização**; parecer anterior referia-se aos recortes D2–D4 da contribuição e não satisfaz esta revisão da SPEC.
- Decisões de usuário continuam necessárias para metas, risco, política, retenção e modalidades. Nenhuma decisão é presumida.
- Revisão documental realizada; implementação PostgreSQL, operação, RPO/RTO observados, restore/migration/testes e adequação de produção não verificados. Casos NOT_EXECUTED; IDs de casos NOT_ASSIGNED até índice central; SPEC DRAFT/OPEN.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC, complementares aos casos planejados da §8. `Estado` sempre `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`; nada foi restaurado, promovido ou medido. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN produz bloqueio, negativa ou `unknown`, nunca valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S021-056-P1 | ADR-056 | Schema com perfil publicado N e N-1; backup tomado em N-1 | Restaurar e abrir a cópia com binário N | Com janela declarada no perfil, restore conclui e leitura funciona, com versão de schema registrada; sem janela declarada, `param_not_configured` mantendo o destino isolado e sem conversão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-056-N1 | ADR-056 | Backup em versão de schema fora da janela declarada | Restaurar com o binário atual | `blocked` antes de servir; nenhuma leitura parcial, nenhuma conversão de schema e motivo registrado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-056-N2 | ADR-056 | Migration que reutilizaria número de campo ou alteraria tipo de coluna em uso | Aplicar a migration | Migration rejeitada no check de pré-condição; nenhuma alteração aplicada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-056-B1 | ADR-056 | Tabela com coluna adicional de versão posterior e backup íntegro | Restaurar e ler pelo binário atual | Coluna desconhecida preservada; nenhum dado descartado e semântica conhecida inalterada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-056-F1 | ADR-056 | Banco já migrado para schema novo; binário anterior ao schema é o único disponível | Tentar subir serviço | Serviço permanece isolado com estado explícito de incompatibilidade; nenhuma promoção e nenhum rollback automático de schema | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-057-P1 | ADR-057 | Checkpoint e Inbox com posição completa `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`; versão de cursor e retenção podem não estar declaradas | Retomar sync após restore | Com versão e retenção declaradas, retomada no prefixo contíguo sem duplicata; sem versão de cursor ou janela de retenção declarada, `param_not_configured` mantendo a retomada bloqueada e o destino isolado | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-057-N1 | ADR-057 | Checkpoint/cursor legado sem `partition` | Retomar consumo | Rejeitado; nenhuma tradução e rebootstrap exigido quando política, snapshot e retenção permitirem | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-057-N2 | ADR-057 | Cursor de epoch anterior após troca de `authority_epoch` | Reapresentar o cursor | Rejeição fail-closed; recuperação declarada incompleta e nenhum evento aplicado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-057-B1 | ADR-057 | Restore com lacuna na sequência e eventos posteriores disponíveis | Retomar sync | Checkpoint não salta a lacuna; estado `partial`/`blocked` com o prefixo comprovado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-057-F1 | ADR-057 | Snapshot insuficiente e retenção expirada para o cursor legado | Tentar rebootstrap | Rebootstrap não executado; recuperação incompleta exposta, sem eventos inventados e sem sucesso declarado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-058-P1 | ADR-058 | Cópia com Inbox durável após restore; projeção ainda não executada; nome wire de `ack_class` pode não estar aprovado | Emitir confirmação | Com campo aprovado, `ack_class=persisted_local` limitado ao prefixo comprovado, com escopo e cópia; sem nome wire aprovado (D8), `param_not_configured`/`unknown` e nenhuma classe presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-058-N1 | ADR-058 | Inbox e projeção em estruturas separadas | Emitir `ack_class=materialized` | Recusado; máximo `persisted_local` e projeção em `pending` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-058-N2 | ADR-058 | Restore concluído | Operador infere entrega/atendimento a partir do estado `ready_isolated` | Recusado: `ready_isolated` não é `delivered_to_device` nem `acknowledged_human`; campos permanecem desconhecidos | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-058-B1 | ADR-058 | Restore concluído e validado em `ready_isolated`; projeção pendente | Consultar `ack_class` e estado de materialização | ACK máximo `persisted_local`; `ready_isolated` não é materialização, entrega ou atendimento e a borda entre estados é preservada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-058-F1 | ADR-058 | Restore interrompido após persistir Inbox | Consultar estado da operação | `partial`/`failed` explícito, sem ACK de sucesso e sem cobertura total declarada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-059-P1 | ADR-059 | Grant legítimo e não revogado na cópia restaurada; limite de frescor pode estar declarado ou não | Usar operação condicional após promoção | Com frescor declarado, permitida apenas nos limites do grant enumerado com autorização atual revalidada; sem limite configurado, `param_not_configured` negando a operação, sem TTL presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-059-N1 | ADR-059 | Backup anterior a revogação de grant | Servir a cópia restaurada e reabrir operação coberta pelo grant | Negado; revogação vigente aplicada antes de servir, e o restore não reativa o grant | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-059-N2 | ADR-059 | Grant com epoch incomparável ao restaurado | Reapresentar o grant | Negado com `stale_epoch`; nenhuma tradução de epoch e nenhum efeito | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-059-B1 | ADR-059 | Limite de frescor da operação condicional não declarado (OPEN); cópia restaurada | Executar operação condicional após promoção | Negada com `param_not_configured` informando frescor não definido; nenhum efeito e nenhum TTL presumido | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-059-F1 | ADR-059 | Restore de snapshot anterior à alta marca de contador do vínculo | Operação condicional no destino restaurado | `rollback_suspected`; operação negada sem efeito e revalidação com a autoridade exigida | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-060-P1 | ADR-060 | Backup contém conteúdo E2EE e timeline derivada | Restaurar e servir timeline | Timeline servida sem texto claro e conteúdo E2EE ilegível ao operador; classificação do derivado só com campo aprovado; sem campo aprovado (D9), `param_not_configured`/`unknown` e nenhuma alegação de E2EE | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-060-N1 | ADR-060 | Operador solicita leitura de conteúdo E2EE recuperado do backup | Tentar decifrar ou exportar | Recusado; ausência de chave não é contornada e nenhuma alegação de E2EE é feita sobre o derivado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-060-N2 | ADR-060 | Cópia restaurada sem metadado de confidencialidade | Consumir o artefato | Tratado como `terminated` por falha fechada; PITR/WAL não são aceitos como prova de proteção | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-060-B1 | ADR-060 | Backup com conteúdo E2EE e derivado de perna terminadora; operador autorizado consulta o estado | Comparar classificação do conteúdo original e do derivado | Original permanece ilegível e sem rótulo rebaixado; derivado cai em `terminated` por falha fechada e nenhum rótulo é agregado por backup | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-060-F1 | ADR-060 | Restore traz derivado de bridge/asr anterior à revogação de consentimento | Servir o derivado | Derivado mantido `terminated`; novos segmentos bloqueados; nenhuma promessa de apagar cópia já entregue | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-P1 | ADR-061 | Banco de domínio disponível; outbox na mesma transação | Mutação, commit e publicação | Commit único; publicação uma vez após commit; nenhuma projeção duplicada | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-N1 | ADR-061 | Autoridade `stale`/`unknown` no escopo | Mutação forte após restore | Bloqueada; sem commit e sem promoção dependente | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-N2 | ADR-061 | Inbox em ledger separado com projetor não validado | Promover e declarar `materialized` | Bloqueado; máximo `persisted_local` e promoção mantida em `blocked` até prova idempotente | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-B1 | ADR-061 | Restore concluído; instância antiga ainda ativa | Instância antiga tenta escrever após promoção | Escrita recusada por fence/autoridade; sem escrever em `unknown` e com estado `authority_transfer_unknown` até prova | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-F1 | ADR-061 | Crash entre commit e publication do outbox | Retry do publicador após reinício | Mesmo evento, posição e digest publicados uma vez; nenhum efeito externo repetido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-F2 | ADR-061 | Backup restaurado com Inbox em voo e eventos publicados após o snapshot | Servir o destino restaurado | Reconciliação obrigatória antes de servir; operação dependente bloqueada enquanto houver divergência | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-061-F3 | ADR-061 | Replicação lógica ativa; DDL e sequence não replicados | Promover réplica | Divergência de schema/sequence detectada na validação; promoção bloqueada e reconciliação explícita exigida | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-P1 | ADR-062 | Política aprovada para a classe de dados do backup, réplica e cursor; prazo ou jurisdição podem faltar | Manter cópia e registrar expiração | Com política completa, cópia aceita com `policy_id/version` e prazo por cópia; com prazo ou jurisdição ausentes, `param_not_configured` bloqueando a manutenção fora do mínimo autorizado | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-N1 | ADR-062 | Classe de dados sem política aprovada | Restaurar ou promover a cópia | `blocked`/`unknown`; nenhum dado servido e degradação declarada, preservando só o mínimo do efeito imediato | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-N2 | ADR-062 | Prazo de retenção do backup não decidido (OPEN) | Consultar retenção da cópia | Informa prazo não definido; nenhum prazo presumido e nenhuma cópia declarada conforme | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão; backup abrangido | Expirar prazo durante o hold | Descarte suspenso apenas no escopo do hold; acesso comum segue negado e o hold não reativa grant | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-F1 | ADR-062 | Purge de cópias de backup; uma cópia em armazenamento fora de controle | Consultar resultado | Recibo por cópia e destino; `out_of_control` reportado como limite de controle, sem conclusão global | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-062-F2 | ADR-062 | Backup expirado e restaurado antes da checagem de retenção | Promover o destino | Promoção negada; cópia expirada não é servida e a expiração é registrada na evidência | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-P1 | ADR-063 | Artefato de restore, imagem de banco e script de migration com SBOM, digest e assinatura válida; `generation` acima do piso | Aplicar o procedimento | Assinatura e digest verificados; procedimento aceito com `generation` registrada | P | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-N1 | ADR-063 | Artefato de backup/migration adulterado | Restaurar ou migrar | Rejeitado antes de qualquer escrita no destino | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-N2 | ADR-063 | Dependência nova no pipeline de restore sem versão resolvida, fonte, data, licença, digest e análise de vulnerabilidade | Incluir no pipeline | Pipeline bloqueado; nenhum artefato distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-B1 | ADR-063 | Piso de versão assinado; release atual com `generation` igual ao piso | Executar restore com o artefato atual | Aceito; `generation` e piso registrados na evidência da operação | B | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-F1 | ADR-063 | Release anterior com falha de restore conhecida e `generation` inferior ao piso | Tentar reaplicar o release antigo | Rejeitado pelo piso; versão atual preservada e motivo registrado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S021-063-F2 | ADR-063 | Rollback de release autorizado e limitado no tempo | Aplicar o rollback | Aceito somente com auditoria e piso rebaixado por decisão registrada; sem exceção, rejeitado | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) RPO/RTO numéricos, janelas e agregação; (2) metas por classe e custo versus risco; (3) modalidades de backup, ferramenta e consistência; (4) autoridade/fence e ponto de linearização para promoção (C06); (5) método exato de verificação de integridade e digest; (6) política de retenção por jurisdição e prazos de backup; (7) limites de retry, timeout e expiração de chave de idempotência; (8) campos/enums wire de `confidentiality` (D9) e do nome `replicated_to_authority` (D8).

### Casos por requisito (P/N/B/F)

RNF-005, RNF-006 e RNF-010 são **candidatos** citados na pauta; o vínculo semântico ainda não foi confirmado por Ada (§9), e a tabela registra cobertura provisória, não atribuição. Casos da §8 são citados por título porque não têm ID formal. `n/a` só com motivo. Estado: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RNF-005 (candidato) | §8 "Restore íntegro para escopo elegível" | §8 "Cópia corrompida ou incompleta" | §8 "Restauração parcial" | §8 "Crash em cada fronteira de restore" | Disponibilidade/recuperação; RPO/RTO sem meta aprovada ⇒ resultado é `param_not_configured`/`unknown`, nunca cumprimento. |
| RNF-006 (candidato) | S021-061-P1 | S021-061-N1 | S021-061-B1 | S021-061-F2, S021-061-F3 | Consistência após restore/promoção; fencing e promoção dependem de C06. |
| RNF-010 (candidato) | S021-056-P1 | S021-056-N1 | S021-056-B1 | S021-056-F1 | Compatibilidade e rollback de schema; janela de compatibilidade OPEN. |
| Operação/recuperação sem RF/RNF atribuído | n/a | n/a | n/a | n/a — motivo: requisitos operacionais ainda não mapeados; atribuição pendente em Ada/Margaret (§9). Cobertura documental não é declarada | Requisitos não listados acima não são declarados cobertos por esta SPEC. |

Nenhum caso desta subseção foi executado, restaurado, promovido ou medido.
