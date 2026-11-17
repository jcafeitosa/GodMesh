# Cobertura de requisitos — mapa requisito → SPEC → seção → aceite → estado

**Worker:** W1 — Integração e matriz. **Data:** 2026-11-17. **Atualização W1 (pós-SPECs novas):** 2026-11-17. **Base:** 60 requisitos (43 RF + 17 RNF) de `docs/00-produto/requisitos.md`; 23 arquivos de SPEC em `docs/08-specs/` (SPEC-001–SPEC-021, SPEC-023 DRAFT-0.1, SPEC-024 DRAFT-0.1); `SPEC-022-produto-organizacoes.md` **ausente** (não existe no repositório); `docs/06-planejamento/matriz-rastreabilidade.md`; ADR-056 em `docs/06-planejamento/decisoes.md`.

> `SPECS.md` cataloga as 24 SPECs (001–024) com hash por arquivo; os 60 vínculos continuam "A conciliar" porque a aprovação depende de revisão independente por hash. As versões DRAFT acima saem dos cabeçalhos das próprias SPECs, não do catálogo.

## Método

1. Extraí de cada SPEC a tabela de rastreabilidade/requisitos cobertos e as seções de critérios de aceite e casos planejados; cada linha abaixo cita uma seção real da SPEC (verificada por leitura do arquivo) e um aceite citado verbatim (abreviado quando indicado por `…`).
2. Estados possíveis e única definição usada nesta matriz:
   - **completa** — existe uma SPEC canônica com contrato da semântica integral do requisito e aceite direto mapeado, sem autodeclaração de cobertura parcial para aquele requisito.
   - **parcial** — a SPEC cobre subescopo, plataforma específica, ou ela mesma declara cobertura parcial/OPENs materiais sobre a semântica do requisito.
   - **ausente** — nenhuma SPEC existente contém texto de contrato ou aceite para o requisito; indica-se o documento mais próximo.
3. Estado documental: **nenhuma linha desta matriz aprova SPEC, requisito ou teste.** Todas as 24 SPECs existem e estão em DRAFT/OPEN (SPEC-022 presente; `SPECS.md` cataloga 022–024); casos planejados permanecem `NOT_ASSIGNED`/`NOT_EXECUTED`. "Completa" ≠ aprovada ≠ implementada ≠ validada. Ter SPEC canônica própria não significa "completa": SPEC-023 e SPEC-024 são DRAFT-0.1 com revisão independente pendente e OPENs materiais sobre a semântica dos requisitos, o que por si só as enquadra em **parcial** pela definição do item 2.

## O que NÃO foi verificado

- Não co-execitei testes nem validei qualquer comportamento de produto (não existe código no repositório).
- Não conferi os arquivos de contribuição em `Shared/GodMesh/...` citados nas SPECs (caminhos locais externos, indisponíveis nesta verificação).
- Não validei a verdade material das citações-fonte internas das SPECs (ex.: hashes de insumos, seções de `01-arquitetura/*` citadas); a citação verifica a presença do texto na SPEC, não sua correção.
- Não renumerei nem alterei requisitos; divergências semânticas entre SPECs permanecem como estão.
- O campo "aceite" é uma citação curta do critério da SPEC, não o registro completo do caso de teste.
- Esta atualização é autoral do mesmo W1 que redigiu a matriz anterior; **não é revisão independente** e não levanta veto nem confere parecer de terceiro.
- Os vetos em vigor permanecem aplicáveis às linhas novas: C1-1 — "Matriz de cobertura não serve como conciliação final" (critério de fechamento: "Conciliação na tabela canônica de `SPECS.md`, dono por caso, aceite não condicional") e C3-1 — "As 21 SPECs não são testáveis como estão" (critério de fechamento: "Aceite observável, casos positivo/negativo/borda/falha por requisito, parâmetros aprovados ou declarados desconhecidos"), ambos de `docs/09-equipe/EQUIPE.md` §11. SPEC-023 e SPEC-024 entram no mesmo alcance: seus casos estão `NOT_EXECUTED` e os parâmetros que precisariam ser observados (frescor, janelas, histerese, token, cifra, retenção, limites antiabuso) continuam OPEN nos §9/§7 de cada uma.

