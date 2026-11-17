# SPEC-005 — Dados, isolamento e persistência

- **Versão:** DRAFT-0.9, 2026-11-17. **Estado:** DRAFT / OPEN; não aprovada nem pronta para implementação.
- **Autor documental:** Michael Stonebraker. **Revisor independente designado:** Bruce Momjian; revisão independente de DRAFT-0.1 registrada; revisão focal de DRAFT-0.2 realizada; revisão do delta DRAFT-0.3 pendente. **Consolidação:** Alan Turing.
- **Rastreabilidade:** RF-002, comportamento de ativos/dispositivos/endpoints. Interfaces com RF-013/RF-016 e RNF-017 para cópias e sincronização; não se atribui cobertura integral desses requisitos a esta SPEC. Demais requisitos por classe de dado e cópia: OPEN com Ada/produto; não inferir vínculos. IDs de casos: NOT_ASSIGNED. Execução/observado: NOT_EXECUTED.

## 1. Fontes, precedência e evidência

O pedido de consolidação consta em `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/CONSOLIDACAO-CANONICA.md`: relato de Ada da ordem do usuário para criar SPECs; aprovação e implementação não são presumidas. A arquitetura de referência e ADRs prevalecem conforme `docs/README.md`; esta SPEC não altera decisões superiores. A matriz `docs/06-planejamento/matriz-rastreabilidade.md:83–88` indica RF-002 e `:152` enumera plataformas; estados de artefatos nesta SPEC não alteram a matriz por si.

| Fonte lida em 2026-11-16 | Localização e alcance |
|---|---|
| `docs/00-produto/requisitos.md:3,8` | RF-002 e distinção entre direção aprovada e implementação. |
| `docs/02-dominio/modelo-dados.md:5–6,27–48,53,122–159` | Entidades; SQL ilustrativo e incompleto; persistência, namespace, migração e limitações. |
| `docs/01-arquitetura/replicacao.md:23–39,51–55` | Autoridade por escopo; checkpoint, autorização, cópias e diferenças entre ACK e projeção. |
| `docs/07-contratos/eventos-sincronizacao.md:79–105,130–155,173–177` | Namespace e persistência propostos, lacunas, replay, transações e retenção. |
| `docs/04-seguranca/retencao-revogacao.md:1–9,75–90` | Política e estados de cópia propostos; prazos e jurisdição abertos. |
| `docs/06-planejamento/pendencias.md:78–94` | Decisões bloqueadoras de autoridade, retenção, restauração e observabilidade. |
| `Shared/.../SPECS-EM-ANDAMENTO/contribuicao-michael-stonebraker.md` | Insumo D1–D4 e pareceres AT-D1–D4; SHA-256 da versão lida `fa645650155407600c65135df31aa2ee552697b9e502f317e02ce74a2624f352`. |
| `Shared/.../SPECS-EM-ANDAMENTO/contribuicao-bruce-momjian.md:95–112` e re-revisão subsequente | Revisão focal D2–D4 da contribuição, inclusive delta com hash `808eff5da051d90229eca53a64bb4f4e39fa2c3e4367497c0700fc2fd95d595b`; sem objeção documental nesse recorte, não revisão desta SPEC. |

Leitura documental em 2026-11-16. Nenhum banco ou implementação foi inspecionado; nenhum SQL, migração, restore, código ou teste foi executado. Esquema no modelo é ilustrativo, não estado real. Avaliação Jev não executada; isso não altera evidências das fontes. Identificadores ou valores dos exemplos abaixo são sintéticos.

## 2. Finalidade e limites

Definir comportamento proposto do modelo lógico tenant→ativo→dispositivo→endpoint, integridade e autorização por tenant, cópias e retenção, e interface de persistência com sincronização e recuperação. A SPEC-004 é a interface prevista para eventos/sync, SPEC-002 para identidade/autorização, SPEC-020 para política de segurança e retenção, SPEC-021 para operação e restore; sua existência, versão, integração e aprovação devem ser verificadas pela coordenação, não pressupostas. Não define API endpoint existente, mecanismo de autoridade, RPO/RTO, jurisprudência ou tecnologia além das referências conceituais já documentadas.

## 3. Atores, autoridade e escopo

- Principal autenticado, serviço e operador exercem somente ações autorizadas para tenant, recurso e finalidade; contexto de tenant deriva de identidade/vínculo validados no servidor. Um campo fornecido pelo cliente é seleção sujeita a autorização, não prova de pertencimento.
- Um tenant contém ativos; um ativo pode conter dispositivos distintos; cada dispositivo apresenta endpoints de transportes. Identidades internas e relações devem permanecer estáveis sob leitura e recuperação. Identidade de endpoint e cardinalidade inversa permanecem OPEN (§10).
- Nó armazena ou replica somente escopo autorizado; não se torna proprietário do dado por manter cópia. Dados compartilhados, locais ou originados em rede externa não dispensam autorização ou proveniência.
- Administração, leitura, escrita, restauração e promoção de cópia exigem papéis e evidências próprios; regras concretas e política anti-enumeração dependem da SPEC-002 e de decisão de segurança.

## 4. Modelo e invariantes

