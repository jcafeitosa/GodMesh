# SPEC-022 — Produto: conta, organizações e governança

**Identificador:** SPEC-022
**Versão:** DRAFT-0.4
**Data:** 2026-11-17
**Estado:** DRAFT / OPEN. Não revisada, não aprovada, não autoriza implementação.
**Autora documental:** Margaret Hamilton (pod P5 — Emergência, produto e jornadas)
**Revisão independente designada:** Ada Lovelace — revisão desta versão **pendente**.
**Casos desta SPEC:** planejados. Identificadores formais de teste `NOT_ASSIGNED`. Nenhum caso executado.
**Mudanças desta versão:** casos por decisão (resposta ao veto C3-1); casos por requisito e casos P desbloqueados (C3-1); DRAFT-0.4 acrescenta ADR-064 (aviso “modo offline indisponível” antes do incidente) e ADR-065 (retenção no encerramento com prazo definido) às §3.1 e aos casos (`S022-064-*`, `S022-065-*`) e troca “conforme política” por critério observável em `P22-C03`.

## 1. Identificação, versão, estado, autoria, revisão e fontes com localizadores

Este cabeçalho cumpre o item 1 de `SPECS.md` §Conteúdo obrigatório. Autoria, parecer e atividade não equivalem a aprovação (`SPECS.md`, §Estados e liberação). A revisão de qualquer versão anterior não vale para esta.

Fontes normativas e locais consultados (caminhos relativos à raiz do repositório):

| Fonte | Localizador | Uso nesta SPEC |
|---|---|---|
| Instruções do repositório | `AGENTS.md:1–34` | Precedência, proibição de inventar parâmetros, distinção proposta/decisão/implementação |
| Catálogo de SPECs | `SPECS.md`, §Conteúdo obrigatório, §Estados e liberação, §Pendências desta consolidação | Estrutura obrigatória e critério de conclusão |
| Equipe e gates | `docs/09-equipe/EQUIPE.md:1–192`, §§1, 2, 5, 7, 11 | Papéis, autoridade, vetos em vigor, DoD de SPEC |
| Registro de gaps | `docs/09-equipe/registro-gaps.md` §5 (U1–U7), §6 (G1-01, G1-02, G1-03, G1-06, G1-10), §11 | Origem do escopo, donos e ordem de execução |
| Visão de produto | `docs/00-produto/visao.md:1–49`, §Produtos, §Capacidades integrais | Personal/Teams/Node e fronteira de ofertas |
| Requisitos | `docs/00-produto/requisitos.md` RF-001, RF-020, RNF-003, RNF-015 | Requisitos cobertos e limites de cobertura |
| Decisões | `docs/06-planejamento/decisoes.md` ADR-004, ADR-034, ADR-035, ADR-036, ADR-056…ADR-065 | Decisões rastreáveis, incluindo ACK (058), retenção (062/065) e aviso de SOS offline (064) |
| Pendências | `docs/06-planejamento/pendencias.md` §Direções de produto (itens 1, 2, 3, 4, 5), §Autorização posterior | Direções aprovadas do usuário e gate de planejamento |
| Identidade e autorização | `docs/08-specs/SPEC-002-identidade-autorizacao.md` §§3, 4, 5.1, 5.3, 6, 7, 8, 9, 10, 13 | Grants, estados de credencial, provas, migração e erros |
| Autoridade de contratos | `docs/07-contratos/autoridade-contratos.md` §1 (ACK), §2 (enums), §3 (confidencialidade), §4 (D1–D9) | Autoria única de enum; não duplicar ACK nem estado de credencial |
| Guardian (interface) | `docs/08-specs/SPEC-012-produto-guardian.md` §§2, 4, 8 | Fronteira entre papel administrativo e direitos do titular |

Nenhum documento externo não versionado foi usado como prova. Contribuições citadas por outras SPECs (`SPECS-EM-ANDAMENTO/…`) não foram lidas e não sustentam nenhuma afirmação aqui.

## 2. Objetivo, requisitos cobertos, escopo, plataformas e exclusões justificadas

### 2.1 Objetivo

Definir contratos documentais para: criação de conta e primeiro acesso; recuperação de acesso por provas previamente cadastradas; ciclo de vida de organização (criação, convite, aceite, expiração, revogação, saída e remoção); catálogo de papéis e matriz de permissão por operação; relação entre papéis de organização e grants da SPEC-002; encerramento e exclusão sob retenção fail-closed; e contestação e suporte. Esta SPEC **não** define identidade criptográfica, wire, algoritmos nem prazos.

### 2.2 Requisitos cobertos (cobertura proposta e parcial)

| Requisito | Subescopo nesta versão | Limite explícito |
|---|---|---|
| RF-001 | Ciclo de vida do principal *pessoa* (conta, primeiro acesso, recuperação) e do principal *organização* (`tenant_id`), na fronteira de produto | Cobertura **parcial**. A semântica de identidade tipada, credencial, vínculo `(iss, sub)`, validação e autorização pertence à SPEC-002 e não é redefinida aqui. Esta SPEC não fecha RF-001 |
| RF-020 | Jornadas de **Teams**: organização, convite, papéis, governança, encerramento; fronteira com Personal e Node | Cobertura **parcial**. Personal e Node são tratados apenas onde a conta/organização é condição de uso; estimativa, perfis de capacidade e operação ficam em SPEC-012/SPEC-019 |
| RNF-003 | Isolamento multi-tenant aplicado a operações de organização: tenant derivado no servidor, papel avaliado por operação | Parcial. Não cobre isolamento de dados em repouso, RLS nem retenção, que pertencem a SPEC-002/005/020/021 |
| RNF-015 | Proteção de informação administrativa por função e tenant, com retenção declarada | **Interface apenas**. O objeto "AP e visitantes" e sua retenção pertencem a SPEC-009/020. Esta SPEC fornece a matriz de papel/tenant e a regra fail-closed, sem reivindicar o requisito |

Vínculos a requisito continuam "A conciliar" em `SPECS.md`; esta seção registra proposta de autoria, não cobertura semântica nem aceite.

### 2.3 Escopo

1. Conta de pessoa: criação, verificação de vínculo, primeiro acesso, estados e encerramento.
2. Recuperação de acesso sem emissor atual, restrita a provas previamente cadastradas.
3. Organização: criação/bootstrap, convite, aceite, expiração, revogação, saída, remoção, transferência de titularidade e encerramento.
4. Catálogo fixo de papéis: `owner`, `admin`, `operator`, `auditor`, `member`, `guest`, com matriz por operação.
5. Relação papel ↔ grant/ACL da SPEC-002 (pré-condição, não substituto).
6. Contestação e suporte: canais, estados e limites de adjudicação.
7. ACK por classe (ADR-058) onde há entrega de convite ou notificação administrativa.
8. Retenção fail-closed (ADR-062) no encerramento de conta/organização.

### 2.4 Plataformas

A semântica é comum. Superfícies previstas: servidor Linux/GodMesh Node, macOS (arm64/x86_64), Windows, Linux desktop, iOS, Android e web, se oferecida (`visao.md` §Plataformas aprovadas; ADR-055). Cada plataforma precisa demonstrar autenticação, armazenamento protegido, relógio e ciclo de vida de sessão antes de afirmar suporte; nenhuma linha implica API ou versão homologada. Papéis administrativos podem existir em qualquer cliente; operações de servidor não têm superfície própria no cliente, mas os clientes dependentes continuam aplicáveis.

### 2.5 Exclusões justificadas