## Requisitos funcionais (RF)

| Requisito | SPEC § seção | Aceite | Estado |
|---|---|---|---|
| RF-001 | SPEC-002 §3.1, §4.1; aceite §12.1 | aceite: "Dado `(iss, sub)` autenticado e vinculado, o recurso resolve o GMI correspondente e permite somente escopo/tenant/ACL/finalidade vigentes" | parcial |
| RF-002 | SPEC-005 §4; aceite §8.1 | aceite: "leitura retorna uma identidade de A, ambos os devices e cada endpoint sob seu pai, sem alegar conectividade" | parcial (rebaixado por veto C1: cardinalidade e identidade de endpoint OPEN em SPEC-005 §10) |
| RF-003 | SPEC-023 §1 (rastreabilidade), §3; aceite §8 C003; SPEC-001 §4.1 (interface compartilhada) | aceite §8 C003: "Evidência atribuível e fresca sustenta nível contextual" / "Declaração isolada não promove verificação nem autoriza" / "Frescor ausente resulta desconhecido operacional"; §3: a projeção `operational ⊆ verified ⊆ supported` "permanece proposta, dependente de conciliação contratual" | parcial |
| RF-004 | SPEC-001 §4.1; aceite §6 C03/C04 | aceite: "capability stale, grant negado/unknown, ausência de caminho ou dependência falha → sem rota e motivo seguro apropriado"; custo só comparado com dados comparáveis (C04) | parcial |
| RF-005 | SPEC-007 §6.1–6.5, §9 (crosswalk) | aceite: "PTT entre clientes autorizados estabelece voz GodMesh sem bridge; desligar Mumble/FRN não interrompe fluxo nativo" — prioridade/floor em OPEN-007-B | parcial |
| RF-006 | SPEC-003 §"Contrato GMP e negociação — RF-006"; aceite 003-A/B | aceite: "Sem major comum ou feature crítica desconhecida → Rejeitar sem efeito e sem downgrade silencioso" — cláusula "sem criptografia própria" sem aceite explícito nesta SPEC | parcial |
| RF-007 | SPEC-010 §1, §7 C010-14; aceite §11 | aceite: "Bridge forma caminho A→B→A, ou serviço externo cai → Bloqueia retorno; serviço GodMesh nativo segue independente; registra degradação"; própria SPEC: "Parcial: contrato comum de bridge, sem homologação Mumble" | parcial |
| RF-008 | SPEC-010 §1, §7 C010-11/C010-12 | aceite: "Anuência do operador existe, mas aplicabilidade/licença regulatória do país/uso é unknown → TX permanece bloqueado" | parcial |
| RF-009 | SPEC-011 §1, §3–5; aceite §9.3 | aceite: "Para geofence com incerteza cruzando o limite… retorna `indeterminate`; não emite entrada/saída categórica"; §1: "Parcial; regra de geofence e retenção OPEN" | parcial |
| RF-010 | SPEC-010 §1, §7 C010-03/05/07; SPEC-017 §4, §9 | aceite: "Origem apresenta IMEI/nickname/DevEUI sem prova de binding → Preserva ID externo e sua evidência; não cria `gmi_id`/GMC" | parcial |
| RF-011 | SPEC-004 §10 (crosswalk); aceite S004-02/04/12 | aceite: "registrar evidência por etapa/destino; materialização e reconhecimento não se inferem do ACK de transporte"; §1: "esta SPEC não cobre a jornada integral" | parcial |
| RF-012 | SPEC-019 §"Rastreabilidade e escopo"; aceite S019-03 | aceite: "Região de menor RTT sem residência ou capacidade elegível → Não selecioná-la; só escolher outro destino elegível, senão informar indisponibilidade"; SPEC-018 §1: "alta disponibilidade não demonstrada" | parcial |
| RF-013 | SPEC-004 §10; aceite S004-05/08–11 | aceite: "sincronizar somente escopo autorizado; preservar prefixos separados; lacuna, prova ou autoridade ausente bloqueia só operação dependente e a retomada revalida escopo" | parcial |
| RF-014 | SPEC-001 §4.3; aceite §6 C09 | aceite: "IPv6 indisponível com IPv4 autorizado… → fallback de perfil sem perda de autenticação; se nenhuma rota segura, indisponibilidade explícita" | parcial |
| RF-015 | SPEC-002 §5.2; aceite §12.8 | aceite: "Descoberta e handshake sem convite/certificado autorizado mantêm nó isolado; não publicam readiness nem recebem snapshot privado"; §2: "não fecha protocolo de ingresso" | parcial |
| RF-016 | SPEC-002 §2; SPEC-019 aceite S019-01/02 | aceite: "Assinatura inválida ou convite negado → Rejeitar antes de ingresso/cópia; registrar causa sanitizada"; SPEC-002 §2: "Parcial, interface de autorização para cada cópia/snapshot/replay" | parcial |
| RF-017 | SPEC-010 §1, §7 C010-01 | aceite: "Descoberta encontra endpoint sem autorização de conexão → Retorna candidato, origem e estado; não abre sessão nem anuncia rota/TX" | parcial |
| RF-018 | SPEC-010 §1, §7 C010-02 | aceite: "Adaptador recebe payload inválido ou versão desconhecida → Rejeita antes da normalização; resposta não expõe segredo nem altera estado canônico"; §1: "IPC/schema não escolhidos" | parcial |
| RF-019 | SPEC-003 §"API, erros e operação idempotente — RF-019"; aceite 003-D | aceite: "Mesmo pedido/chave reapresentado após perda de resposta → Retornar mesma operação conhecida; sem segundo efeito"; texto: "RF-019 não está integralmente coberto" | parcial |
| RF-020 | SPEC-022 §1–§7 (SPEC canônica nova, DRAFT-0.1, revisão de Ada pendente); recortes: SPEC-012 §7.1, SPEC-019 S019-09/10 | aceite: capacidades por oferta com papéis e convites; OPENs de G1-01/G1-02/G1-03 declarados na SPEC | parcial |
| RF-021 | SPEC-020 §2; SPEC-010 §7 C010-11 | aceite: "Bridge está conectada para RX; usuário solicita TX → TX segue negado até obter grants/floor e cada autorização externa/regulatória aplicável" | parcial |
| RF-022 | SPEC-020 §2, §5 | aceite: "Só alegar E2EE quando o desenho e gestão de chaves limitarem leitura de conteúdo aos endpoints participantes; servidor ou bridge que termine proteção… não é invisível ao conteúdo" — algoritmos e gestão de chaves OPEN | parcial |
| RF-023 | SPEC-001 §2, §4.1; aceite §6 C06 | aceite: "bridge autorizada desliga durante serviço nativo independente → rota externa degrada; nativo permanece elegível se suas dependências estão íntegras" | parcial (rebaixado por veto C1: C06 cobre um cenário de bridge, não a independência de quaisquer servidores externos) |
| RF-024 | SPEC-001 §4.3; aceite §6 C10; ADR-056 (decisão normativa) | aceite: "peer sem versão/feature crítica comum, canary falho ou rollback inseguro → rejeitar antes do efeito, pausar progressão". A decisão N/N-1 já foi tomada: ADR-056, Status "Aprovada tecnicamente pelo CTO em 17/11/2026; janela numérica de suporte depende de decisão de produto", torna N/N-1 obrigatória "dentro de uma janela de suporte declarada por perfil versionado" e rejeita "quando viável" como qualificador normativo. O que resta OPEN é o tamanho da janela: "Pendente do usuário — Tamanho da janela (ex.: 6, 12 ou 24 meses) e quais perfis entram no v1". A redação de SPEC-001 §4.3 é anterior à ADR-056 e ainda precisa ser conciliada com ela (não tratada aqui: edição de SPEC é proibida nesta matriz) | parcial |
| RF-025 | SPEC-008 §5, §6; aceite §9 | aceite: "Timeline de voz independe do ASR; resultado parcial/final vincula sessão, speaker/origem e estado"; cabeçalho: "Cobertura destes requisitos é parcial enquanto as decisões abaixo permanecerem abertas" | parcial |
| RF-026 | SPEC-006 §"Estados e fronteiras de evidência"; aceite §"Aceites e casos" | aceite: "Pedido autorizado sem rede e com persistência local comprovada → Exibir registro local e ausência de confirmação externa, sem alegar entrega"; §Objetivo: cobertura integral "permanece parcial" | parcial |
| RF-027 | SPEC-012 §3; aceite §7.2 | aceite: "contexto manual entra em conflito com sugestão de sensor: manual prevalece; sugestão permanece identificada; nenhuma permissão RF ou SOS é gerada"; §1: "Taxonomia final, conflitos e expiração OPEN" | parcial |
| RF-028 | SPEC-012 §4; aceite §7.5 | aceite: "prazo decorrido sem check-in: aviso de ausência de confirmação… não há incidente SOS automático"; §10: "RF-028 cobre parcialmente sessão, convites, atraso, estados terminais e Beacon" | parcial |
| RF-029 | SPEC-011 §3; aceite §9.1; SPEC-015/016 §recortes | aceite: "a resposta preserva fonte(s), instante medido/recebido, qualidade, incerteza e estado; não produz direção/ponto a partir de RSSI"; §1: "Parcial; fusão, suporte e critérios por plataforma OPEN" | parcial |
| RF-030 | SPEC-024 §1–§5 (SPEC canônica proposta), aceite §6; recortes: SPEC-015 §5.1/§8 (Android), SPEC-020 §2, SPEC-012 §4 | aceite §6: "Token válido é anunciado, relato protegido chega somente à audiência autorizada e exibe proximidade com incerteza" / "Operação correspondente é negada e identidade não é revelada" / "Início é aceito sob demais condições; fim é rejeitado"; §1: "A associação canônica no catálogo e a revisão independente permanecem pendentes"; §7 lista nove OPENs (token e rotação, cifra, sybil/correlação/relay, retenção, offline, pausa Guardian, matriz Android/iOS) | parcial |
| RF-031 | SPEC-023 §1, §4 (tabela por classe), aceite §8 C031; recortes: SPEC-015 §5.2, SPEC-016 §2 (aceite §10.6), SPEC-018 §6 | aceite §8 C031: "API identifica classe e teste autorizado distingue alcance" / "SSID ou perda celular não afirma satélite" / "API indisponível resulta desconhecido; nova observação atualiza estado"; §4: "Critérios, histerese e janelas são OPEN"; §2: matriz de APIs, versões e operadoras é dependência de validação | parcial |
| RF-032 | SPEC-023 §1, §5 (estados e erros), aceite §8 C032; recorte: SPEC-016 §2 (aceite §10.7) | aceite §8 C032: "Ajustes suportados abrem e confirmação humana é identificada" / "Abrir ajustes não afirma ativação" / "API ausente ou permissão negada apresenta instrução e estado não verificado"; §5: "O estado externo permanece `unverified` sem observação oficial suficiente. Abrir ajustes não comprova ativação" | parcial |
| RF-033 | SPEC-023 §1, §6 (prioridade, orçamento, recuperação), aceite §8 C033; recortes: SPEC-015 §5.2, SPEC-016 §2 (aceite §10.8), SPEC-001 §2 (OPEN-05) | aceite §8 C033: "Restrição prioriza SOS, check-in e mensagem na ordem proposta" / "ACK local não vira entrega ou atendimento" / "Falha de armazenamento de SOS é visível; reconexão reavalia e retoma envio autorizado sem duplicar efeitos"; §6: "Nenhum valor de prazo, taxa, tamanho ou energia foi escolhido" | parcial |
| RF-034 | SPEC-013 §12.1–12.3; SPEC-014 §3 | aceite: "cada área alcançável e mostra, de forma separada, conectividade, disponibilidade local, autorização, estado de sync, frescor e capacidade daquela função"; SPEC-013 §1: RF-034 "Não reivindicado como aceite próprio" | parcial |
| RF-035 | SPEC-009 §6.1; aceite §8.1, §9 P-WIFI-01 | aceite: "Descoberta sem permissão ou com dado obsoleto não cria registro público nem inventa estado"; §11: plataformas e parâmetros permanecem parciais/OPEN | parcial |
| RF-036 | SPEC-009 §6.2; aceite §8.2/8.3 | aceite: "SSID/BSSID/conexão/senha conhecida sem prova administrativa não permite AutoShare" | parcial |
| RF-037 | SPEC-009 §6.3; aceite §8.4 | aceite: "Pedido do app e aprovação do SO não afirmam associação; associação não afirma autenticação; autenticação não afirma Internet" | parcial |
| RF-038 | SPEC-009 §6.4; SPEC-011 §7 | aceite: "Registrar coordenada somente quando disponível e sem inventar precisão; preservar origem, qualidade, instante, responsável pelo cadastro/publicação quando aplicável e histórico de alterações" | parcial |
| RF-039 | SPEC-009 §6.4; aceite §8.6 | aceite: "Publicação exata sem opt-in específico e autoridade vigente é negada. Mapa público não retorna BSSID bruto, credencial, GMI, visitante, sessão ou inventário privado" | parcial |
| RF-040 | SPEC-009 §6.5; aceite §8.10 | aceite: "Consulta administrativa é escopada por papel/tenant e gera trilha; leitura cross-tenant sem compartilhamento explícito é negada" | parcial |
| RF-041 | SPEC-009 §6.6; aceite §8.7/7.8; SPEC-003 §"Wi-Fi — interface RF-041" | aceite: "Operação repetida não duplica efeito; chave reutilizada com corpo diferente ou versão obsoleta conflita"; requisito ESPECIFICAR na origem; §11: "RF-041 continua a especificar" | parcial |
| RF-042 | SPEC-013 §5; aceite §8 (LP-lock-screen, LP-concurrency); SPEC-016 §9 | aceite: "nenhuma coordenada precisa/segredo/ID civil aparece; detalhe solicita auth/ACL"; SPEC-013 §1: RF-042 "Coberto parcialmente" | parcial |
| RF-043 | SPEC-014 §3, §10.7; SPEC-015 §8 | aceite: "Restrição de bateria/rede com SOS e trabalhos discricionários → Política de prioridade aplicada sem cortar emergência do SO nem inventar orçamento energético"; SPEC-014: metas de energia/latência "não foram aprovadas aqui" | parcial |