**Entidades conceituais:** `tenant`, `asset`, `device`, `device_endpoint`, `copy`, e referências versionadas a políticas, eventos e checkpoints. Os campos essenciais de asset/device/endpoint constam em `modelo-dados.md:36–38`. Um identificador externo de endpoint pertence ao escopo do protocolo/origem a decidir; não é identidade global presumida. Existência de endpoint declarado não prova disponibilidade de transporte, capability verificada ou operação bem-sucedida.

**Isolamento:** toda relação deve corresponder a pais e tenant autorizados. Um device não referencia asset de outro tenant; um endpoint não é associado a device não autorizado; leitura e mutação não revelam objetos de outro tenant. Uma forma candidata é integridade referencial composta por tenant e pai, mais autorização e RLS como defesa adicional: a escolha física e os papéis de conexão seguem OPEN. O SQL ilustrativo não demonstra esse invariante; `modelo-dados.md:158–159` registra restrições faltantes. Nenhum índice, RLS ou migração é declarado instalado.

**Consistência e posição (ADR-057):** posição canônica `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. `event_sequence`, `dispatch_sequence`, `dispatch_epoch` e `authority_epoch` são tipos distintos. Cursor opaco versionado vincula principal, escopo, filtros, perfil e retenção; sem partição ou com epoch incomparável, rejeitar sem tradução. Falha anterior ao commit não confirma vínculo completo. Concorrência de associação, reatribuição e exclusão deve produzir resultado consistente ou conflito explícito; precedência segue OPEN.

## 5. Operações, estados, erros e recuperação

| Operação / estado conceitual | Pré-condição e resultado esperado | Negativa e recuperação |
|---|---|---|
| Cadastrar/vincular ativo, device e endpoint | Autoridade e tenant validados; após commit, identidades e pais coerentes podem ser lidos pelo principal autorizado. | Pai ausente ou de outro tenant: recusar sem vínculo nem divulgação. Erro wire e equivalência 403/404: OPEN. |
| Endpoint `registrado` / saúde `unknown` | Estado de registro e evidência de alcance são distintos; sem observação válida, saúde permanece desconhecida. | Offline ou perda de evidência não apaga ou reatribui vínculo. Recuperação atualiza somente com fonte e autorização adequadas. |
| Cópia `autorizada`, `pending`, `unknown` ou `revoked_applied` | Referência de política eficaz por classe, finalidade, cópia e escopo; revogação impede acesso novo conforme política. | Cópia offline ou sem prova permanece `unknown`/pendente; não afirmar exclusão ou revogação global. Estados e semântica definitivos: SPEC-020 OPEN. |
| Cópia `legal_hold` / fora de controle | Tratamento isolado e informação coerente com política aprovada. | Não equivalem a remoção; cópia fora de controle não permite prometer apagamento remoto. |
| Restore isolado / promoção | Origem, integridade, escopo, schema, chaves, autorização, revogações e autoridade são comprovados para o destino. | Falta de prova mantém isolado ou recuperação incompleta; promoção só após decisão autorizada e fencing aplicável. SPEC-021 detalha método. |

Nomes acima são vocabulário conceitual, não enums ou erros wire aprovados. Prazos de retenção seguem a ADR-065 (tabela com finalidade/cópia/jurisdição/gatilho); pendente apenas confirmação jurídica por jurisdição. Conteúdo de auditoria, cancelamento e reatribuição continuam OPEN. Logs e evidências devem minimizar dados sensíveis; não registrar credenciais ou provas reutilizáveis.

## 6. Eventos, cópias e recuperação — interfaces

Conforme ADR-061, mutação de domínio, alocação de posição e Outbox compartilham transação local como padrão; publicação é assíncrona após commit. Inbox em ledger separado só é permitida quando recepção e projeção não podem compartilhar transação. Seu ACK máximo é `persisted_local`. Toda resposta carrega `ack_class` obrigatório, de uma das seis classes: `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device`, `acknowledged_human`, com emissor/evidência/escopo próprios. Replay reconstrói projeção e nunca repete efeitos externos irreversíveis (bridge, RF, pagamento ou notificação). Autoridade `stale/unknown` bloqueia mutação forte. A posição canônica segue ADR-057; cursor sem partição ou com epoch incomparável é rejeitado sem tradução.

No receptor, se Inbox, dedupe, projeção e checkpoint dividem uma transação, o commit é conjunto. Se não dividem, aplica-se a limitação acima. Lacuna ou conflito não avança prefixo; cursor expirado exige snapshot/rebootstrap autorizado quando houver dados e prova suficientes, senão recuperação incompleta.

Para migração eventual, inventariar cópias, versões, consumidores e proveniência antes de presumir legado. Preservar envelope histórico e assinatura/digest; posição derivada e crosswalk autenticado/versionado são proposta quando necessária renumeração. Envelope derivado com prova e proveniência próprias é alternativa aberta. Dado ambíguo fica em quarentena ou bloqueia o corte. Rollback de schema não desfaz eventos aceitos ou efeitos externos; sem preservação/reconciliação de todos os efeitos e prova de autoridade, não se declara rollback seguro. Restore anterior a revogação/corte precisa revalidar controles atuais antes de promoção.

Prazos de retenção por classe/cópia estão definidos na ADR-065 (mensagem 24 m; posição 30 d; áudio não retido; auditoria 12/24 m; backup 35 d), com revisão jurídica por jurisdição e redução permitida por tenant. ADR-066 fixa região primária Brasil (São Paulo) e secundária Colômbia (Bogotá): a réplica entre regiões só existe conforme classe e autorização da ADR-062/065, e a residência de dados é declarada por tenant — conteúdo de classe não autorizada não é copiado para a região secundária. RPO e RTO continuam `OPEN — decisão do usuário`. A medição proposta identifica requisito, janela, origem de evidência, último ponto recuperável e marcos de início e de serviço apto; recuperação parcial contabiliza escopo desconhecido separadamente. Sem denominador, prova ou relógio compatível, o resultado é `unknown`, não cumprimento. ADR-062 exige política aprovada por `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)` antes de persistir; classe sem política falha fechada, degrada explicitamente e conserva apenas o mínimo do efeito imediato autorizado.

## 7. Aplicabilidade de produtos e plataformas

| Superfície | Aplicabilidade proposta |
|---|---|
| Personal, Teams | Relações e acesso aos ativos de seus tenants quando esses fluxos forem oferecidos. |
| Node | Armazenamento e replicação das cópias autorizadas; não propriedade implícita. |
| Platform/SDK | Preservar os contratos quando expuser operações; formato e acesso são de SPEC-003. |
| Servidor Linux | Persistência, controle de acesso e recuperação dos dados sob responsabilidade do serviço. |
| macOS universal arm64/x86_64 e iOS | Cliente deve preservar escopo e estados ao oferecer o fluxo; persistência local depende de SPEC-014/016. |
| Windows e Linux desktop, Android, web | Mesma semântica de autorização e representação quando o fluxo existir; contratos e disponibilidade por plataforma dependem dos autores responsáveis. |

Nenhuma plataforma é excluída globalmente. Um fluxo específico pode ser N/A somente com justificativa aprovada por produto e referência ao contrato; ausência de implementação conhecida não prova N/A.

## 8. Critérios de aceite e casos planejados

Os casos são propostas documentais, **NOT_EXECUTED**; não são rotas existentes nem fixtures criadas. Pré-condições comuns: autorização, política e versões aprovadas para o caso. Os identificadores oficiais dos casos ainda são NOT_ASSIGNED.

1. **RF-002 — cardinalidade positiva:** em tenant T1 autorizado, criar A, associar D1 e D2, e registrar endpoints distintos; leitura retorna uma identidade de A, ambos os devices e cada endpoint sob seu pai, sem alegar conectividade.
2. **Isolamento negativo:** tentar vincular D de T2 a A de T1, consultar endpoint de T1 como principal T2 ou reutilizar conexão com contexto T1; nenhuma relação ou dado de T1 é criado/revelado. Resposta específica depende da política anti-enumeração.
3. **Falha e repetição:** interromper antes/depois do commit de vínculo; não apresentar sucesso parcial nem duplicar efeito sob o contrato idempotente aprovado. Evento externo de resultado desconhecido não é repetido cegamente.
4. **Estado degradado:** endpoint indisponível conserva associação e representa saúde desconhecida ou evidenciada; recuperação não fabrica disponibilidade.
5. **Retenção/revogação:** revogação com réplica offline registra pendência/unknown; não afirma propagação ou descarte global. Restore anterior a revogação permanece isolado até revalidar controles; legal hold e cópia fora de controle não são declarados apagados.
6. **Inbox e replay:** 12 chega antes de 11 com prefixo 10: manter 12 pendente e prefixo 10; reentrega idêntica não reaplica, conflito não sobrescreve; ACK não comprova projeção. Ensaiar falha em cada fronteira quando escolhida.
7. **Migrar/recuperar:** envelope assinado mantém bytes e prova; crosswalk não revalida bytes alterados; registro sem proveniência bloqueia/quarentena. Cursor sem tradução e snapshot ausente resulta em recuperação incompleta, sem pular lacuna.
8. **Corte e restore:** um destino omitido do inventário ou fence desconhecido impede conclusão dependente; após restore antigo, revalidar autoridade e revogação antes de promover. Rollback não descarta evento pós-corte nem repete efeito externo incerto.

## 9. Dependências e critérios de revisão

SPEC-002 define identidade, direitos e prova; SPEC-003 define API, idempotência e erro; SPEC-004 é principal transversal de eventos/sync, namespace, handoff e ACK; SPEC-020 define política por classe/cópia e descarte; SPEC-021 define modalidade, backup/restore e rollback; SPEC-018 coordena medição de recuperação; SPEC-011 cobre dados espaciais; SPEC-014 clientes offline. Referências são IDs atribuídos pelo mapa de consolidação, não confirmação de que os arquivos existam ou de que sejam compatíveis. Leslie, Kim, Graydon, Kleppmann, Paul, Bruce Schneier, Momjian e Brendan revisam respectivas interfaces; Alan consolida.

Critério de prontidão documental: requisitos e plataformas rastreados; decisões da §10 registradas e reconciliadas nas versões revisadas; casos com precondição, estímulo, esperado e falha; fonte e autoridade verificáveis; revisão independente da versão e aprovação do usuário. A existência desta SPEC não fecha planejamento, GMD, B06/B12 ou B09/B10 nem autoriza código.

## 10. Decisões OPEN

| Questão / impacto | Alternativas e recomendação para discussão | Responsável proposto / fechamento |
|---|---|---|
| Cardinalidade asset/device e identidade de endpoint | Device sem ativo ou vínculo obrigatório; chave externa composta ou somente identificador interno. Recomenda-se definir escopo e regra de colisão antes de escolher restrição. | Michael, produto, Graydon; registrar cardinalidade, lifecycle, concorrência, API e aceite por requisito. |
| Isolamento e autorização | Integridade composta + RLS/roles e autorização de serviço versus outras barreiras comprováveis. Recomenda-se defesa em camadas sem presumir implementação. | Michael, Kim, segurança; especificar tenant derivado, papéis, conexão, negação e evidência. |
| Namespace, epoch e commit | Chave namespaceada e vínculo verificável à partição versus alternativa de ordem global reconciliada com replicação. Fronteira Inbox/projeção comum versus ledger recuperável. | Kleppmann, Leslie, Michael, Graydon; conciliar versões C06/C10/C11, exclusão de perfis e falhas de commit. |
| Retenção e cópias | Política por classe/cópia/finalidade: prazos definidos na ADR-065 (tabela com finalidade/cópia/jurisdição/gatilho); pendente apenas confirmação jurídica por jurisdição. Cópia fora da tabela falha fechada até ter política própria. | Usuário, Bruce Schneier, Kim, Lawrence; registrar classe, jurisdição, cópias, hold, revogação e evidência de descarte. |
| Migração, provas e rollback | Envelope original + posição derivada/crosswalk versus envelope derivado com identidade e prova próprias; voltar somente se seguro ou recuperar adiante. | Michael, Momjian, Kleppmann, segurança; provar proveniência, compatibilidade, corte e tratamento de efeitos. |
| RPO/RTO e promoção | Metas por classe/cópia versus estado sem garantia aprovada; promoção após comprovação versus manter isolado. | Usuário/produto, Brendan, Momjian; definir relógios, denominador, janela, prova e risco aprovado. |
| Cobertura e revisão | Completar vínculos adicionais RF/RNF somente com fonte semântica. | Ada, Margaret, Alan; atribuir requisitos e revisão independente, registrar lacunas. |

## 11. Mudanças desta versão — DRAFT-0.7

- Retenção (ADR-065): onde havia “prazos de retenção OPEN”, o texto passa a declarar que os prazos são os da ADR-065 (tabela com finalidade/cópia/jurisdição/gatilho); pendente apenas confirmação jurídica por jurisdição. Acrescenta os casos `S005-065-P1/P2/B1/N1/F1`.
- Residência (ADR-066): região primária Brasil e secundária Colômbia com réplica só por classe e autorização; caso negativo `S005-066-N1` (conteúdo de classe não autorizada não aparece na região secundária).
- Título dos casos por decisão passa a cobrir ADR-056…066. Nenhum valor numérico novo, aprovação ou execução é criado por este delta.

### Herdado de DRAFT-0.6

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito coberto e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; casos de borda acrescentados em ADR-058, 059, 060, 062 e 063. Nenhum caso foi executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com negativo e falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`. Nenhum caso foi executado.