| Exclusão | Justificativa e dono |
|---|---|
| Planos, preços, faturamento e limites comerciais | G1-04; decisão de produto U4 do usuário. Inventar preço ou limite é proibido aqui |
| Portal de autoatendimento de organizações | G1-05; backlog pós-v1, P7 |
| SCIM/SAML e ciclo de vida corporativo | G2-09/G2-10; P1, dependente de SPEC-002/003 |
| Importação, exportação e portabilidade | G1-09; depende de U2 (retenção) e de P3 |
| Catálogo de dispositivos e compatibilidade visível | G1-07; derivado de RF-003 em SPEC-023/P6 |
| Preferências globais de notificação, i18n/RTL, busca em histórico | G1-08, G1-12, G1-13; P7/P3 |
| Moderação, abuso, menores de idade, jurisdição e base legal | G5-01…G5-14; U1/U5, revisão jurídica pendente |
| Papéis personalizados por tenant | Não decidido; catálogo fixo nesta versão (OPEN-06) |
| Wire, schema, códigos de erro e mapeamento HTTP | Graydon (SPEC-003); aqui só rótulos lógicos |
| Conteúdo E2EE, ASR, localização precisa e RF | Bruce/Kim/Radia/Daniel; ADR-060 e SPEC-020/008/011 |

## 3. Decisões rastreáveis, alternativas, consequências, divergências e dependências

### 3.1 Decisões rastreáveis

| Decisão | Como se aplica aqui |
|---|---|
| ADR-004 (núcleo multi-tenant para Personal, Teams e Node) | Organização é `tenant_id`; Personal é tenant próprio; Node opera tenants sob sua autoridade, não como papel de organização |
| ADR-034 (GMI imutável, OIDC `iss`+`sub`, WebAuthn/passkeys) | Conta é o invólucro de produto do GMI; login não cria vínculo nem membership. `(iss, sub)` é a prova de entrada; e-mail, handle e nome não |
| ADR-035 (GMC pseudônimo, `acct:` opt-in) | Convite e listagem de membros apresentam GMC/alias autorizado, não identidade civil |
| ADR-036 (indicativo em mensagem, rejeição de impersonação) | Aceite de convite não pode ser forjado por indicação de identidade no corpo da requisição |
| ADR-058 (taxonomia de ACK) | Convite e notificação administrativa carregam `ack_class`; nenhuma classe superior é inferida |
| ADR-062 (retenção fail-closed) | Encerramento/exclusão não apaga classe sem política aprovada; ausência de política falha fechado |
| ADR-065 (política inicial de retenção por classe) | O encerramento aplica o prazo da classe na tabela da ADR-065 (conta encerrada com carência declarada; cópias em backup expiram no ciclo da classe), com prazo por classe e por cópia, redução permitida ao tenant e descarte verificado e registrado |
| ADR-064 (SOS offline: negar sem fonte de frescor confiável) | O produto declara “modo offline indisponível neste aparelho” **antes** do incidente, quando não há F1/F2 nem verificação online; organização não promete em contrato, convite ou material de onboarding uma operação offline que o aparelho do membro não pode sustentar, e o caminho oferecido é o discador do sistema |
| ADR-056 (N/N-1 por perfil; rejeição antes do efeito) | Cliente incompatível com a versão de contrato de organização é rejeitado antes de criar/alterar membership |
| ADR-061 (outbox/Inbox, replay não reemite efeito externo) | Convite/notificação reenviada por replay não gera segunda mensagem externa nem segundo convite válido |
| ADR-059 (grant offline limitado) | Nenhuma mutação de membership, convite, papel ou recuperação é autorizada offline |
| ADR-060 (confidencialidade e proibição de alegação) | Dados administrativos não são apresentados como E2EE; conteúdo de canal continua sob SPEC-020 |

### 3.2 Alternativas consideradas

| Alternativa | Consequência | Situação |
|---|---|---|
| Convite por e-mail como prova de titularidade | Simples, mas contradiz SPEC-002 (e-mail não é chave estável nem prova). Rejeitada como prova | E-mail permanece apenas canal de entrega |
| Membership automática por domínio DNS/e-mail corporativo | Cria conta órfã e escalada por domínio. Exige verificação de domínio com autoridade, fora do escopo (OPEN-07) | Não adotada |
| Papéis personalizáveis desde o v1 | Aumenta superfície de autorização e conflita com matriz auditável | Não adotada nesta versão (OPEN-06) |
| Adjudicação automática de contestação por timeout | Transforma silêncio em aprovação; viola a direção "estado desconhecido não vira sucesso" | Rejeitada |
| Encerrar organização apagando tudo imediatamente | Viola ADR-062 e hold legítimo | Rejeitada; encerramento é fail-closed e explícito |
| Recuperação assistida excepcional (suporte "reseta" a conta) | Não aprovada em `pendencias.md` §Direções de produto item 5; abre impersonação | Não autorizada |
| Múltiplos owners obrigatórios | Reduz risco de órfão, mas cria fricção e não foi decidido | OPEN-03 |

### 3.3 Consequências

Operações de organização passam a exigir papel + tenant + finalidade, não apenas sessão válida. Encerramento pode ser **negado ou degradado** enquanto faltar política de retenção por classe — inclusive impedindo exclusão imediata de dados legítimos. Convites não entregues não podem ser tratados como pendentes para sempre: precisam de expiração declarada (valor OPEN).

### 3.4 Divergências e conflitos registrados

| ID | Divergência | Estado |
|---|---|---|
| DV-22-1 | `docs/07-contratos/autoridade-contratos.md` §2 (D7) registra enum de credencial definido duas vezes (SPEC-002 × `identidade-autorizacao.md`). Esta SPEC **não** redefine estado de credencial; referencia SPEC-002 §5.1 | Aberta; dono Kim/Bruce |
| DV-22-2 | G3-06/D2: vocabulário de ACK duplicado em domínios. Esta SPEC usa exclusivamente as seis classes da ADR-058 e não cria sinônimo | Aberta; dono C2/Graydon |
| DV-22-3 | G3-07: enums repetidos sem tabela de autoridade. O catálogo de papéis aqui é **proposta nova**, sem enum anterior a conciliar; ainda assim não é wire nem valor ratificado | Aberta; dono Graydon/P5 |
| DV-22-4 | D3: `ack_status` de check-in Guardian sem enum definido. Esta SPEC não usa esse campo nem cria equivalente | Aberta; dono SPEC-012/domínio Guardian |
| DV-22-5 | `EQUIPE.md` §11 mantém vetos C1-1, C3-1, C4-1 em vigor. Esta SPEC não levanta veto algum | Vigente |

### 3.5 Dependências

SPEC-002 (identidade, grants, provas, erros, partição), SPEC-003 (wire/erros), SPEC-004 (eventos, idempotência, replay), SPEC-005/020/021 (retenção, segurança, restore), SPEC-012 (fronteira titular × administrador no Guardian), SPEC-013 (UX de convite/estado), SPEC-015/016 (armazenamento de chave e lifecycle por SO), SPEC-018 (readiness, suporte operacional), SPEC-023 (capacidades e conectividade na recuperação). Nenhuma dessas dependências está aprovada.

## 4. Estados e transições, contratos, dados, erros, permissões e limites

### 4.1 Conta e primeiro acesso

Estados propostos da conta de pessoa (rótulos lógicos, sem wire): `invited_by_org`, `pending_first_access`, `active`, `suspended`, `closure_requested`, `closed`. A transição `pending_first_access → active` exige vínculo `(iss, sub)` validado pela autoridade de identidade e prova de posse aceita pela política da operação (SPEC-002 §§4, 5.1); não é concluída por simples presença de e-mail, GMC ou indicação do cliente.