## Requisitos não funcionais (RNF)

| Requisito | SPEC § seção | Aceite | Estado |
|---|---|---|---|
| RNF-001 | SPEC-001 §4.3; aceite §6 C09 | aceite: "IPv6 indisponível com IPv4 autorizado, ou UDP falha com TCP elegível → fallback de perfil sem perda de autenticação"; §2: "Vint e contratos de transporte detalham perfis; não presume capacidade universal" | parcial |
| RNF-002 | SPEC-002 §7; aceite §12.6 | aceite: "Nova chave passa somente após manifesto e vínculo válidos; chave antiga não ganha validade adicional por rotação"; §2: "sem aceite integral nem prazo numérico" | parcial |
| RNF-003 | SPEC-002 §4.1–4.2; aceite §12.3; SPEC-020 §2 | aceite: "Principal do tenant A pede recurso de B via body/path/query… nega o efeito e não revela externamente se B ou o recurso existe"; §2: "Parcial… Retenção e auditoria completas não são cobertas aqui" | parcial |
| RNF-004 | SPEC-004 §10; aceite S004-01–11 | aceite: "identidade/posição separadas, dedupe no namespace definido, sequência contígua, conflito/quarentena, snapshot/replay e tombstone sem salto"; limite: "Chave final… continuam OPEN" | parcial |
| RNF-005 | SPEC-018 §3; aceite §8 (Fonte stale, Partição); SPEC-019 S019-04/13 | aceite: "tornar prova requerida vencida ou contraditória → `unknown`, bloqueio dependente, função independente autorizada preservada"; §1: "alta disponibilidade não demonstrada" | parcial |
| RNF-006 | SPEC-018 §4; aceite §8 (Saturação) | aceite: "carga efetiva, fila, decisão por classe, reserva e recuperação conforme política; sem limite aprovado não há PASS quantitativo" | parcial |
| RNF-007 | SPEC-007 §6.4 (crosswalk §9) | aceite: "Inspeção do bus comprova ausência de payload de áudio em todas as formas exercitadas; falha do bus não persiste nem migra áudio"; OPEN-007-D/E: definição de bus e metadados em aberto | parcial |
| RNF-008 | SPEC-001 §4.3; aceite §6 C10 | aceite: "peer sem versão/feature crítica comum, canary falho ou rollback inseguro → rejeitar antes do efeito, pausar progressão; isolar se recuperação segura não for demonstrada"; "Não declarar N/N-1 universal" | parcial |
| RNF-009 | SPEC-004 §10; aceite S004-02–04/12–15 | aceite: "conservar referências de evento, posição, operação, tentativa, cópia e prova, sob acesso/retention autorizados"; limite: "Esquema wire, assinatura e códigos C07" abertos | parcial |
| RNF-010 | SPEC-018 §5; aceite §8 (Restore positivo/negativo); SPEC-021 §3, §8 | aceite: "evidência física, lógica, autoridade, replay, cobertura e decisão de admissão; comparar RPO/RTO somente após decisão"; SPEC-021 §1: RNF são "candidatos… não cobertura integral confirmada"; SPEC-018 §1: "política RF e compliance não cobertos integralmente" | parcial |
| RNF-011 | SPEC-008 §5.3, §9 | aceite: "ASR indisponível → PTT e timeline continuam; motivo indisponível; nenhum texto inventado"; cabeçalho: cobertura parcial enquanto decisões abertas | parcial |
| RNF-012 | SPEC-006 §"Aceites e casos"; SPEC-004 §10 | aceite: "Edge confirma etapa local; falta prova de domínios → Exibir somente camada comprovada, sem ACK protegido" | parcial |
| RNF-013 | SPEC-009 §6.2; aceite §9 P-WIFI-11 | aceite: "Auditoria e telemetria recebem evento de segurança → Sanitiza antes do buffer/exportação e registra cobertura/fonte"; §11: "RNF-013/014 mantêm dependências quantitativas/ADR" | parcial |
| RNF-014 | SPEC-009 §6.3; aceite §8.5 | aceite: "Ausência de controlador ou observação de Internet produz `unknown`/pendente, salvo evidência específica que sustente outro estado"; método/frescor OPEN | parcial |
| RNF-015 | SPEC-020 §2; aceite §9.5; SPEC-009 §8.10 | aceite: "Acesso a registros de visitantes é limitado por papel, AP, tenant e finalidade; terceiro sem escopo não pode inferir existência"; §2: "papéis, campos e prazo ainda OPEN"; SPEC-002 §2: "Esta SPEC não reivindica cobertura do requisito" | parcial |
| RNF-016 | SPEC-013 §5 (tabela por `kind`); aceite §8 LP-SOS-normal | aceite: "`active` não prova entrega; ACK técnico não prova `claimed`; `claimed` não prova resolução"; §1: "Coberto parcialmente… falta ligação por rótulo/fase a eventos de domínio definidos" | parcial |
| RNF-017 | SPEC-004 §10; aceite S004-10–13; SPEC-014 §3 | aceite: "após partição/revogação, não reabrir replay ou acesso protegido sem autorização atual, prova de autoridade e retenção aplicável; desconhecido permanece desconhecido"; SPEC-014: "Não reivindico cobertura integral do teste sob partição, nem disponibilidade de AP" | parcial |