Mudanças herdadas de DRAFT-0.4:

- **ADR-057:** posição completa com partição, distinção entre sequências/epochs e rejeição de cursor incompleto/incomparável.
- **ADR-058:** `ack_class` obrigatório entre seis classes; ACK de Inbox limitado a `persisted_local`.
- **ADR-061:** Outbox na transação de domínio como padrão; Inbox separada apenas sem transação comum; replay sem efeitos externos irreversíveis e bloqueio de mutação forte em autoridade `stale/unknown`.
- **ADR-062:** política por finalidade/classe/cópia/jurisdição/prazo/gatilho obrigatória; persistência falha fechada sem política.
- **G3-02:** cabeçalho datado em 2026-11-17.

## 11. Estado da evidência

Rascunho DRAFT-0.3; revisão de Momjian do delta desta versão **PENDENTE**; revisão focal de insumo não equivale à revisão da SPEC. Aprovação, código, migração, banco e execução: **não demonstrados**. Casos planejados e resultados: **NOT_ASSIGNED / NOT_EXECUTED**. Nada neste texto afirma que o sistema já assegura os invariantes.


## 12. Conciliação da revisão de Momjian — DRAFT-0.2

Revisão focal de DRAFT-0.1 no SHA-256 `5ecbf8c013d3e0a1b1e576a26efb34b105e66e90580e6737f079062cac32d5c9`, em `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-bruce-momjian.md`, seção “Revisão independente focal da SPEC-005”: sem contradição material no recorte e quatro pontos ainda abertos. A revisão de 0.1 não aprova esta versão nem valida implementação.