Primeiro acesso, em ordem: (1) autenticação no provedor aceito; (2) validação do vínculo e derivação do `gmi_id` no servidor; (3) escolha de contexto entre vínculos **já permitidos** — sem tenant padrão ambíguo; (4) cadastro de prova(s) de recuperação enquanto a titularidade é verificável (SPEC-002 §8); (5) aceite das políticas e base legal aplicável, com registro de versão aceita. Sem etapa 4 válida, a conta pode operar, mas a recuperação posterior fica **negada** — e a UI precisa dizer isso.

Conta não é criada por convite de organização: o convite exige principal existente ou cria apenas um `invite` pendente. `invited_by_org` não concede acesso a nada além da página de aceite.

### 4.2 Recuperação por provas previamente cadastradas

Direção aprovada do usuário: provas previamente cadastradas; sem prova válida a identidade **não é transferida** (`pendencias.md` §Direções de produto item 5). Regras herdadas de SPEC-002 §8 e §5.3, não redefinidas aqui:

- Prova é cadastrada antes da perda, vinculada a titular, classe, emissor, finalidade, desafio e pedido; repetição é consumida/rejeitada.
- Prova não cadastrada, expirada, reutilizada, comprometida, de titular errado ou conflitante não transfere identidade.
- Pedido de recuperação nunca cadastra o fator que aprovará o mesmo pedido.
- Estados candidatos: `requested`, `proof_pending`, `recovery_pending`, `contested`, `approved`, `rejected`, `transferred`, `closed` — não ratificados.
- Nenhum timeout aprova. Estado pendente preserva o vínculo atual e bloqueia alteração administrativa sensível.
- Recuperar identidade não recupera chaves nem conteúdo E2EE (ADR-060).
- Recuperação offline: **negada**. Exige autoridade atual; `stale`/`unknown` nega (ADR-059).
- Via assistida por suporte: **não autorizada**. Suporte não é adjudicador (ver §4.7).

Classes, quantidade e independência de provas, notificação, contestação, autoridade adjudicadora e prazos permanecem OPEN-01. Nenhum valor presumido.

### 4.3 Organização: criação, convite, aceite, expiração, revogação, saída e remoção

Estados da organização: `draft`, `active`, `suspended`, `closure_requested`, `closing`, `closed`. Estados do convite: `created`, `sent` (apenas com ACK de classe correspondente), `accepted`, `expired`, `revoked`, `rejected`. Estados da associação: `invited`, `active`, `suspended`, `removed`, `left`.

| Etapa | Pré-condição | Ação e resultado | Invariante |
|---|---|---|---|
| Criação | Principal `active`, vínculo validado, aceite de política | Cria `tenant_id`, associa o criador como `owner` e registra a transição `draft → active` | Organização nasce com exatamente um `owner`; criação é idempotente por chave de operação |
| Convite | Papel `owner`/`admin`, tenant derivado no servidor, política de convite | Emite convite com escopo, papel proposto, emissor, audiência, `single_use` e validade | Convite **não** cria membership nem concede acesso; convite não é prova de titularidade |
| Aceite | Principal autenticado igual à audiência, convite `sent` e não expirado/revogado | Cria associação `active` com o papel proposto; consome o convite | Aceite duplicado retorna o mesmo resultado (idempotente); não cria duas associações |
| Expiração | Validade do convite vencida | Convite passa a `expired`; aceite posterior é negado com erro explícito | Expiração não altera associações existentes; valor de validade OPEN-02 |
| Revogação | `owner`/`admin` revoga convite não aceito | Convite passa a `revoked`; tentativa posterior é negada | Revogação não exige consentimento do convidado; não remove associação já existente (essa é outra operação) |
| Saída | Associação `active` do próprio principal | Associação passa a `left`; grants e escopos derivados deixam de valer para efeitos futuros | Último `owner` não pode sair sem transferir titularidade ou encerrar a organização |
| Remoção | `owner`/`admin` com escopo sobre o alvo | Associação passa a `removed`; sessões e grants do alvo ficam sujeitos à revogação da SPEC-002 §7 | Remoção não apaga conteúdo produzido; não é exclusão de dados pessoais |
| Transferência de titularidade | `owner` atual + confirmação própria | Papel `owner` passa ao novo principal com epoch de autoridade crescente (SPEC-002 §5.4) | Resultado `unknown` bloqueia mutações dependentes nos dois lados; nunca inferir sucesso por timeout |
| Encerramento | `owner`, sem hold ativo bloqueante | `active → closure_requested → closing → closed` conforme política de retenção | Ver §6.3 e §6.4; sem política, falha fechado |

Limites: um principal pode pertencer a múltiplas organizações; a seleção de contexto é do cliente, a validação é do servidor. Não existe convite com papel `owner` sem decisão explícita e sem confirmação adicional (OPEN-04). Não existe convite de `guest` para outro tenant que não o emissor. Não existe transferência automática de titularidade por inatividade.

### 4.4 Catálogo de papéis e matriz de permissão por operação

Papéis propostos (catálogo fechado nesta versão, não customizável):

- **owner** — titularidade da organização: governança final, transferência, encerramento. Não contorna consentimento, retenção nem direitos do titular (SPEC-012 §§2, 4).
- **admin** — administra pessoas, convites e configuração; não cria nem remove `owner`.
- **operator** — operação corrente: dispositivos, canais e resposta a incidentes dentro de política delegada; sem governança de pessoas.
- **auditor** — leitura de auditoria e evidência; não opera, não altera pessoas, não vê localização precisa por padrão.
- **member** — uso normal dos próprios recursos e dos recursos compartilhados com ele.
- **guest** — escopo mínimo, por tempo e por recurso convidado; sem inventário, sem auditoria, sem governança.

Legenda: **sim** = autorizado com pré-condições normais; **não** = negado; **cond.** = permitido apenas sob a condição declarada na coluna de limite; **próprio** = apenas sobre o próprio principal/recurso.

| Operação | owner | admin | operator | auditor | member | guest | Condição / limite |
|---|---|---|---|---|---|---|---|
| `org.read_profile` | sim | sim | sim | sim | sim | cond. | Guest vê apenas nome/aviso do recurso convidado |
| `org.update_profile` | sim | sim | não | não | não | não | Alteração registrada em auditoria |
| `org.invite.create` | sim | sim | não | não | não | não | Papel proposto limitado (OPEN-04); validade OPEN-02 |
| `org.invite.revoke` | sim | sim | não | não | não | não | Somente convite não aceito |
| `org.invite.accept` | — | — | — | — | — | — | Ação do convidado autenticado, não de papel interno |
| `membership.role.change` | sim | cond. | não | não | não | não | Admin apenas sobre `member`/`guest`/`operator`/`auditor`; nunca `owner`/`admin` |
| `ownership.transfer` | sim | não | não | não | não | não | Confirmação própria + epoch; OPEN-03 |
| `membership.remove` | sim | cond. | não | não | não | não | Admin apenas `member`/`guest`; último `owner` protegido |
| `membership.leave` | cond. | sim | sim | sim | sim | sim | Owner apenas se houver outro owner ativo |
| `device.enroll` | sim | sim | cond. | não | próprio | não | Operator limitado à política delegada; member apenas dispositivo próprio |
| `device.revoke` | sim | sim | cond. | não | próprio | não | Revogação de dispositivo alheio exige papel acima |
| `asset.read` | sim | sim | sim | cond. | próprio | não | Auditor vê metadados, não posição precisa |
| `grant.create` / `grant.revoke` (ACL, geofence, canal) | sim | sim | cond. | não | não | não | Operator apenas dentro da política delegada e do escopo do tenant |
| `location.precise.view` | cond. | cond. | cond. | não | próprio | não | Exige finalidade, grant vigente e auditoria (ADR-041); auditor não recebe por padrão |
| `audit.read` | sim | cond. | não | sim | não | não | Admin apenas escopo próprio e sem segredo/prova |
| `audit.export` | cond. | não | não | cond. | não | não | Bloqueado sem política de exportação/retenção (U2, G1-09) |
| `retention.configure` | cond. | não | não | não | não | não | Só com política por classe/jurisdição aprovada (ADR-062); sem política, negado |
| `support.contest.open` | sim | sim | sim | sim | sim | cond. | Guest somente sobre o próprio caso |
| `support.contest.decide` | não | não | não | não | não | não | Autoridade independente, fora do tenant (OPEN-05) |
| `org.close` | sim | não | não | não | não | não | Bloqueado por hold ativo; fail-closed (§6.4) |
| `account.close` (própria conta) | próprio | próprio | próprio | próprio | próprio | próprio | Fora da matriz de tenant; sujeita a §6.3 |