## Contagem por estado

| Estado | RF | RNF | Total |
|---|---|---|---|
| completa | 0 | 0 | 0 |
| parcial | 43 | 17 | 60 |
| ausente | 0 | 0 | 0 |
| **Total** | **43** | **17** | **60** |

Contagem inalterada pela atualização: as cinco linhas reescritas (RF-003, RF-030, RF-031, RF-032, RF-033) ganharam SPEC canônica própria (SPEC-023, SPEC-024), mas continuam **parcial** pela definição desta matriz, porque ambas são DRAFT-0.1 com revisão independente pendente e OPENs materiais sobre a semântica coberta, e porque SPEC-022 — a única que poderia mudar a situação de RF-020 — **está ausente**. Nenhum requisito passou a "completa" e nenhum era "ausente" textual.

## Requisitos sem SPEC canônica própria

Os cinco requisitos que antes não tinham SPEC canônica dedicada passaram a ter, na atualização de 2026-11-17:

- **RF-003, RF-031, RF-032, RF-033** — SPEC-023 `capacidades-conectividade.md` (DRAFT-0.1, revisão independente designada a Vint Cerf, **pendente**), com rastreabilidade própria em §1 e aceites C003/C031/C032/C033 em §8.
- **RF-030** — SPEC-024 `crowdfind-beacon.md` (DRAFT-0.1, revisão independente designada a Bruce Schneier, **pendente**), que se declara "SPEC canônica proposta para RF-030 e ADR-043/044".