1. **Chave de posição e dedupe — OPEN:** não classificar como duplicata somente por ID/digest. Exigir mesma posição no namespace e mesmo ID/digest; posição igual com ID ou digest divergente gera conflito, sem avanço de checkpoint. Mesmo ID em posições distintas fica condicionado ao escopo de identidade e eventual crosswalk, não sendo automaticamente duplicata. O vínculo injetivo stream/partição, epoch ausente, perfil versionado e escopo de `event_id` exigem conciliação C10/C11 por Kleppmann, Michael e Leslie.
2. **Fronteira de ACK e projeção — OPEN:** definir por serviço/efeito dono e fronteira física. Se houver transação comum, cobrir Inbox validada, dedupe, projeção e checkpoint contíguo antes de ACK; se separados, definir ledger durável e prova de idempotência/recuperação do projetor, inclusive falha entre efeito e marcador. ACK atesta prefixo Inbox durável, não efeito ou entrega; manter operação dependente bloqueada até contrato e prova aplicável. Casos futuros de crash, retry e restore não foram executados.
3. **Autoridade após restore antigo — OPEN:** antes de promover, comparar a cópia com fonte autoritativa vigente independente do backup, checar autorização/revogações, epoch, inventário aplicável, prova de fence por efetor e corte por namespace. São evidências distintas; checkpoint do consumidor não prova último commit da origem. Se fonte atual ou prova suficiente não estiver disponível, manter destino isolado e estado desconhecido. Leslie/Kim/Kleppmann/Momjian e Michael devem conciliar prova e política; nenhum mecanismo é escolhido.
4. **Retenção das provas — OPEN:** mapear envelope original/derivado, crosswalk, evidências de corte/fence, dedupe e chaves por classe, cópia, finalidade, acesso, dependências criptográficas e descarte; registrar autoridade e versão da política. Ausência de prova, chave ou intervalo impede alegar recuperação ou autenticidade e pode exigir recuperação incompleta. Não se pressupõe retenção indefinida ou apagamento criptográfico comprovado; segurança e produto decidem política, prazo e jurisdição.