Regras invariantes: (a) papel não substitui grant, consentimento ou finalidade; (b) `owner` não vê conteúdo E2EE nem posição precisa sem grant próprio; (c) elevação de papel nunca é automática por tempo, inatividade, convite aceito ou migração de owner; (d) ausência de papel explícito é negação, não herança; (e) toda mudança de papel ou remoção é auditada com principal, tenant, papel anterior/novo, autoridade e motivo sanitizado.

### 4.5 Relação com os grants da SPEC-002

Papel de organização e grant são coisas distintas e ambas necessárias:

1. Papel responde "quem pode administrar este tenant e o quê"; grant responde "qual operação sobre qual recurso, com qual finalidade e audiência" (SPEC-002 §§3.2, 6.1–6.2).
2. Todo acesso a recurso continua exigindo decisão do serviço que protege o recurso, na ordem de SPEC-002 §4.2. Ser `owner` não encurta a lista.
3. Mudança ou remoção de associação **não** edita grants existentes: dispara revogação declarada por destino/cópia, com estado por destino (SPEC-002 §7, ADR-062). Recibo de recebimento não é prova de bloqueio nem de descarte.
4. Role change não reescreve grant histórico; grants novos usam o papel vigente. Revalidar antes de efeito é obrigatório.
5. Nenhuma operação de organização é autorizada por grant offline (ADR-059). `stale`, `unknown`, relógio incerto ou política indefinida negam convite, aceite, mudança de papel, remoção, transferência e recuperação.
6. Papel é sempre do par (principal, tenant). Não existe papel global; `auditor` de um tenant não lê outro.
7. Autoridade de tenant/recurso (SPEC-002 §4) permanece a única fonte de membership. Nenhum campo de corpo, path, query ou cabeçalho do cliente cria papel.

### 4.6 Dados e contratos (semânticos)

Campos semânticos propostos, não wire: `tenant_id`; `principal_ref`; `membership_id`; `role`; `role_assigned_by`; `assigned_at`; `state`; `invite_id`; `invite_audience`; `invite_scope`; `invite_proposed_role`; `invite_issued_by`; `invite_expires_at` (valor OPEN); `invite_consumed_at`; `idempotency_key`; `policy_ref`; `retention_policy_ref`; `ack_class`. Referência a pessoa usa identificador opaco; nome civil, e-mail, telefone e GMC não são chave.

Persistência de qualquer um desses campos exige política ADR-062 identificada por `(finalidade, classe, cópia, jurisdição, prazo, gatilho de descarte)`. Sem política, a escrita **falha fechada** e a UI mostra a operação indisponível com motivo.

### 4.7 Contestação e suporte

Escopo: contestar recuperação de identidade, remoção ou mudança de papel indevida, titularidade de organização, convite recebido sem solicitação, e cobrança de ação administrativa. Não cobre emergência (SPEC-006), SOS (SPEC-012/006) nem disputa comercial (U4).

Estados propostos do caso: `opened`, `triaged`, `evidence_pending`, `under_review`, `decided`, `closed`. Regras: (a) abrir contestação **não** transfere autoridade nem suspende automaticamente efeito legítimo; (b) contestação sobre recuperação mantém o vínculo atual e bloqueia a transferência (SPEC-002 §§5.3, 8); (c) suporte **não** é adjudicador de identidade — não reseta conta, não aprova prova e não cria papel; (d) toda decisão registra autoridade, escopo, evidência e motivo sanitizado; (e) contestação não revela a terceiro a existência de conta, vínculo ou tenant (anti-enumeração); (f) prazos de triagem e resposta são **desconhecidos** e dependem de decisão de produto (U1/U7) — nenhum SLA é inventado aqui.

### 4.8 Erros (rótulos lógicos)

`invite_expired`, `invite_revoked`, `invite_consumed`, `invite_wrong_audience`, `invite_not_found` (indistinguível de "sem autorização"), `membership_exists`, `membership_not_found`, `role_not_permitted`, `last_owner_violation`, `tenant_context_invalid`, `insufficient_scope`, `authority_unavailable`, `stale_epoch`, `recovery_proof_missing`, `recovery_pending`, `recovery_contested`, `retention_policy_missing`, `closure_blocked_hold`, `outcome_unknown`. São rótulos documentais; wire, envelope e status são de SPEC-003. Erro nunca enumera identidade, convite, tenant ou papel de terceiro.

## 5. Timeout, cancelamento, repetição limitada, idempotência, offline, revogação e recuperação

| Tema | Regra proposta | Pendência |
|---|---|---|
| Timeout | Toda operação de organização tem prazo de espera e resultado explícito `outcome_unknown` quando a confirmação não chega; nunca sucesso por decurso de tempo (ADR-061) | Valores numéricos: OPEN-08 |
| Cancelamento | Criação de convite, envio de notificação e encerramento em curso podem ser cancelados antes do efeito; após efeito, a compensação é uma nova operação auditada, não rollback silencioso | Semântica de cancelamento por operação: SPEC-003 |
| Repetição limitada | Retry apenas com limite e backoff declarados; convite reenviado não duplica convite válido nem gera segunda associação | Limites: OPEN-08 |
| Idempotência | `idempotency_key` obrigatória em criação de organização, convite, aceite, mudança de papel, remoção e encerramento; mesma chave + mesmo conteúdo = mesmo resultado; conteúdo diferente = inválido | Armazenamento da chave: SPEC-004 |
| Offline | Nenhuma mutação de conta/organização offline. Leitura pode usar cache **marcado como desatualizado**, sem alegar atualidade. Aceite de convite offline é negado | Nenhum grant offline previsto (ADR-059) |
| Revogação | Remoção, saída, suspensão e encerramento revogam escopo por destino; revogação conhecida prevalece sobre cache/lease. Estado por cópia distingue bloqueio, descarte confirmado, pendência e cópia fora de controle | Confirmação por cópia: SPEC-002 §7 |
| Recuperação | Depois de partição ou restore, revalidar autoridade, epoch, membership e retenção **antes** de reabrir acesso; replay reconstrói projeção e não reemite convite, e-mail, push ou qualquer efeito externo | Fronteira: ADR-061, SPEC-021 |
| ACK de convite/notificação | `transport_accepted` (adaptador de e-mail/push aceitou) ≠ `persisted_local` (servidor registrou) ≠ `delivered_to_device` (destino confirmou) ≠ `acknowledged_human` (pessoa agiu). Aceite de convite é transição de estado com evidência própria, não uma classe de ACK. É proibido exibir "convite entregue/lido" sem evidência do destino ou da pessoa | Sem infraestrutura de e-mail/push, `delivered_to_device` fica indisponível e deve ser exibido como tal |

## 6. Segurança, isolamento, consentimento, retenção, compatibilidade e migração

### 6.1 Segurança e isolamento