Requisito que segue sem SPEC canônica própria:

- **RF-020** — coberto por `SPEC-022-produto-organizacoes.md` (DRAFT-0.2) em §1–§7; o que falta é revisão independente, não contrato: é citada como entrega em `docs/09-equipe/registro-gaps.md` (G1-01 "Escrever jornada + SPEC-022", G1-02 "SPEC-022 §organizações") e no item "Três SPECs novas", mas o arquivo não existe em `docs/08-specs/` e não há § nem aceite a citar. Cobertura real continua sendo recorte de SPEC-012 §1/§3 e SPEC-019 S019-09/10; a jornada de cadastro/organizações (Personal, Teams, Node) segue sem contrato.

Nenhum requisito está textualmente ausente das 23 SPECs existentes. O risco que o C1 apontava como "cobertura alegada" se reduz, mas não desaparece: as SPECs novas estão em DRAFT-0.1 sem catálogo em `SPECS.md` e sem aceite observável fechado (veto C3-1).

## Divergências registradas (sem julgamento)

- `SPECS.md` mantém os 60 vínculos como "A conciliar"; esta matriz não substitui essa conciliação — é evidência de leitura para ela.
- RF-002 e RF-023 foram rebaixados de "completa" para "parcial" por veto do crítico C1 (`thr_693tyueebr`), que conferiu 23 linhas da matriz e considerou citações fiéis, mas insuficientes para cobertura integral; as duas SPECs continuam DRAFT/OPEN com revisões pendentes (SPEC-005 DRAFT-0.3 revisão de Momjian pendente; SPEC-001 DRAFT-0.3 revisão de Erich pendente).
- A matriz histórica cita RF-004/RNF-007 com "complementos explícitos de Erich e Radia" recebidos, não conferidos (`SPECS.md` §Pendências); mantive-os como parcial.

**Não executado:** aprovação de qualquer SPEC, atribuição de IDs de caso, execução de testes. Este documento não declara nada pronto.