Conciliação documental e revisão de Momjian deste delta são pendentes. Nenhum dos quatro pontos fecha B06/B12 ou B09/B10. SPEC DRAFT/OPEN; casos NOT_ASSIGNED, execução NOT_EXECUTED.


## 13. Conciliação de identidade e ACK — DRAFT-0.3

**Fontes:** parecer de Momjian sobre DRAFT-0.2, SHA-256 `9b152cc81b94fbbfdafd1641646eafee04565b17e72bdc05e679cbbf9676fda0`, em `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-bruce-momjian.md`; proposta de Kleppmann em `contribuicoes/proposta-integrada-b06-kleppmann.md:53` e `SPECS-EM-ANDAMENTO/contribuicao-martin-kleppmann.md:23,27`; `docs/07-contratos/eventos-sincronizacao.md:79–87,161–177` e `docs/01-arquitetura/replicacao.md:23–33`. Martin Kleppmann concordou documentalmente com a conciliação em mensagem `debce531-a88d-4dec-ad78-54bc27cf2dcb` de 2026-11-16. A concordância não aprova contrato, mecanismo ou implementação.

**Mesmo event_id em posições distintas:** Momjian recomenda conflito/quarentena salvo crosswalk autorizado. C10 e a proposta de Kleppmann mantêm OPEN o escopo de identidade entre posições; portanto, propõe-se **quarentena preventiva**, não afirmar conflito definitivo nem novo evento, e não aplicar ou avançar o prefixo afetado até verificar escopo e eventual crosswalk autenticado e autorizado. Crosswalk não comprova equivalência automaticamente; política de retenção eficaz deve cobrir a quarentena e suas evidências, e sua ausência bloqueia persistência afetada. Classificação final e prova necessárias permanecem OPEN com Kleppmann, Michael e segurança.

**ACK e materialização:** Momjian propõe explicitar a materialização quando o perfil usa commit comum. Aceita-se que o commit de Inbox validada, dedupe, projeção e checkpoint é pré-condição para emitir ACK nesse perfil. Entretanto, `replicacao.md:33` define o significado do ACK como prefixo Inbox durável, não sucesso de toda materialização. Assim, o perfil documenta o predicado transacional de emissão sem ampliar automaticamente a semântica wire do ACK para entrega, materialização global ou efeito externo. Em ledger/projeção separados, ACK pode seguir Inbox durável e preceder projeção, que deve ser recuperável e idempotente. Fronteira por serviço e eventual recibo adicional seguem OPEN e sujeitos à conciliação C10/C11/C07.

**Casos planejados:** (1) mesmo ID em posições distintas e escopo/crosswalk desconhecido: quarentena com política eficaz, sem aplicar ou avançar o prefixo afetado; sem política, bloquear persistência; (2) commit comum falha antes de concluir projeção ou checkpoint: sem ACK; (3) Inbox durável com projeção separada pendente: ACK limitado ao prefixo, sem alegar projeção. Casos NOT_ASSIGNED / NOT_EXECUTED. Revisão independente deste delta solicitada; SPEC permanece DRAFT/OPEN, sem aprovação ou execução.

## Casos por decisão (ADR-056…066)