Tenant derivado no servidor; validação de papel e grant antes do efeito; negação por padrão; anti-enumeração em todas as respostas; auditoria com referências opacas, sem token, prova, segredo, coordenada ou conteúdo. Operações administrativas exigem MFA conforme direção existente em `04-seguranca/seguranca.md`; fatores e recuperação de segundo fator dependem de SPEC-002. Acesso de suporte a dados de tenant exige autoridade, escopo, finalidade e registro; nunca é acesso comum nem "acesso de emergência" permanente (ADR-062).

### 6.2 Consentimento

Cadastro de prova de recuperação, entrada em organização, participação em canal e compartilhamento de posição são consentimentos distintos, com versão de política registrada. Aceitar convite consente em pertencer ao tenant, não em compartilhar localização, conteúdo ou dados de dispositivo. Administrador não consente pelo titular.

### 6.3 Encerramento de conta e exclusão de dados

Fluxo proposto: pedido → confirmação própria → `closure_requested` → verificação de hold → `closing` → `closed`. Regras:

- Encerrar conta remove acesso e associações ativas; **não** apaga automaticamente classes sujeitas a retenção legal ou contratual.
- A exclusão de cada classe exige política ADR-062 aprovada. Sem política, a operação de exclusão dessa classe **falha fechado** com `retention_policy_missing`, e a UI informa o que permanece e por quê.
- Cópia já entregue a terceiro (bridge, exportação, notificação por e-mail/push) não é remotamente recolhível; o destino é declarado por cópia, nunca "global".
- Conta encerrada não pode ser reaberta por convite; nova conta é novo principal.
- Encerrar conta não cancela obrigações de auditoria em curso e não revoga por si efeitos já consumados.

### 6.4 Encerramento de organização

`owner` solicita; encerramento exige confirmação própria, verificação de hold e política de retenção por classe. Resultado: associações terminam, convites pendentes são revogados, grants deixam de valer para efeitos futuros, dados entram em `closing`. Estado `closed` só é declarado quando a política de cada classe foi aplicada ou explicitamente bloqueou a operação. Hold exige autoridade, escopo e revisão, e nunca reativa acesso comum. Encerramento é idempotente e auditado.

### 6.5 Compatibilidade e migração

Contratos de organização seguem N/N-1 por perfil com rejeição antes do efeito e sem downgrade silencioso (ADR-056); cliente incompatível não cria, aceita nem altera nada. Migração de `tenant_id`, `membership_id` ou papel exige epoch crescente, inventário de efeitores e reconciliação de resultado desconhecido (SPEC-002 §5.4). Campos desconhecidos são preservados; campos removidos entram em `reserved`. Rollback é pré-condição de release, não promessa retroativa.

## 7. Critérios de aceite observáveis e casos planejados positivos, negativos, falhas e recuperação

Todos os casos abaixo são **NOT_EXECUTED**. Cada linha descreve resultado observável planejado, com caso positivo, negativo, borda e falha/recuperação. IDs `P22-Cnn` são desta SPEC; identificadores formais de teste `NOT_ASSIGNED`.

| Caso | Positivo | Negativo | Borda | Falha e recuperação |
|---|---|---|---|---|
| P22-C01 — primeiro acesso (RF-001) | Vínculo `(iss, sub)` validado ativa a conta e permite escolher contexto entre vínculos permitidos | GMC, e-mail ou nome no corpo da requisição não ativa conta nem cria vínculo | Conta com dois vínculos não usa tenant padrão ambíguo | Autoridade indisponível mantém `pending_first_access` explícito; nova tentativa idempotente |
| P22-C02 — cadastro de prova (RF-001) | Prova cadastrada com titularidade atual verificável habilita recuperação futura | Pedido de recuperação não cadastra o fator que aprovará o mesmo pedido | Conta sem prova opera, mas exibe recuperação indisponível | Falha ao persistir prova não deixa estado parcial; operação pode ser repetida com mesma chave |
| P22-C03 — recuperação (RF-001) | Prova previamente cadastrada e válida transfere acesso com `policy_id/version` da classe de auditoria e motivo registrados na trilha | Prova não cadastrada, expirada, reutilizada, comprometida ou de titular errado não transfere | Resultado `unknown` mantém vínculo atual e bloqueia mutações sensíveis | Timeout não aprova; estado fica `recovery_pending` e reconsulta é idempotente |
| P22-C04 — criação de organização (RF-020) | Principal válido cria `tenant_id` com exatamente um `owner` | Principal sem vínculo válido não cria organização | Repetição com mesma `idempotency_key` retorna a mesma organização | Falha entre criação e auditoria não deixa organização sem owner; operação é reconciliada |
| P22-C05 — convite (RF-020, ADR-058) | Convite emitido com escopo, papel, audiência e validade; `ack_class` fiel ao estágio real | Convite não concede acesso antes do aceite; corpo da requisição não define papel | Convite expirado é negado com erro explícito e sem criar associação | Entrega não confirmada aparece como classe menor, nunca como entregue/lido |
| P22-C06 — aceite (RF-020) | Principal igual à audiência aceita e recebe exatamente o papel proposto | Outro principal ou convite revogado/expirado é negado | Aceite repetido devolve o mesmo resultado sem duplicar associação | Aceite offline é negado; reconexão revalida convite antes de consumir |
| P22-C07 — expiração e revogação (RF-020) | Convite vencido/revogado não pode ser aceito | Revogação não remove associação já existente (operação distinta) | Revogação de convite já consumido é negada e não altera membership | Replay de evento não reemite convite nem e-mail (ADR-061) |
| P22-C08 — saída e remoção (RF-020) | Saída e remoção encerram escopo para efeitos futuros e são auditadas | Último `owner` não pode sair nem ser removido sem transferência | Remoção de associação inexistente é idempotente e não revela existência | Revogação não confirmada no destino aparece como pendente, não como concluída |
| P22-C09 — matriz de papéis (RNF-003) | Cada operação da matriz é permitida/negada conforme papel e condição | `auditor` não vê posição precisa; `operator` não governa pessoas; `guest` não lê inventário | Consulta cross-tenant é negada sem revelar se o tenant existe | Papel desconhecido ou estado `stale` nega a operação e não herda permissão |
| P22-C10 — papel × grant (RNF-003, SPEC-002) | `owner` com grant vigente acessa o recurso; sem grant, é negado | Ser `owner` não substitui consentimento, finalidade nem grant | Grant expirado bloqueia mesmo com papel alto | Revogação conhecida prevalece sobre cache; revalidação antes do efeito |
| P22-C11 — encerramento de conta (RNF-015, ADR-062) | Encerramento remove acesso e aplica política por classe | Classe sem política não é apagada; retorna `retention_policy_missing` | Hold ativo impede `closed` e mantém estado explícito | Falha no meio do `closing` é retomável e idempotente; nunca declara descarte sem prova |
| P22-C12 — encerramento de organização (RF-020, ADR-062) | `owner` encerra com confirmação, hold verificado e convites revogados | `admin` não encerra; encerramento não reativa acesso em hold | Organização já encerrada responde de forma idempotente | Perda de confirmação resulta `outcome_unknown`; nova consulta reconcilia antes de repetir |
| P22-C13 — contestação (G1-06) | Contestação abre com estado, autoridade e escopo registrados | Contestar não transfere identidade nem cria papel; suporte não reseta conta | Contestação sem evidência passa a `evidence_pending`, não a rejeição automática | Autoridade indisponível mantém o caso aberto; nenhum prazo converte silêncio em decisão |
| P22-C14 — anti-enumeração e auditoria (RNF-003) | Auditoria registra principal, tenant, papel, decisão e motivo sanitizado | Consulta sem papel/finalidade é negada sem revelar conta, convite ou tenant | Respostas de convite inexistente e de convite sem autorização são indistinguíveis | Registro sem segredo, prova, coordenada ou conteúdo; retenção da auditoria conforme política |
| P22-C15 — plataformas | Fluxos aplicáveis funcionam em servidor Linux, macOS arm64/x86_64, Windows, Linux desktop, iOS, Android e web onde a função existir | Cliente sem capacidade de armazenamento protegido não habilita cadastro de prova nem papel administrativo | Comportamento após suspensão/resume e troca de rede é registrado por SO/versão | Falha de API do SO resulta estado explícito, não degradação silenciosa para autenticação fraca |

Aceite observável exige, para cada caso aprovado no futuro: ambiente, versão de cliente/SO, pré-condição, estímulo, resultado esperado, resultado observado e evidência. Sem isso, o caso permanece planejado.

## 8. Pendências com responsável e condição de fechamento; evidências de revisão e aprovação

| ID | Pendência | Responsável proposto | Condição de fechamento |
|---|---|---|---|
| OPEN-01 | Classes, quantidade, independência e inscrição das provas de recuperação; notificação, contestação e autoridade adjudicadora | Usuário (decisão) + Kim Cameron + Bruce Schneier | Política aprovada com classes, inscrição, compromisso, notificação, disputa e critérios de negativa; casos P22-C02/C03 atualizados |
| OPEN-02 | Validade de convite, reenvio, número de reenvios e janela de expiração | Usuário + Margaret Hamilton + Kim Cameron | Valores aprovados com justificativa de produto; nenhum valor presumido |
| OPEN-03 | Mínimo de `owner` ativo, saída do último owner, transferência por inatividade e recuperação de organização órfã | Margaret Hamilton + Leslie Lamport | Regra aprovada e caso de borda P22-C08 ampliado |
| OPEN-04 | Papéis convidáveis por `owner` × `admin`, convite de `owner` e aprovação dupla | Margaret Hamilton + Kim Cameron | Matriz de convite aprovada; matriz de §4.4 revisada |
| OPEN-05 | Autoridade adjudicadora de contestação, prazos de triagem/resposta e SLA mínimo (G1-06) | Usuário + Lawrence Lessig (jurídico) + CTO | Autoridade, prazos e evidência aprovados; U1 decidido. Nenhum SLA é inventado antes disso |
| OPEN-06 | Papéis personalizados por tenant e limites de delegação | Kim Cameron + Graydon Hoare | Decisão registrada; se aprovado, contrato e matriz versionados |
| OPEN-07 | Prova de domínio corporativo para associação automática e prevenção de conta órfã (G2-09/G2-10) | Kim Cameron + Gregor Hohpe | Contrato aprovado junto de SPEC-002/003 e integração SCIM |
| OPEN-08 | Timeouts, retry, backoff e limites de reenvio por operação | Graydon Hoare + Brendan Gregg | Valores aprovados após medição em ambiente autorizado; hoje `NOT_EXECUTED` |
| OPEN-09 | Política de retenção por classe para conta, convite, membership, auditoria e contestação | Usuário (U2) + Bruce Schneier + Lawrence Lessig | Tabela `(finalidade, classe, cópia, jurisdição, prazo, gatilho)` aprovada; sem ela, encerramento permanece fail-closed |
| OPEN-10 | Portabilidade/exportação de dados de organização (G1-09) | Martin Kleppmann + P3 | Depende de OPEN-09; contrato próprio, fora desta SPEC |
| OPEN-11 | Wire, envelope de erro, `ack_class` por resposta e mapeamento HTTP | Graydon Hoare | Contrato em SPEC-003 conciliado com ADR-058; nenhuma duplicação de enum |
| OPEN-12 | Comportamento por plataforma: WebAuthn/passkey, armazenamento protegido e lifecycle por SO | Donny Wals + Chet Haase + Lea Verou + Alex Crichton | Matriz SO/versão documentada por fonte oficial e depois validada |
| OPEN-13 | MFA para papéis administrativos: fatores, provedor e recuperação | Kim Cameron + Bruce Schneier | Política aprovada; caso P22-C09 ampliado |
| OPEN-14 | Revisão independente desta versão | Ada Lovelace | Parecer identificado sobre o hash desta DRAFT-0.1, com achados tratados um a um |
| OPEN-15 | Aprovação formal | CTO + usuário | DoD de `EQUIPE.md` §7 cumprido, vetos aplicáveis levantados com critério de fechamento, registro de versão/hash/revisor/data |

### Evidências de revisão e aprovação

- Revisão independente de qualquer versão: **inexistente** para esta DRAFT-0.1.
- Parecer, aprovação, hash registrado: **não há**.
- Vetos em vigor que alcançam esta SPEC: C1-1 (cobertura não é conciliação), C3-1 (aceite e casos), C4-1 (fronteiras de autorização/abuso). Nenhum foi levantado.
- Conciliação no catálogo `SPECS.md` (RF-001/RF-020 como "A conciliar"): **pendente**, dono W1/Ada.

## 9. Mapa das seções obrigatórias

| Item de `SPECS.md` §Conteúdo obrigatório | Seção desta SPEC |
|---|---|
| 1. Identificador, versão, estado, autor, revisor e fontes com localizadores | cabeçalho + §1 |
| 2. Objetivo, requisitos cobertos, escopo, plataformas e exclusões justificadas | §2 (2.1–2.5) |
| 3. Decisões rastreáveis, alternativas, consequências, divergências e dependências | §3 (3.1–3.5) |
| 4. Estados e transições, contratos, dados, erros, permissões e limites | §4 (4.1–4.8) |
| 5. Timeout, cancelamento, repetição limitada, idempotência, offline, revogação e recuperação | §5 |
| 6. Segurança, isolamento, consentimento, retenção, compatibilidade e migração | §6 (6.1–6.5) |
| 7. Critérios de aceite observáveis e casos planejados positivos, negativos, falhas e recuperação | §7 |
| 8. Pendências com responsável e condição de fechamento; evidências de revisão e aprovação | §8 |

## 10. Estado e limitações

- **Especificado nesta versão:** estados e transições propostos de conta, recuperação, organização, convite e associação; matriz de papéis por operação; relação papel × grant; ACK por classe aplicado a convite/notificação; encerramento fail-closed; contestação; erros lógicos; casos planejados; pendências com dono.
- **Herdado, não redefinido:** identidade tipada, credencial, grant, estados de validação e erros de autorização (SPEC-002); taxonomia de ACK (ADR-058); retenção fail-closed (ADR-062); posição/cursor (ADR-057).
- **OPEN:** OPEN-01…OPEN-15.
- **Não demonstrado:** nenhum fluxo, endpoint, schema, política, tela, integração de e-mail/push ou adjudicação existe. Nenhum número, prazo, preço, limite ou SLA foi fixado.
- **`NOT_EXECUTED`:** casos P22-C01…P22-C15 não executados; nenhum código, build, teste, medição, ambiente ou revisão independente realizado. Identificadores de teste `NOT_ASSIGNED`.
- **Gate:** implementação só depois de planejamento encerrado e **todas** as SPECs aprovadas (`pendencias.md` §Autorização posterior; `SPECS.md` §Estados e liberação). Esta SPEC não altera esse gate e não aprova nada.
## Casos por decisão (ADR-056…066)