Casos locais desta SPEC. `Estado` sempre `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`; nenhum banco, migração ou fixture foi criado. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN produz falha fechada ou informação de desconhecido, nunca valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S005-056-P1 | ADR-056 | Schema persistido com perfil publicado N e N-1; registro gravado em N-1 | Ler e mutar o registro com binário N-1 e com binário N | Com janela declarada no perfil, ambos leem e escrevem e a versão de schema é registrada; sem janela declarada, `param_not_configured` recusando a operação, sem aceite por padrão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-056-N1 | ADR-056 | Registro gravado em versão fora da janela declarada | Abrir o registro com binário atual | Recusa antes de qualquer efeito, com erro explícito de incompatibilidade; nenhuma reinterpretação de campo e nenhuma conversão silenciosa | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-056-N2 | ADR-056 | Migração que reutilizaria número de campo já usado ou alteraria tipo de campo existente | Aplicar a migração | Migração rejeitada no validador de schema; nada aplicado e número de campo permanece reservado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-056-B1 | ADR-056 | Registro com campo desconhecido de versão posterior | Reescrever o registro pelo binário atual | Campo desconhecido preservado; nenhum dado descartado e semântica conhecida inalterada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-056-F1 | ADR-056 | Tabela já escrita no schema novo; rollback do binário para versão anterior | Retomar serviço e ler tabela | Serviço mantido isolado com estado explícito de incompatibilidade; nenhuma leitura parcial de colunas e nenhuma migração reversa automática | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-057-P1 | ADR-057 | Chave de posição completa `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` | Gravar e retomar por checkpoint | Registro único por posição; retomada no prefixo contíguo sem colisão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-057-N1 | ADR-057 | Chave de posição gravada sem `partition`, herdada de versão anterior | Consultar e avançar checkpoint | Consulta rejeitada; nenhuma tradução da chave e nenhum checkpoint avançado | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-057-N2 | ADR-057 | Duas partições P1 e P2 com mesmo `(origin_node_id, tenant_id, stream_id, authority_epoch, seq)` | Tentar gravar ambas | Duas linhas distintas aceitas; nenhuma sobrescrita por chave incompleta e nenhuma colisão silenciosa | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-057-B1 | ADR-057 | Cursor opaco vinculado a principal, escopo, filtros, perfil e retenção | Reutilizar cursor de outro principal no mesmo escopo | Rejeitado; nenhuma ampliação de acesso e nenhum evento retornado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-057-F1 | ADR-057 | `authority_epoch` trocado; registros do epoch anterior permanecem | Reabrir escrita sobre o escopo | Escrita antiga recusada; estado de recuperação incompleta exposto e rebootstrap exigido quando snapshot/prova faltar | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-058-P1 | ADR-058 | Mutação de domínio commitada localmente, sem projeção; nome wire de `ack_class` pode não estar aprovado | Ler a resposta | Com campo aprovado, `ack_class=persisted_local` e projeção separada como `pending`; sem nome wire aprovado (D8), resposta marca `param_not_configured`/`unknown` sem classe presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-058-N1 | ADR-058 | Inbox em ledger separado da projeção | Tentar responder `ack_class=materialized` | Recusado; máximo `persisted_local` e registro da tentativa de escalada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-058-N2 | ADR-058 | Resposta de confirmação sem `ack_class` | Cliente/SDK processa a resposta | Resposta inválida; nenhuma classe presumida e estado local `unknown` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-058-B1 | ADR-058 | Inbox em ledger separado confirmou o prefixo; projeção pendente | Consultar `ack_class` da cópia e da projeção | ACK máximo `persisted_local`; projeção permanece `pending` e a borda entre classes não é apagada por tempo ou retry | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-058-F1 | ADR-058 | Commit local concluído e resposta perdida | Repetir a operação com a mesma chave de idempotência | Estado conhecido retornado; nenhum segundo registro e nenhuma classe elevada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-059-P1 | ADR-059/064 | Grant offline válido, operação enumerada; limite de frescor pode estar declarado ou não | Registrar observação de localização local | Com frescor declarado, registro local aceito e enfileirado sem divulgação nem cópia federada; sem limite configurado, `param_not_configured` negando o registro, sem TTL presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-059-N1 | ADR-059 | Grant offline válido | Tentar gravar ou alterar grant, vínculo de tenant ou ACL | Negado antes do commit; nenhuma linha criada ou alterada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-059-N2 | ADR-059 | Cópia restaurada cuja evidência de contador é anterior à alta marca | Gravar dado condicionado ao grant | Negado com `rollback_suspected`; nenhum registro persistido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-059-B1 | ADR-059 | Limite de frescor da operação condicional não declarado (OPEN) | Gravar dado condicionado ao grant offline | Negado com `param_not_configured` informando frescor não definido; nenhum registro persistido e nenhum TTL presumido | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-059-F1 | ADR-059 | Estado de revogação desconhecido no destino; cópia local disponível | Abrir acesso a dado protegido por grant | Negado com `revocation_state_unknown`; nenhuma leitura servida e nenhuma exceção temporária | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-060-P1 | ADR-060 | Cópia primária de conteúdo protegido por MLS; derivado gerado por bridge | Consultar a cópia e o derivado | Cópia sem texto claro no servidor; derivado reporta confidencialidade só com campo aprovado; sem campo aprovado (D9), `param_not_configured`/`unknown` e nenhuma alegação de E2EE na API ou na UI | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-060-N1 | ADR-060 | Cópia de derivado sem metadado de confidencialidade | Ler a cópia | Tratada como `terminated` por falha fechada; ausência de metadado nunca lida como E2EE | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-060-N2 | ADR-060 | Backup de conteúdo E2EE sem chave disponível | Tentar recuperar texto claro a partir do backup | Recusado; nenhuma promessa de leitura sem chave e nenhum registro de conteúdo em log | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-060-B1 | ADR-060 | Duas cópias do mesmo conteúdo: original MLS e derivado de perna terminadora | Ler ambas e comparar a classificação | Original permanece protegido e ilegível ao serviço; derivado cai em `terminated` por falha fechada, sem agregar rótulo por objeto e sem apagar a cópia original | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-060-F1 | ADR-060 | Membro removido do grupo; mensagens posteriores gravadas | Entregar mensagens futuras ao removido | Acesso negado pelas chaves atuais; derivados anteriores mantêm `terminated` e cópias já entregues não são declaradas apagadas | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-P1 | ADR-061 | Banco de domínio disponível | Mutação que grava estado, posição e Outbox | Commit único; publicação após commit uma vez; leitura do estado coerente | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-N1 | ADR-061 | Autoridade `stale` ou `unknown` | Mutação forte | Bloqueada sem commit e sem ACK de sucesso; estado explícito de bloqueio | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-N2 | ADR-061 | Projeção em ledger separado | Tentar ACK `materialized` como prova de projeção | Recusado; ACK máximo `persisted_local` e projeção reportada `pending` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-B1 | ADR-061 | Outbox publicado e estado restaurado de backup anterior à publicação | Reconciliar após restore | Divergência entre estado restaurado e eventos publicados detectada; operação dependente bloqueada e reconciliação exigida | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-F1 | ADR-061 | Crash entre commit e publicação | Retry do publicador | Mesmo evento, posição e digest publicados uma vez; nenhuma duplicata e nenhum efeito externo repetido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-061-F2 | ADR-061 | Projeção interrompida após efeito externo e antes do marcador | Reconhecer e reexecutar | Estado `failed_retryable`; retry apenas idempotente; nenhum segundo efeito presumido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-P1 | ADR-062 | Política aprovada para `(finalidade, classe, cópia, jurisdição, prazo, gatilho)`; prazo ou jurisdição podem faltar | Gravar dado pessoal da classe | Com tupla completa, persistência aceita com `policy_id/version` e tupla registrados; com prazo ou jurisdição ausentes, `param_not_configured` com falha fechada e nada persistido além do mínimo autorizado | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-N1 | ADR-062 | Classe de dado sem política aprovada | Tentar inserir registro | Falha fechada com erro explícito; nenhuma linha persistida além do mínimo do efeito imediato e operação degradada declarada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-N2 | ADR-062 | Prazo ou jurisdição da classe não decididos (OPEN) | Consultar ou gravar a classe | Negado e informado como política incompleta; `null` não tratado como infinito | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-B1 | ADR-062 | Política com prazo definido e jurisdição ausente para a cópia | Gravar e consultar retenção da cópia | `param_not_configured` para a dimensão ausente; nenhum prazo presumido, nenhum `null` tratado como infinito e registro limitado ao mínimo do efeito autorizado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-F1 | ADR-062 | Prazo expirado em uma cópia; réplica offline | Executar descarte por cópia | Recibo por destino; réplica reportada `pending`/`unknown`, nunca `delete_confirmed`; nenhuma conclusão global | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-062-F2 | ADR-062 | Hold com autoridade, escopo e revisão; cópia fora de controle | Consultar estado durante o hold | Descarte suspenso só no escopo do hold; `out_of_control` reportado como limite de controle e acesso comum segue negado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-P1 | ADR-063 | Imagem de banco e artefato de migração com SBOM, digest registrado e assinatura válida; `generation` acima do piso | Aplicar artefato em ambiente de validação | Assinatura e digest verificados; aplicação aceita com `generation` registrada | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-N1 | ADR-063 | Artefato de migração adulterado | Aplicar | Rejeitado antes de alterar qualquer estrutura; nenhuma instrução executada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-N2 | ADR-063 | Dependência nova de migração sem versão resolvida, fonte, data, licença, digest e análise de vulnerabilidade | Adicionar ao build | Entrada bloqueada; nenhum artefato distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-B1 | ADR-063 | Piso de versão assinado; artefato de migração atual com `generation` igual ao piso | Aplicar o artefato | Aceito; piso e `generation` registrados e nenhum rebaixamento implícito | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-F1 | ADR-063 | Migração anterior com falha conhecida e `generation` inferior ao piso | Tentar reaplicar a versão antiga | Rejeitado pelo piso; versão atual preservada e motivo registrado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-063-F2 | ADR-063 | Rollback de schema autorizado e limitado no tempo | Aplicar rollback aprovado | Aceito somente com auditoria e piso rebaixado por decisão registrada; sem isso, bloqueado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-065-P1 | ADR-065 | Classe e cópia com entrada na tabela da ADR-065 (finalidade, cópia, jurisdição, prazo, gatilho) e política `effective` | Gravar dado pessoal da classe | Gravação aceita com `policy_id/version`, classe, cópia, prazo e gatilho registrados; expiração agendada pelo gatilho declarado e nenhum prazo emprestado de outra classe | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-065-P2 | ADR-065 | Registro da classe com prazo da ADR-065 vencido no gatilho declarado (`observed_at`, `session_closed_at`, `incident_closed_at` ou equivalente da tabela) | Executar o descarte por cópia | Conteúdo descartado no gatilho e descarte verificado e registrado com classe, cópia, política, instante e resultado; leitura posterior negada e réplicas reportadas separadamente | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-065-B1 | ADR-065 | Tenant declarou redução de prazo para uma classe/cópia, dentro do permitido | Gravar e consultar retenção da cópia daquele tenant | Prazo reduzido aplicado só ao escopo daquele tenant; outro tenant mantém o prazo padrão; ampliação sem nova decisão registrada é recusada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S005-065-N1 | ADR-065 | Cópia sem jurisdição declarada (campo vazio/desconhecido) | Gravar ou replicar dado pessoal nessa cópia | Falha fechada: gravação/replicação negada com `RETENTION_SCOPE_UNRESOLVED`; nenhum `null` tratado como retenção indefinida e nenhum prazo presumido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-065-F1 | ADR-065 | Hold legal com autoridade, escopo e revisão declarados sobre a classe/cópia | Atingir o prazo de expiração durante o hold | Descarte suspenso apenas no escopo do hold, com `DATA_UNDER_LEGAL_HOLD`; revisão de 90 dias registrada e acesso comum mantido negado; ao final do hold o prazo volta a correr e o descarte é verificado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S005-066-P1 | ADR-066 | Tenant com residência declarada e classe autorizada a replicar entre regiões | Replicar para a região secundária (Colômbia/Bogotá) | Réplica criada só para a classe autorizada, com região, cópia, política e residência declaradas na API; primária Brasil (São Paulo) registrada na evidência | P | NOT_EXECUTED / NOT_ASSIGNED |
| S005-066-N1 | ADR-066 | Classe sem autorização de replicação regional | Consultar a região secundária pelo conteúdo daquela classe | Conteúdo **não** aparece na região secundária: nenhum registro, nenhuma posição servida e nenhuma cópia derivada; consulta negada sem revelar existência | N | NOT_EXECUTED / NOT_ASSIGNED |
| S005-066-F1 | ADR-066 | Região primária indisponível com réplica secundária existente | Consultar conteúdo de classe não replicada | Continuidade apenas para as classes replicadas; demais classes permanecem indisponíveis e declaradas como tal, sem criação de cópia de emergência não autorizada | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) janela N/N-1 do schema persistido; (2) escolha física de chave, RLS e papéis de conexão; (3) RPO/RTO (os prazos de retenção por classe/cópia seguem a ADR-065, com confirmação jurídica por jurisdição pendente); (4) precedência de concorrência em reatribuição e exclusão; (5) cardinalidade inversa e identidade de endpoint; (6) estados e semântica definitivos de cópia (`unknown`, `revoked_applied`, `legal_hold`) — OPEN em SPEC-020; (7) campo `confidentiality` e nome wire de `replicated_to_authority` (D8/D9); (8) entidade controladora, foro e bases legais por finalidade da ADR-066.

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. RF-002 tem cobertura própria; RF-013, RF-016 e RNF-017 são tratados apenas como interface, e as células refletem esse limite. Casos da §8 são citados por número porque ainda não têm ID formal. `n/a` só com motivo. Estado: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-002 | §8 caso 1 (cardinalidade positiva tenant→ativo→device→endpoint) | §8 caso 2 (isolamento entre tenants) | §8 caso 4 (endpoint indisponível conserva associação e saúde desconhecida) | §8 caso 3 (interrupção antes/depois do commit de vínculo) | Cobertura de ativos/dispositivos/endpoints; erro wire e equivalência 403/404 OPEN. |
| RF-013 (interface) | S005-057-P1 | S005-057-N1 | §8 caso 6 (12 antes de 11 mantém prefixo 10) | S005-061-F1 | Só o subescopo de persistência/posição; semântica de sync em SPEC-004. |
| RF-016 (interface) | S005-058-P1 | S005-058-N1 | §8 caso 5 (réplica offline permanece pendente/unknown) | S005-061-F2 | Réplica e ACK por cópia; não cobre a jornada de degradação. |
| RNF-017 (interface) | S005-059-P1 | S005-059-F1 | §8 caso 5 (restore anterior a revogação permanece isolado) | §8 caso 8 (restore antigo revalida autoridade e revogação antes de promover) | Apenas subescopo de cópia/isolamento; grants e revogação em SPEC-002/020. |

Fora desta tabela: RPO/RTO não têm meta aprovada, portanto não existe caso positivo de cumprimento; qualquer medição futura reporta `param_not_configured`/`unknown` (§6). Prazos de retenção têm caso próprio em ADR-065 (`S005-065-*`) e residência em ADR-066 (`S005-066-*`). Nenhum caso desta subseção foi executado.