Casos planejados desta versão, complementares a §7 e sem substituí-la. Nenhum foi executado; identificador formal de teste `NOT_ASSIGNED`. Rótulos de erro citados são os de §4.8 (`invite_expired`, `stale_epoch`, `retention_policy_missing`, `closure_blocked_hold`, `outcome_unknown`, `authority_unavailable`); `ack_class` é o conjunto fechado da ADR-058. Parâmetro OPEN (validade de convite, janela N/N-1, prazos de retenção, wire do `ack_class`) resulta em **negar ou declarar `unknown`**, nunca em valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S022-056-P | ADR-056 compatibilidade | Cliente e servidor no par declarado, sem janela de contrato aprovada | Criar organização e aceitar convite | `compatibility=param_not_configured` registrado na auditoria; `membership` e `role` gravados conforme a resposta; nenhuma compatibilidade declarada suportada nem negada por valor presumido | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-056-N | ADR-056 compatibilidade | Cliente sem feature crítica comum (ex.: campo obrigatório de associação ausente) | Tentar criar organização ou aceitar convite | Rejeição antes do efeito, sem `membership` parcial; erro explícito; nenhum downgrade silencioso aceito | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-056-B | ADR-056 compatibilidade | Perfil de contrato de organização sem janela publicada (OPEN) | Declarar o cliente como compatível | Compatibilidade `unknown`; mutações de organização negadas com motivo; leitura mantida sem alegar suporte | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-056-F | ADR-056 compatibilidade | Falha de release em curso com cliente antigo ainda conectado | Romper compatibilidade e tentar continuar operando | Cliente antigo rejeitado antes do efeito; operação administrativa indisponível, não parcial; rollback exige compatibilidade e estado de dados, sem promessa retroativa | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-057-P | ADR-057 posição/cursor | Listagem de membros ou auditoria com cursor versionado vinculado a principal, tenant, filtros, perfil e retenção | Solicitar página seguinte | Página aceita; posição canônica `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` na evidência; nenhum membro omitido ou repetido | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-057-N | ADR-057 posição/cursor | Cursor sem `partition` ou com `authority_epoch` incomparável | Apresentar o cursor à listagem | Cursor rejeitado com erro explícito; nunca traduzido; nenhuma página sintetizada nem lista vazia apresentada como resultado | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-057-B | ADR-057 posição/cursor | Cursor de epoch superado após migração de `tenant_id` | Consultar auditoria paginada | Resposta `stale_epoch` explícita; nenhuma reutilização de `seq`; auditoria permanece consultável a partir de posição válida | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-057-F | ADR-057 posição/cursor | Restore de projeção de auditoria com cursores emitidos antes | Retomar listagem com cursor antigo e `authority_epoch` divergente | Falha fechada; cursor recusado; estado `unknown` na consulta; nenhuma posição ou registro inventado | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-058-P | ADR-058 ACK por classe | Convite emitido e adaptador de e-mail/push com recibo autenticado | Consultar estado do convite | Resposta com `ack_class=transport_accepted` e evidência do adaptador; `delivered_to_device` ausente enquanto não houver recibo do destino | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-058-N | ADR-058 ACK por classe | Sem infraestrutura de e-mail/push, ou sem recibo do destino | Abrir lista de convites na UI | UI exibe classe menor ou `delivered_to_device` indisponível; proibido "convite entregue/lido"; nenhuma promoção por tempo | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-058-B | ADR-058 ACK por classe | Resposta de API de convite sem campo `ack_class` (OPEN-11) | Consumir a resposta | Resposta tratada como inválida/erro de contrato; `ack_class` não é inferido por status HTTP nem por ausência de erro | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-058-F | ADR-058 ACK por classe | Convite persistido com notificação entregue e confirmação perdida | Reenviar e consultar estado | Resultado `outcome_unknown` para a notificação; reenvio idempotente não duplica convite válido; nenhuma classe elevada | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-059-P | ADR-059 grant offline | Grant vigente e `epoch` comparável, sem limite de frescor aprovado | Consultar membership em cache e executar operação enumerada fora do escopo de organização | Leitura em cache marcada como desatualizada com `freshness_limit=param_not_configured`; `authorization=unknown` na dimensão frescor; nenhuma mutação de organização autorizada offline e nenhuma atualidade alegada | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-059-N | ADR-059 grant offline | Cliente offline com grant aparentemente válido | Aceitar convite, mudar papel ou remover associação | Operação negada; aceite offline recusado explicitamente; `role` e `membership` inalterados | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-059-N2 | ADR-059 grant offline | Grant `stale`, `unknown`, relógio incerto ou política indefinida | Solicitar convite, transferência de titularidade ou recuperação | Negação com motivo classificado para cada uma das operações; nenhuma autoridade criada por cache, convite aceito ou migração de owner | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-059-B | ADR-059 grant offline | Janela de frescor não aprovada e membership obtida de cache antigo | Abrir painel administrativo offline | Estado por dimensão: `membership=cached`, `freshness=unknown`/`param_not_configured`, `grant=valid`; nenhuma ação administrativa habilitada e nenhum prazo padrão aplicado | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-059-F | ADR-059 grant offline | Snapshot restaurado contendo grant ainda na validade; revogação de membership conhecida depois | Reabrir operação administrativa com base no snapshot | Grant negado e revogação prevalece sobre cache/lease; estado por cópia registra bloqueio, pendência ou cópia fora de controle | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-060-P | ADR-060 confidencialidade | Resposta administrativa com perna terminadora declarada (e-mail, push, exportação) | Consultar dados de organização na API e na UI | `confidentiality=terminated` presente e fiel, com a perna identificada; nenhuma alegação de E2EE sobre dados administrativos | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-060-N | ADR-060 confidencialidade | `owner` ou `admin` com papel alto e sem grant próprio | Tentar ler conteúdo E2EE de canal ou posição precisa de membro | Negação: papel não substitui grant nem remove E2EE; conteúdo não é decifrado nem apresentado; resposta não alega E2EE sobre notificação por e-mail/push | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-060-B | ADR-060 confidencialidade | Cópia de dado administrativo sem metadado de confidencialidade (D9 aberta) | Exportar relatório de membros | Cópia tratada como `confidentiality=terminated`; ausência não é suprida por cache ou versão anterior; UI exibe a terminação | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-060-F | ADR-060 confidencialidade | Exportação/auditoria gera derivado de conteúdo antes protegido | Consultar o derivado e tentar alegar E2EE | Derivado herda `terminated`; API rejeita a alegação; destino declarado por cópia e não recolhível | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-061-P | ADR-061 transação/recuperação | Criação de organização com `outbox` na mesma transação da membership | Criar organização e observar publicação | `tenant_id` e registro de auditoria commitados juntos; publicação após commit; `ack_class` fiel ao commit comprovado | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-061-N | ADR-061 transação/recuperação | Evento de convite reprocessado por replay | Repetir a publicação do mesmo evento | Nenhum convite novo, nenhum e-mail/push reemitido, nenhuma segunda associação; efeito externo irreversível não é repetido | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-061-F | ADR-061 transação/recuperação | Falha entre criação da organização e gravação da auditoria | Retomar a operação com a mesma `idempotency_key` | Reconciliação conclui sem organização sem `owner`; nenhuma auditoria ausente publicamente declarada como completa; resultado permanece `outcome_unknown` até prova | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-061-F2 | ADR-061 transação/recuperação | Restore com Inbox em voo e autoridade `stale`/`unknown` | Reabrir acesso administrativo | Mutação forte bloqueada com `authority_unavailable`/`stale_epoch`; replay reconstrói projeção e não reemite convite, e-mail ou push; revalidação de autoridade, epoch, membership e retenção antes de reabrir | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-061-B | ADR-061 transação/recuperação | Organização criada com `outbox` commitado e publicação pendente | Consultar estado da criação antes da publicação | `tenant_id` e auditoria legíveis com prova de commit; `ack_class` máximo coerente com o commit; materialização e efeito externo ausentes; leitor não publica nem promove classe | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-062-P | ADR-062 retenção fail-closed | Classe de conta/convite/membership com política aprovada em `(finalidade, classe, cópia, jurisdição)` e prazo/gatilho não decididos | Consultar e depois aplicar o gatilho de descarte | Registro grava `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo ou gatilho aplicado como aprovado; estado de descarte por cópia permanece `unknown` na UI e na auditoria | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-062-N | ADR-062 retenção fail-closed | Classe sem política aprovada (OPEN-09) | Encerrar conta ou organização e excluir a classe | Operação falha fechada com `retention_policy_missing`; dado permanece; UI informa o que fica e por quê; nenhum descarte declarado | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-062-B | ADR-062 retenção fail-closed | Hold ativo sobre dados de uma classe | Encerrar conta ou organização | `closure_blocked_hold` explícito; estado `closed` não é declarado; hold exige autoridade, escopo e revisão e não reativa acesso comum | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-062-F | ADR-062 retenção fail-closed | Falha no meio de `closing` com cópias parciais | Retomar o encerramento | Retomada idempotente; nenhuma declaração de descarte sem prova; revogação e estado por cópia (bloqueio, descarte confirmado, pendência, cópia fora de controle) declarados por destino, nunca global | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-063-P | ADR-063 supply chain/anti-rollback | Release do console/API de organização com SBOM, build reproduzível, assinatura e canal autenticado; `generation` ≥ piso | Instalar e repetir o build | Ingresso aceito; digest do build repetido igual; SBOM e `generation` na evidência de release | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-063-N | ADR-063 supply chain/anti-rollback | Artefato adulterado ou release sem assinatura verificável | Distribuir e instalar | Rejeição antes do efeito; release declarado não distribuível; causa sanitizada em auditoria, sem segredo | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-063-N2 | ADR-063 supply chain/anti-rollback | Piso de versão assinado superior ao artefato ofertado | Instalar artefato com `generation` inferior | Rejeição registrada pelo cliente; nenhuma substituição; downgrade para release vulnerável proibido | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-063-B | ADR-063 supply chain/anti-rollback | Piso de `generation` não publicado para o console/API e releases N/N-1 assinadas | Instalar a release mais recente e consultar anti-rollback | Piso exposto como `param_not_configured`; ingresso não declara `generation` aceita contra piso desconhecido; nenhum piso presumido a partir da versão instalada | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-063-F | ADR-063 supply chain/anti-rollback | Chave de assinatura comprometida ou lista de revogação `unknown` | Atualizar e, depois, executar rollback autorizado | Atualização bloqueada com revogação `unknown`; revogação/reemissão verificada no cliente; rollback aceito só como exceção autorizada, limitada e auditada com piso rebaixado registrado | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-064-N | ADR-064 SOS offline | Membro em aparelho sem F1/F2 e sem verificação online; organização com onboarding ativo | Abrir o app do membro antes de qualquer incidente e tentar acionar SOS offline | UI declara “modo offline indisponível neste aparelho” **antes** do acionamento, oferece o discador do sistema como caminho e **não promete persistência**; nenhum texto de contrato, convite ou onboarding promete SOS offline nesse aparelho | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-064-P | ADR-064 SOS offline | Aparelho com F3 disponível (verificação online) | Acionar SOS offline com verificação online | Verificação executada antes do efeito e registrada; incidente aberto com `ack_class` máximo `persisted_local`; aviso de indisponibilidade ausente porque o caminho é sustentado | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-065-P | ADR-065 retenção no encerramento | Conta/organização encerrada; classes com prazo na tabela da ADR-065 (mensagem, metadado, auditoria, backup, conta encerrada com carência) | Executar `closure_requested → closing → closed` | Cada classe segue o prazo da ADR-065 por classe e por cópia; expiração verificada e registrada com `policy_id/version`, classe, cópia e instante; UI declara o que fica, por quanto tempo e com qual política | P | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-065-B | ADR-065 retenção no encerramento | Tenant configurou redução de prazo permitida para uma classe | Encerrar a conta/organização daquele tenant | Redução aplicada só ao escopo do tenant; prazo padrão preservado para os demais; tentativa de ampliar prazo sem nova decisão registrada é recusada e informada | B | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-065-N | ADR-065 retenção no encerramento | Classe/cópia sem entrada na tabela da ADR-065 (nem por padrão, nem por política do tenant) | Encerrar e tentar excluir essa classe | Falha fechada com `retention_policy_missing`; dado permanece, UI informa a lacuna e a cópia fica bloqueada para uso comum até existir política própria | N | NOT_EXECUTED · ID formal NOT_ASSIGNED |
| S022-065-F | ADR-065 retenção no encerramento | Hold legal com autoridade, escopo e revisão durante `closing` | Atingir o prazo da classe durante o hold | `closure_blocked_hold`/`DATA_UNDER_LEGAL_HOLD` explícito; descarte suspenso só no escopo, revisão registrada e acesso comum negado; encerrado o hold, o prazo volta a correr e o descarte é verificado | F | NOT_EXECUTED · ID formal NOT_ASSIGNED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §1, respeitando o limite de cobertura declarado (parcial/interface). Células citam casos da tabela `Casos por decisão` ou ID novo descrito na Nota; prefixo `S022-R###` agrupa os quatro tipos. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-001 (parcial) | S022-R001-P | S022-R001-N | S022-R001-B | S022-057-F | P: ciclo de conta (primeiro acesso, recuperação) e criação de `tenant_id` na fronteira de produto. N: identidade tipada/credencial não é redefinida aqui; operação sem vínculo válido é recusada. B: conta sem organização associada não cria `tenant_id` implícito. F: recuperação interrompida não declara conta recuperada. Semântica e autorização pertencem à SPEC-002. |
| RF-020 (parcial) | S022-061-P | S022-056-N | S022-056-B | S022-061-F | P: jornada Teams de organização, convite, papéis e encerramento com auditoria. N: operação de organização incompatível é rejeitada antes do efeito. B: contrato sem janela aprovada → `param_not_configured`. F: falha entre criação e auditoria não declara conclusão. Capacidade/oferta pertencem a SPEC-019. |
| RNF-003 | S022-R003-P | S022-060-N | S022-059-B | S022-061-F2 | P: `tenant_id` derivado no servidor e papel avaliado por operação. N: papel alto não substitui grant nem atravessa tenant. B: membership em cache antigo não habilita mutação. F: restore com autoridade `stale` bloqueia mutação forte. Isolamento em repouso/RLS não são cobertos. |
| RNF-015 (interface) | S022-062-P | S022-062-N | S022-062-B | S022-062-F | P: matriz papel/tenant e retenção declarada para informação administrativa. N: sem política, encerramento falha fechado. B: hold ativo bloqueia `closed`. F: descarte declarado por cópia, nunca global. Objeto "AP e visitantes" pertence a SPEC-009/020. |
| ADR-058 (convites) | S022-058-P | S022-058-N | S022-058-B | S022-058-F | Classe de ACK exibida como recebida; ausência de `ack_class` é erro de contrato, não sucesso. |
| ADR-060 (dados administrativos) | S022-060-P | S022-060-N | S022-060-B | S022-060-F | Perna terminadora declarada também em e-mail/push/exportação; nenhuma alegação de E2EE sobre dado administrativo. |
| ADR-064 (aviso de offline) | S022-064-P | S022-064-N | n/a — motivo: a borda pertence à superfície do cliente (SPEC-013) e ao contrato de grant (SPEC-002/006) | S022-059-F | O produto se compromete apenas com o que o aparelho pode demonstrar: sem F1/F2/F3 a UI avisa antes do acionamento, oferece o discador do sistema e não promete persistência. |
| ADR-065 (retenção no encerramento) | S022-065-P | S022-065-N | S022-065-B | S022-065-F | Prazos por classe e por cópia da tabela da ADR-065 aplicados ao `closing`; redução por tenant permitida, ampliação exige nova decisão; hold suspende descarte com revisão. |
