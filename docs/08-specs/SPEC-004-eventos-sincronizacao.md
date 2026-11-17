# SPEC-004 — Eventos e sincronização durável

Versão: DRAFT-0.6 · Estado: DRAFT / OPEN · Data: 2026-11-17  
Autor documental: Martin Kleppmann · Revisor independente designado: Leslie Lamport, revisão desta versão pendente.

Esta SPEC descreve o comportamento planejado e as decisões ainda abertas. Não é aprovação, implementação, evidência operacional ou teste executado. A autoria canônica foi autorizada pela ordem relatada em `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/CONSOLIDACAO-CANONICA.md:1-12`; a aprovação de todas as SPECs e a conclusão do planejamento precedem implementação. Alterações de risco e política dependem de decisão do usuário. Este documento não substitui a autoridade das fontes normativas nem declara encerrado B06 ou B12.

## 1. Objetivo, escopo e rastreabilidade

Garantir que eventos e comandos tenham identidade distinta, recebimento durável rastreável, retomada por prefixo e recuperação explícita de lacunas sem converter um recibo técnico em sucesso de domínio ou reconhecimento humano. Abrange produtor/Outbox, receptor/Inbox, projeção, namespace, cursor, snapshot/replay e interface de autoridade/ACK. Não define implementação de consenso, banco, wire, atendimento SOS ou plataforma.

| Requisito | Subescopo desta SPEC | Fonte e limite |
|---|---|---|
| RF-013 | Federação e sincronização escopada sem origem única permanente. | `docs/00-produto/requisitos.md:19`; autoridade e topologia também pertencem a SPEC-001/Leslie. |
| RNF-004 | Idempotência, partição, duplicata, reordenação, snapshot e tombstone. | `requisitos.md:40`; persistência física é interface de SPEC-005. |
| RF-011 | Evidências de mensagens e SOS por etapa; presença continua efêmera. | `requisitos.md:17`; esta SPEC não cobre a jornada integral. |
| RNF-009 | Rastreabilidade de eventos, comandos, tentativas e confirmações. | `requisitos.md:45`; recibo técnico não é ato humano. |
| RF-026 e RNF-012 | Interface de evidência e recuperação para SOS, sem prometer socorro. | `requisitos.md:33,49`; semântica integral e riscos de atendimento pertencem à SPEC-006/Nancy. |
| RNF-017 | Interface de revogação e retomada sob partição. | `requisitos.md:85`; AP e cliente local dependem de SPEC-009 e SPEC-014. |

A matriz `docs/06-planejamento/matriz-rastreabilidade.md:97,112,116,121,124,146,164-166` associa responsáveis e critérios sem declarar produto validado. Relações a achados GMD só serão adicionadas após conferência do registro original; não são inferidas por proximidade.

### Fontes e versões examinadas

- Contribuição de origem: `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-martin-kleppmann.md`, SHA-256 `4b3e5b47fbd297b78758288c714673b462fd53d841ee77ac3c57e2942a343e23`, lida em 16/11/2026, incluindo §§A–C e resposta MK-1–MK-4. Este hash identifica o insumo, não a presente SPEC.
- `contribuicao-leslie-lamport.md:80-90,106-121,163-176`: revisão independente do insumo e de DRAFT-0.1/0.2; MK-3 e S004-L5/L6 resolvidos textualmente; MK-1/2/4 e S004-L1–L4 parciais. Nenhuma classificação aprova esta nova versão.
- `docs/README.md:8,97-98`; `00-produto/visao.md:3,14-15,27-39`; `01-arquitetura/arquitetura-referencia.md:3,64-77,99-100`; `06-planejamento/decisoes.md:49`; `06-planejamento/pendencias.md:83-94,107-113,123-131`. Arquitetura/ADRs prevalecem segundo suas qualificações; direções 3A e proteção SOS são relatadas por Ada, sem acesso direto à resposta original.
- `01-arquitetura/replicacao.md:17-43,51-55`; `02-dominio/eventos.md:28,39-47,58-59`; `07-contratos/api-inicial.md:3-5,54-65`; `07-contratos/eventos-sincronizacao.md:49-105,130-155,173-193`; `02-dominio/modelo-dados.md:112-156`; `01-arquitetura/autoridade-escrita.md:3,7,34-48,67-76,90-92`; `02-dominio/servico-sos.md:20-35,55-57`; `04-seguranca/retencao-revogacao.md:3-9,23-25,53-56`. Os contratos detalhados e SQL são propostas/ilustrações, não prova de banco, API ou produto existente.
- `Shared/GodMesh/Planejamento-Integral-2026-11-16/DECISAO-PROPOSTA-SOS-ACK.md:3-13`, `DECISOES-ADICIONAIS-E-ENCAMINHAMENTO.md:3,7,13,20` e `SPECS-EM-ANDAMENTO/PROPOSTA-IDENTIDADE-TRANSFERENCIA-ALAN.md:3-9`: propostas e direção relatada, sujeitas à crítica e integração.
- Revisões focais B06/B12: `contribuicoes/revisao-proposta-b06-michael.md` (M-01–M-05), `revisao-proposta-b06-leslie.md` (N1–N4), `revisao-proposta-b06-graydon.md`, `revisao-b06-integrado-edsger.md`, `revisao-dedupe-b06-edsger.md`, `revisao-namespace-michael.md`, `revisao-namespace-leslie.md` e `revisao-delta-namespace-edsger.md`. São pareceres documentais de recortes/versões indicados nos próprios arquivos, não aprovação da SPEC nem prova de enforcement.
- Interfaces em elaboração conferidas: `SPEC-001-arquitetura-federacao.md` DRAFT-0.3, `SPEC-002-identidade-autorizacao.md` DRAFT-0.3, `SPEC-003-gmp-api.md` DRAFT-0.2, `SPEC-005-dados-persistencia.md` DRAFT-0.3 e `SPEC-006-sos-assistencia.md` DRAFT-0.2. Revisões e OPENs de terceiros são dependências, não contratos ratificados.

## 2. Produtos, plataformas e atores

Node e serviços de Teams/Platform que produzem ou recebem eventos duráveis estão diretamente no escopo; Personal e SDK aplicam o contrato quando participam do fluxo. A necessidade de cada produto e os pontos de retenção ainda exigem revisão de produto.

| Plataforma | Aplicabilidade planejada e dependência |
|---|---|
| Servidor Linux | Produção, Inbox, replay e interfaces de autoridade nos serviços que os assumirem. Topologia e storage não escolhidos. |
| macOS arm64 e x86_64, SwiftUI | Consumo/retomada e estado local quando a jornada usar sync; armazenamento/FFI e ciclo de vida em SPEC-014. |
| iOS, SwiftUI | Mesma semântica de consumo aplicável; background e persistência definidos em SPEC-016/014. |
| Windows e Linux desktop | Contrato de cliente quando suportado; toolkit e limites seguem OPEN. |
| Android | Cliente e persistência conforme SPEC-015/014; sem pressupor execução em background. |
| Web | Consumo autorizado quando disponível; persistência e limites do navegador não são presumidos. |

Nenhuma plataforma é dispensada globalmente. Componente sem produção, consumo ou persistência de sync pode ter parte N/A somente mediante justificativa do dono de plataforma e revisão. Um cliente não é presumido full node.

Atores: produtor com principal/tenant autenticados; receptor autorizado; autoridade de escrita para mutações sensíveis; projetor; consumidor que consulta cursor; operador de recuperação autorizado. Kim define identidade/grants; Leslie autoridade/fence; Michael persistência; Graydon GMP/API; Bruce retenção; Alex estado local; Nancy incidente SOS. Esses papéis são responsabilidades documentais propostas, não designação de plantão.

## 3. Modelo lógico e contratos

| Objeto ou referência | Semântica e presença | Regra |
|---|---|---|
| `event_id` | ID lógico do evento; obrigatório no perfil de evento. | Mantido em redelivery; escopo definitivo entre posições OPEN. Se reaparecer em posição distinta sem crosswalk autenticado e autorizado, colocar em quarentena preventiva, não aplicar nem avançar checkpoint dependente enquanto se verifica escopo; isso não declara conflito definitivo nem autoriza crosswalk automático. Não substitui identidade de operação. |
| `operation_id` | Chave de operação mutável idempotente; obrigatória conforme perfil de comando. | Junto ao namespace autenticado e fingerprint versionada identifica a mesma operação; não é `incident_id` ou `event_id`. |
| Posição canônica (ADR-057) | `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. | Partição e `authority_epoch` são componentes obrigatórios; sem conversão de posição ambígua. |
| Digest, schema e prova | Digest de conteúdo canônico versionado, perfil e referência de integridade/autorização. | Bytes, cobertura, algoritmo e wire são OPEN em SPEC-003/002; digest igual não autentica emissor. |
| Inbox e checkpoint | Registro durável validado e maior prefixo contíguo no namespace. | Checkpoint nunca é máximo observado; lacuna impede avanço. |
| Cursor e snapshot | Cursor opaco associado ao vetor autorizado; snapshot com escopo, perfil, integridade e prefixo. | Principal, filtros, autorização e retenção devem ser revalidados; cursor não concede acesso. |
| Tentativa, recibo e claim | Tentativa identifica transporte/destino; recibo comprova etapa; claim é ato humano autorizado. | Nenhum deles se converte automaticamente em outro ou em efeito de domínio. |

Tipos binários, campos wire, códigos e versões concretas não estão aprovados. `IDEMPOTENCY_CONFLICT`, `CURSOR_EXPIRED`, `SEQUENCE_GAP` e demais nomes em C10:189-193 são rótulos candidatos, não API existente. Mensagem de erro não divulga dados de outro tenant nem conteúdo sensível.

### Estados e transições propostas

- Inbox: `received` → `validated` após verificações, ou `rejected`/`quarantined` por prova, escopo, esquema ou conflito. Somente `validated` duravelmente registrado integra prefixo.
- Projeção: `pending` → `applied` com evidência do efeito; falha pode permanecer `failed_retryable` ou `failed_terminal`; `not_required` exige perfil aplicável. Marcar `applied` sem prova é vedado. Detalhes de retry dependem de política.
- Tentativa externa pode ficar `outcome_unknown` depois do envio sem resultado; consultar ou reconciliar a mesma identidade. Não inferir sucesso/falha nem iniciar novo efeito inseguro. Um prazo expirado impede novo efeito, mas não substitui a consulta autorizada de resultado anterior.
- Autoridade e `authority_epoch` são escopados por tenant/partição; `dispatch_epoch` e `dispatch_sequence` pertencem ao dispatcher e não são checkpoint de evento. A máquina de autoridade é frente de Leslie/C06; SPEC-002 cobre identidade e autorização. A atribuição canônica final e a interface entre SPEC-001/002/004 permanecem OPEN, sujeitas ao índice e aos responsáveis.

## 4. Produção, ingestão e confirmações

### Pré-condições e autorização

Validar principal, tenant, origem, escopo, versão e integridade. Para mutação sensível, exigir prova de autoridade/fence aplicável no ponto de efeito. Para cópias persistidas e replay, exigir política eficaz por classe, finalidade e cópia e autorização válida. Dados ausentes não se convertem em permissão; negar ou manter indisponível o fluxo protegido segundo o contrato aplicável, sem expor conteúdo. Antes de deduplicar, a identidade lógica e a posição precisam ser compatíveis no escopo cuja definição está aberta: `event_id` observado em posição diferente, sem crosswalk versionado, autenticado e autorizado, entra em quarentena preventiva; não é aplicado nem avança checkpoint dependente até verificar identidade, escopo e prova. A quarentena não decide que houve colisão maliciosa, não confere autorização para remapear e está sujeita à retenção eficaz C08.

### Produção e Inbox

ADR-061 fixa como padrão mutação de domínio, alocação de posição e Outbox na mesma transação local. Sem commit não confirmar persistência; após commit, retry publica o mesmo evento, posição e digest. Inbox em ledger separado é permitida somente quando recepção e projeção não podem compartilhar transação; nesse caso, ACK máximo `persisted_local`, nunca `materialized`.

No receptor, validar e persistir Inbox e dedupe antes de avançar checkpoint. As respostas carregam `ack_class` obrigatório, uma das classes `transport_accepted`, `persisted_local`, `replicated_to_authority`, `materialized`, `delivered_to_device`, `acknowledged_human`; cada classe tem emissor, evidência e escopo próprios. ACK de Inbox atesta somente prefixo contíguo durável e seu máximo é `persisted_local`; não prova projeção, entrega, sucesso de domínio ou atendimento. Projeções são reconstruíveis e idempotentes. Replay reconstrói projeções e nunca repete efeito externo irreversível (bridge, RF, pagamento ou notificação). Estado de autoridade `stale/unknown` bloqueia mutação forte.

Mesma chave de operação e mesma fingerprint retornam a operação/estado conhecido após revalidar autorização; mesma chave com fingerprint divergente é conflito. Após resposta perdida, consultar com a mesma identidade; reenvio somente dentro das condições de autorização, prazo, retenção e dedupe. Chave antiga desconhecida depois da janela não é automaticamente nova operação. Escopo, janela e canonicalização seguem OPEN, e os nomes de erro são candidatos em C10:69-77.

### ACK por destino e SOS

ACK de sync atesta prefixo Inbox durável para o destino e escopo comprovados. Deve vincular evidência autenticada a identidade de operação/evento, cópia e camada; associação por sessão versus campo wire depende de SPEC-003. Gateway, rede e endpoint têm evidências distintas; confirmação técnica não prova leitura, `claimed` ou atendimento humano. Um agregado por grupo exige conjunto elegível e predicado aprovados.

A proposta de SOS distingue recepção local da aceitação técnica protegida por nó/zona, relatada como direção do usuário; ACK Inbox não comprova, sozinho, independência dos domínios nem recuperação regional. Emitir aceitação protegida somente quando o predicado e a prova da proteção escolhida forem definidos e verificáveis. Sem prova, manter pendente/desconhecido; instalação autogerida não presume zonas. Tratamento de risco regional por modalidade, prova e retenção são OPEN com Nancy/Brendan/Michael e usuário. Não afirmar atendimento, despacho ou risco zero.

## 5. Ordem, replay, handoff e restore

**Namespace canônico (ADR-057):** `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)`. `event_sequence` ordena eventos no namespace; `dispatch_sequence` ordena despacho; `dispatch_epoch` cerca dispatcher; `authority_epoch` cerca autoridade. São tipos distintos, não intercambiáveis. Cursor é opaco e versionado, vinculado a principal, escopo, filtros, perfil e retenção; cursor sem partição ou com epoch incomparável é rejeitado, nunca traduzido. Ilustrações antigas sem partição são históricas e não normativas.

Lacuna 11 com 12 recebido mantém prefixo em 10; outro namespace progride apenas sob suas próprias condições. A contiguidade é requisito do perfil; uma lacuna visível não é prova de que ela foi resolvida. `replicacao.md:38` mantém sequência lógica independente entre streams. Contador físico global, se avaliado, não substitui posição lógica nem vetor de checkpoints; uma única sequência/checkpoint global alteraria a garantia textual e exige decisão normativa.

A prova de atribuição e fence no ponto de efeito é distinta da prova de corte dos eventos. Proposta: inventário autoritativo versionado e verificável de origens, namespaces e efetores, inclusive vazios; corte committed por namespace; atribuição/epoch e fence em cada efetor aplicável; resolução de operações em voo e resultados desconhecidos. Assinatura de uma lista não prova que o universo está completo. A fonte desse inventário, sua completude, o ponto de linearização e o método de provar fence após restore permanecem OPEN.

Sem prova de autoridade/fence, bloquear mutação forte afetada; sem histórico comprovado bloquear operação dependente. A direção 3A, relatada em `pendencias.md:123-128`, preserva fluxos comprovadamente independentes segundo suas políticas. Após fence verificável no efetor, uma nova mutação do antigo escritor é recusada; reconciliação consulta resultado já committed antes do corte ou desconhecido e não autoriza novo commit antigo. Distinguir admissão, commit, corte/fence, chegada e confirmação. Uma chegada tardia de evento historicamente committed é elegível a replay somente com prova de commit dentro do corte, integridade, vínculo e autorização histórica e atual, mais retenção eficaz. Chave expirada hoje não invalida automaticamente prova histórica; assinatura isolada não prova commit.

Snapshot íntegro e escopado pode recuperar estado quando autorizado e disponível após cursor expirado. Não fabrica evento ausente nem prova efeitos externos. Se prova, retenção ou snapshot forem insuficientes, expor recuperação incompleta e não afirmar continuidade. Restore antigo precisa reconciliar autoridade/fence, revogações, cortes e eventos aceitos antes de reabrir operação dependente. Tombstone é evento de domínio, não evidência de exclusão física em todas as cópias.

## 6. Privacidade, retenção e migração

Registros e evidências devem ser minimizados e acessíveis apenas por escopo/finalidade autorizados; logs não incluem segredos ou payload privado desnecessário. Retenção por classe/cópia inclui evento, Inbox, Outbox, dedupe, recibos, provas de commit/fence, snapshots, tombstones, caches e backups. C08 define política eficaz, prazo, descarte e exceções; `null` não significa infinito. A duração das provas deve ser compatível com a janela de recuperação aprovada ou a limitação de replay deve ficar explícita. A expiração de localização não encerra incidente por si.

Eventual migração requer inventário comprovado, não presunção de dados existentes. Preservar envelope histórico e bytes/provas originais; se posição derivada for necessária, usar crosswalk versionado/autenticado com autoridade, escopo e vínculos verificáveis. Crosswalk não revalida assinatura sobre bytes alterados. Envelope derivado com identidade/prova próprias é alternativa sujeita a decisão. Cursor sem tradução demonstrada requer snapshot/rebootstrap autorizado quando possível; caso contrário manter estado incompleto. Rollback só é elegível se preserva ou reconcilia eventos e efeitos posteriores; não desfaz efeito externo irreversível por definição. Resultado remoto desconhecido exige consulta/reconciliação antes de nova emissão.

## 7. Aceites e casos planejados

Os identificadores abaixo identificam cenários documentais desta versão, não testes implementados. Todos: caso formal `NOT_ASSIGNED`, observado `NOT_EXECUTED`. Pré-condições incluem política eficaz e autorização quando pertinentes.

| Caso | Pré-condição e estímulo | Resultado verificável planejado |
|---|---|---|
| S004-01 | Transação produtora aborta antes do commit. | Mutação, posição e Outbox não são confirmadas; não emitir ACK de persistência. |
| S004-02 | Inbox confirma; ACK se perde; evento idêntico é reentregue. | Deduplicar no namespace e retornar prefixo comprovado; não presumir projeção ou efeito externo. |
| S004-03 | Mesma posição recebe ID/digest divergente. | Quarentena/conflito sem sobrescrita nem avanço do checkpoint. |
| S004-15 | Mesmo `event_id` aparece em posições distintas, sem crosswalk verificável nem escopo global do ID ratificado. | Quarentena preventiva; não aplicar nem avançar checkpoint dependente. Só sair após resolver identidade/escopo com prova e autoridade; a classificação final como duplicata ou conflito e eventual crosswalk permanecem OPEN. |
| S004-04 | Inbox separada confirma; projetor falha após efeito e antes de marcador. | Estado pendente/desconhecido; retry apenas com idempotência ou reconciliação demonstrável; sem segundo efeito presumido. |
| S004-05 | Evento 12 chega antes de 11; outro stream tem prefixo completo. | Primeiro checkpoint permanece 10; segundo avança sob seu próprio namespace. |
| S004-06 | P1 e P2 compartilham tenant/stream/epoch/seq. | Perfil de partição única rejeita vínculo inválido ou chave explícita distingue posições; nunca confundir provas. |
| S004-07 | Escritor antigo admitido antes do fence tenta nova mutação após fence verificável. | Efetor recusa nova mutação; resultado anterior desconhecido é consultado sem novo efeito. |
| S004-08 | Manifesto omite efetor ou namespace vazio; corte ainda não comprovado. | Não declarar handoff concluído nem liberar operação dependente; prova de assinatura sozinha é insuficiente. |
| S004-09 | Evento antigo committed antes do corte chega após fence; outro pedido é nova escrita antiga. | Replay somente com prova histórica e autorização atual; nova escrita antiga recusada. |
| S004-10 | Restore antigo carece de fence atual ou revogação; outro fluxo é comprovadamente independente. | Bloquear operação dependente; preservar apenas fluxo independente autorizado segundo sua política. |
| S004-11 | Cursor expirou; snapshot é incompleto ou autorização revogada. | Não avançar checkpoint nem servir dado protegido; mostrar recuperação incompleta. |
| S004-12 | ACK SOS local sem prova de proteção nó/zona; A confirma e B não. | Mostrar camada/destino reais e estado pendente; não afirmar aceitação protegida, claim ou socorro. |
| S004-13 | Chave de assinatura histórica expirou ou foi revogada após o commit. | Aplicar regra histórica aprovada quando existir; sem ela manter prova desconhecida e bloquear replay dependente. |
| S004-14 | Perfil novo aceitou evento; operador solicita rollback sem capacidade de preservá-lo. | Bloquear rollback automático e exigir reconciliação/recuperação autorizada; não reemitir efeito externo desconhecido. |

## 8. Decisões e dependências OPEN

| Item | Alternativas e consequências | Dono proposto e condição de fechamento |
|---|---|---|
| B06 fronteira | Produtor com transação local atual; mudança entre recursos exige nova garantia. Receptor: commit conjunto ou ledger e projetor recuperável. | Michael/Martin/Leslie; decisão do usuário se garantia mudar. Fecho: fronteiras e falhas reconciliadas e revisadas. |
| B06/B12 partição e perfil | Stream versionado por partição ou partição explícita; exclusão dos perfis com/sem epoch e alocação contígua. | Martin/Leslie/Michael/Graydon; fecho: representação injetiva, escopo, corte e compatibilidade documentados. |
| Handoff/restore | Fronteira única comprovada ou protocolo recuperável entre efetores, com possível indisponibilidade. Ledger sozinho não prova fence. | Leslie/Michael/Martin/Graydon; fecho: inventário completo, linearização, provas e casos de falha revisados; risco pelo usuário. |
| Identidade e wire | Escopo de `event_id`, fingerprint, bytes assinados, versão de schema/cursor, erros e negociação. Até decisão, mesmo ID em posição diversa permanece em quarentena preventiva, sem avançar prefixo dependente. | Graydon/Kim/Martin; fecho: matriz e exemplos de conflito, alteração e versão sem downgrade silencioso, com crosswalk apenas se autorizado e verificável. |
| Retenção/prova histórica | Janelas por finalidade e cópia, incluindo validade histórica da chave e retenção de provas. | Bruce/Kim/Michael/Martin; usuário decide risco e política. Fecho: política efetiva e consequências de expiração documentadas. |
| SOS e ACK | Evidência por cópia/destino e proteção nó/zona; risco regional por modalidade e agregação. | Nancy/Brendan/Michael/Martin; usuário decide risco residual. Fecho: predicados e provas por modalidade revisados. |
| Operação e cliente | Backpressure, limites de fila, janela de dedupe, offline, RPO/RTO/SLO, armazenamento/background por plataforma. | Brendan/Alex/donos de plataforma e produto; fecho: parâmetros aprovados ou lacunas explícitas e critérios observáveis. |
| Migração/rollback | Envelope imutável e crosswalk versus derivado com nova prova; compatibilidade e recuperação de efeitos. | Michael/Momjian/Graydon/Kim/Martin; fecho: cortes, provas, rollback e limites revisados. |

Revisão independente de Leslie desta versão e conciliação com SPEC-001/002/003/005/006/014 e contratos canônicos são pendentes. Releitura das fontes e verificação textual não demonstram implementação, API, desempenho, restore ou segurança. Não houve execução de produto, SQL, infraestrutura ou testes. `Shared/TypeSafe/USO.md` foi consultado; Jev não foi executado nesta parcela. Nenhum número de retenção, disponibilidade ou risco foi inventado.


## 9. Resposta às revisões independentes S004-L1–L6

Versão examinada por Leslie: DRAFT-0.1, SHA-256 `53867097e926f8faa6bb81bb97f3fc5054d1e33dda038a73322abbfd3189bc1b`; parecer `Shared/GodMesh/Planejamento-Integral-2026-11-16/SPECS-EM-ANDAMENTO/contribuicao-leslie-lamport.md:163-176`. DRAFT-0.2 registra critérios e mantém lacunas materiais OPEN. Nenhum parecer anterior aprova esta versão.

**S004-L1 — inventário e linearização, OPEN.** O contrato deve identificar fonte autoritativa, geração e escopo do inventário, além de uma barreira para ingresso/saída de efetores durante o corte. Antes de declarar transferência concluída, comprovar fence em cada ponto de efeito e reconciliar operações em voo com resultado confirmado ou `unknown`; após restore, revalidar que nenhum efetor antigo permanece habilitado. Alternativas: fronteira única verificável ou protocolo recuperável entre efetores; ledger isolado não prova completude nem fence. Casos planejados: ingresso concorrente e falha antes, durante e depois do fence; se o conjunto ou resultado for desconhecido, transição dependente incompleta. Leslie/Michael/Martin/Graydon definem mecanismo e prova; decisão de risco/disponibilidade não é presumida.

**S004-L2 — vínculo de partição, OPEN.** Produtor e receptor deverão resolver referência autenticada à fonte autorizada do vínculo tenant/stream/partição e sua versão aplicável no commit. Checkpoint, cursor, snapshot e crosswalk devem preservar essa mesma identidade ou equivalência injetiva demonstrada. Remapeamento requer corte de versão e não reaproveita posição ambígua. Casos planejados: duas partições com mesmo epoch/posição e mudança de vínculo concorrente; sem referência válida, não aceitar efeito dependente nem atribuir posição por inferência. Formato wire e mecanismo permanecem com Leslie/Michael/Graydon/Martin.

**S004-L3 — confiança histórica, OPEN.** A autoridade da evidência, a política para revogação posterior ou comprometimento da chave, a retenção de evento e prova por cópia e o acesso devem ser definidos por Kim/Bruce/Michael/Martin. Casos planejados: revogação posterior, compromisso da chave, prova vencida e restore sem prova. Se a comprovação faltar, manter `unknown` e bloquear a operação dependente; ausência de prova não demonstra ausência do evento. Sem prazo presumido.

**S004-L4 — ACK SOS, OPEN.** Duas cópias no mesmo domínio de falha ou ACK local não demonstram proteção de zona. Nancy/Brendan/Michael/Martin devem definir universo de falhas, independência, evidência e escopo por cópia e modalidade, tratamento sem prova e risco regional. Os casos existentes são planejamento, não resultado operacional.

**S004-L5 — epoch histórico aguardando corte, OPEN.** Evento de epoch anterior com prova de corte ainda desconhecida não é automaticamente inválido nem elegível para aplicação: manter pendente ou em quarentena conforme política de retenção eficaz, sem compor prefixo validado dependente, sem projeção nem efeito externo. Reconciliar por namespace com identidade, integridade, prova de commit dentro do corte e autorização histórica e atual; se prova não chegar, continuar incompleto. Reentrega idêntica não altera o estado pendente por si; conteúdo divergente segue conflito; autorização atual revogada impede replay protegido. Martin/Leslie/Kim/Michael devem conciliar estados e nomes com C10/C07 antes de fixar wire. Casos planejados incluem chegada após fence, duplicata, corte ausente e revogação.

**S004-L6 — referências de autoridade, OPEN.** Na releitura de 16/11/2026, `SPEC-002-identidade-autorizacao.md` já existe; sua existência não confirma que contém a máquina de autoridade por partição. `SPEC-001-arquitetura-federacao.md:46-50` descreve baseline e deixa detalhamento de autoridade pendente; SPEC-002 diferencia autoridades de identidade, tenant e federação. A frase anterior atribuindo a máquina a SPEC-002 foi corrigida na §3. Ada, Leslie e donos devem definir arquivo/versão e fronteiras antes de fechar a referência.

Os cenários desta seção são critérios documentais planejados, não testes executados. As soluções e garantias dependem de integração e revisão independente desta versão; nenhum achado OPEN foi fechado.

## 10. Cobertura requisito → seção → aceite → caso planejado

Esta matriz fecha somente a rastreabilidade documental desta versão. Os casos são planejados, sem ID de teste executável (`NOT_ASSIGNED`) e sem execução (`NOT_EXECUTED`). As aprovações do requisito na origem não aprovam a SPEC, e a cobertura de um subescopo não substitui a jornada integral dos outros donos.

| Requisito | Seções e critério comportamental nesta SPEC | Casos planejados | Limite e dependência |
|---|---|---|---|
| RF-013 | §§3–6: sincronizar somente escopo autorizado; preservar prefixos separados; lacuna, prova ou autoridade ausente bloqueia só operação dependente e a retomada revalida escopo. | S004-05, 08–11 | Autoridade/topologia e recuperação integral em SPEC-001/C06; retenção efetiva C08. |
| RNF-004 | §§3–6: identidade/posição separadas, dedupe no namespace definido, sequência contígua, conflito/quarentena, snapshot/replay e tombstone sem salto. | S004-01–11, 13–15 | Chave final, equivalência injetiva, perfil sem epoch e política de retenção continuam OPEN com C06/C07/C08/C11. |
| RF-011 | §§3–4, 7: registrar evidência por etapa/destino; materialização e reconhecimento não se inferem do ACK de transporte. | S004-02, 04, 12 | Jornada integral de mensagem/SOS e semântica de domínio são das SPEC-006 e donos de produto. |
| RNF-009 | §§3–4, 6–7: conservar referências de evento, posição, operação, tentativa, cópia e prova, sob acesso/retention autorizados. | S004-02–04, 12–15 | Esquema wire, assinatura e códigos C07; política C08; persistência C11. |
| RF-026 | §§4, 7: não declarar aceite SOS protegido, `claimed` ou socorro sem predicado/prova daquela camada e destino. | S004-12 | Predicados e risco operacional são de SPEC-006; proteção por cópia/modalidade OPEN. |
| RNF-012 | §§4, 7: diante de perda de rede/prova, expor estado comprovado ou pendente e recuperação autorizada; não anunciar despacho/entrega. | S004-10–13 | Limiares, alertas e comportamento humano são externos a esta SPEC; não há valores aprovados. |
| RNF-017 | §§4–6: após partição/revogação, não reabrir replay ou acesso protegido sem autorização atual, prova de autoridade e retenção aplicável; desconhecido permanece desconhecido. | S004-10–13 | Regras de grant/identidade C09/SPEC-002, política C08, estado local SPEC-014. |

## 11. Mudanças desta versão — DRAFT-0.6

- Casos por requisito e casos P desbloqueados (C3-1): nova subseção "Casos por requisito (P/N/B/F)" com uma linha por requisito coberto e quatro colunas de caso; casos P dependentes de parâmetro OPEN reescritos para comportamento `param_not_configured`; casos de borda acrescentados em ADR-059, 060, 062 e 063. Nenhum caso foi executado.
- Casos por decisão (resposta ao veto C3-1): nova seção "Casos por decisão (ADR-056…063)", com negativo e falha/recuperação por decisão aplicável; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`. Nenhum caso foi executado.

Mudanças herdadas de DRAFT-0.4:

- **ADR-057:** posição canônica completa com partição; separação dos quatro campos de sequência/epoch; cursor opaco vinculado a contexto e rejeição de cursor sem partição/epoch incomparável.
- **ADR-058:** seis classes fechadas de ACK com `ack_class` obrigatório e ACK de Inbox limitado a `persisted_local`.
- **ADR-061:** Outbox atômica como padrão; Inbox ledger separada somente sem transação comum; replay sem repetir efeitos externos irreversíveis; autoridade `stale/unknown` bloqueia mutação forte.
- **ADR-062:** retenção exige política aprovada por finalidade/classe/cópia/jurisdição/prazo/gatilho; ausência falha fechada.

## 12. Delta DRAFT-0.3 e estado de revisão

Baseline substituído: DRAFT-0.2, SHA-256 `a4fd9955a01db6ef905c731a563bb98195a6990def791062c5e98ff01a9f7b85`. Esta versão incorpora clarificações documentais apoiadas pelos pareceres de B06/B12 e pela comparação das interfaces vigentes:

1. **`event_id` repetido em posições diferentes:** aplica quarentena preventiva até comprovar escopo ou crosswalk admissível; não avança prefixo dependente, não aplica efeito e não declara conflito definitivo. É compatível com a regra C10 de duplicata apenas para mesma identidade completa/posição e com o tratamento de identidade ou sequência conflitante (C10:81–87,95–105). Escopo global do ID, objeto assinado, autoridade do crosswalk e retenção continuam OPEN em C07/C08/C10/C11; não presumir legado nem criar remapeamento.
2. **ACK e fronteira transacional:** commit conjunto, quando as estruturas compartilham transação, é pré-condição de envio do ACK. O significado wire permanece apenas prefixo contíguo Inbox durável para cópia/destino/escopo comprovados, conforme `replicacao.md:33` e `api-inicial.md:54–57`; projeção na mesma transação não amplia o que o ACK declara.
3. **Conciliação das interfaces:** preservada a transação local do produtor mutação+posição+Outbox (`replicacao.md:23–30`; C10:149; C11:136–139); alternativa entre recursos é mudança sujeita a decisão. Inbox/ledger do receptor é outra fronteira. C06 governa autoridade/fence, C07 identidade/wire/cursor/erro, C08 retenção/provas, C11 persistência e migração; nenhuma revisão cruzada substitui integração pelos donos.

| Ponto | Estado textual desta versão | Evidência e próximo fecho |
|---|---|---|
| S004-L1 inventário, linearização e fence | Parcial / OPEN | §9, item S004-L1. Fonte de universo completo, efetores e commits em voo ainda requer decisão técnica e evidência; risco de disponibilidade pelo usuário. |
| S004-L2 vínculo partição/namespace | Parcial / OPEN | §§3,5,9 (S004-L2). Chave/C11, checkpoint/cursor/C07 e mapeamento autenticado ainda precisam integrar representação injetiva ou campos explícitos. |
| S004-L3 prova histórica/retenção | Parcial / OPEN | §§5–6,9 (S004-L3). Confiança de prova, revogação e janela por cópia dependem C08/Kim/Michael; usuário decide política e risco. |
| S004-L4 ACK SOS/domínios independentes | Parcial / OPEN | §§4,7,9 (S004-L4). Predicado por modalidade/cópia e risco regional dependem Nancy/Brendan/Michael e usuário. |
| S004-L5 evento antigo/corte desconhecido | Resolvido textualmente nesta versão | §9 (S004-L5) e S004-09/13: pendente/quarentena, sem efeito ou checkpoint dependente até prova; integração de nomes/retenção segue OPEN. |
| S004-L6 atribuição SPEC-001/002/004 | Resolvido textualmente nesta versão | §§3,9 (S004-L6): C06/Leslie para máquina de autoridade; SPEC-002 para identidade/autorização; fronteira canônica permanece sob conciliação do índice e donos. |
| Escopo de `event_id` e crosswalk | Parcial / OPEN | §§3,4,6,10. Quarentena preventiva é regra provisória de segurança; crosswalk só após autorização/verificação. C07/C11 precisam alinhar contrato e migração. |
| ACK vs materialização | Resolvido textualmente nesta versão | §§4,10; ACK permanece prefixo Inbox; commit compartilhado é condição, não semântica wire adicional. Prova de implementação não existe. |

Fonte de comparação por autor: Michael B06-M-01–05 (`revisao-proposta-b06-michael.md`); Leslie N1–N4 (`revisao-proposta-b06-leslie.md`); Graydon questões de identidade, versão, assinatura, ACK e fallback (`revisao-proposta-b06-graydon.md`); Edsger revisões focais de dedupe/namespace (`revisao-b06-integrado-edsger.md`, `revisao-dedupe-b06-edsger.md`, `revisao-delta-namespace-edsger.md`). Todas são específicas a seus recortes, não constituem aprovação da SPEC-004 DRAFT-0.3. Leslie Lamport deve revisar esta versão pelo hash publicado pelo autor; Michael e Graydon devem conferir as interfaces de dados e GMP descritas acima. Permanecem OPEN as integrações com SPEC-001/002/003/005/006 e as decisões de garantia/política que alterem risco. Nenhum código, SQL, infraestrutura ou teste de produto foi executado; Jev não foi executado nesta parcela; nenhum ACK, recuperação ou sync foi validado.

## Casos por decisão (ADR-056…063)

Casos locais desta SPEC, complementares a S004-01…S004-15. `Estado` sempre `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`. `Tipo`: P positivo, N negativo, B borda, F falha/recuperação. Parâmetro OPEN produz negativa, quarentena ou estado pendente, nunca valor presumido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S004-056-P1 | ADR-056 | Perfil de evento publicado declara N e N-1; evento do perfil N-1 com feature crítica comum e integridade válida | Receber o evento no Inbox | Com janela declarada no perfil, Inbox `validated`, evento integra prefixo contíguo e é projetado; sem janela declarada, `param_not_configured` com rejeição do evento e nenhum aceite por padrão | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-056-N1 | ADR-056 | Evento em versão de perfil fora da janela declarada | Receber o evento | Inbox `rejected`; não integra prefixo, não é projetado e não há downgrade de interpretação | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-056-N2 | ADR-056 | Envelope com campo de semântica crítica desconhecida e sem indicação de opcionalidade | Aplicar o evento | Não aplicado: `quarantined` ou `rejected`; checkpoint dependente não avança | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-056-B1 | ADR-056 | Evento com campo opcional desconhecido e campo removido marcado `reserved` | Repassar o evento a outra cópia | Campo desconhecido preservado byte a byte na reemissão; nenhum número de campo reutilizado; semântica crítica inalterada | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-056-F1 | ADR-056 | Prefixo contíguo contém eventos gravados em perfil novo; serviço revertido para binário anterior | Ler e projetar eventos já persistidos | Leitura bloqueada com estado explícito de incompatibilidade de schema; prefixo congelado, sem reinterpretação nem conversão implícita | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-057-P1 | ADR-057 | Cursor opaco versionado do mesmo principal, escopo, filtros, perfil e retenção; versão do cursor e janela de retenção podem não estar declaradas | Retomar consumo | Com versão e retenção declaradas, eventos a partir do prefixo contíguo sem salto nem duplicata; sem versão de cursor ou janela declarada, `param_not_configured` recusando a retomada sem tradução e sem número presumido | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-057-N1 | ADR-057 | Cursor legado sem `partition` | Apresentar o cursor | Rejeitado; nenhuma tradução para `stream_id` e nenhum evento entregue | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-057-N2 | ADR-057 | Cursor com `authority_epoch` incomparável ao do namespace | Retomar consumo | Rejeição fail-closed; estado de recuperação incompleta e rebootstrap autorizado exigido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-057-B1 | ADR-057 | Partições P1 e P2 com `(authority_epoch, seq)` idênticos | Consumir e deduplicar | Posições distintas preservadas; nenhuma colisão, quarentena indevida ou confusão de provas | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-057-F1 | ADR-057 | Evento de epoch anterior chega após troca de autoridade, sem prova de corte | Aplicar o evento | Permanece pendente/quarentena; não compõe prefixo validado, não projeta e não produz efeito externo | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-058-P1 | ADR-058 | Inbox persistiu o prefixo contíguo; projeção não executou; nome wire de `ack_class` pode não estar aprovado | Emitir confirmação | Com campo aprovado, `ack_class=persisted_local` com escopo e cópia e projeção `pending`; sem nome wire aprovado (D8), confirmação reporta `param_not_configured`/`unknown` e não emite classe presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-058-N1 | ADR-058 | Cópia com Inbox em ledger separado da projeção | Emitir `ack_class=materialized` | Recusado; ACK máximo `persisted_local` e registro de tentativa de escalada | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-058-N2 | ADR-058 | ACK de Inbox recebido para evento SOS | UI/contrato trata o ACK como aceitação protegida, leitura ou atendimento | Recusado; estado exibido como pendente, com camada e destino reais e sem `acknowledged_human` | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-058-F1 | ADR-058 | ACK emitido e perdido; evento idêntico reentregue | Reentregar o evento | Mesmo prefixo comprovado retornado; nenhuma projeção ou efeito externo presumido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-058-B1 | ADR-058 | Grupo de eventos cujo conjunto elegível está incompleto | Emitir agregado por grupo | Agregado não emitido; estados individuais mantidos e nenhuma classe superior inferida | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-059-P1 | ADR-059 | Grant offline válido para operação enumerada; limite de frescor pode estar declarado ou não | Enfileirar evento local e sincronizar | Com frescor declarado, evento aceito com `persisted_local` e nenhum efeito federado; sem limite configurado, `param_not_configured` com recusa e nenhum evento enfileirado | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-059-N1 | ADR-059 | Grant restaurado de snapshot com evidência anterior à alta marca | Sincronizar eventos da cópia restaurada | Negado com `rollback_suspected`; nenhum evento da cópia entra no prefixo | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-059-N2 | ADR-059 | Replay autorizado de eventos históricos; grant revogado no destino | Aplicar replay | Replay não reativa o grant nem amplia prazo; acesso protegido negado e eventos mantidos indisponíveis | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-059-B1 | ADR-059 | Limite de frescor da operação offline não declarado (OPEN) e grant offline válido | Sincronizar evento condicionado ao frescor | Evento mantido fora do prefixo com `param_not_configured` informando frescor não definido; nenhum TTL presumido e nenhum efeito federado | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-059-F1 | ADR-059 | Restore de snapshot anterior a revogação conhecida | Servir dados da cópia e reabrir mutação sensível | Negado até revalidar revogação e autoridade; estado `restore_history_incomplete` enquanto faltar prova | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-060-P1 | ADR-060 | Evento de canal com MLS estabelecido entre endpoints nativos | Projetar timeline do canal | Timeline sem texto claro e busca de conteúdo indisponível; derivados reportam confidencialidade somente com campo aprovado; sem campo aprovado (D9), `param_not_configured`/`unknown` e nenhuma alegação de E2EE | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-060-N1 | ADR-060 | Transcrição produzida por ASR no servidor com consentimento de canal | Consulta declara E2EE para a transcrição | Alegação rejeitada; `confidentiality=terminated` retornado e exibido | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-060-N2 | ADR-060 | Evento derivado sem metadado de confidencialidade | Consumir e projetar o derivado | Tratado como `terminated` por falha fechada; nenhum E2EE inferido de cache, versão anterior ou ausência de erro | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-060-B1 | ADR-060 | Evento de canal com perna nativa MLS e derivado de perna terminadora no mesmo stream | Projetar timeline e consultar derivado | Evento nativo permanece MLS; derivado cai em `terminated` por falha fechada, sem agregar rótulo por stream e sem remover o conteúdo nativo | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-060-F1 | ADR-060 | Bridge recebeu payload e falha antes de devolver resultado | Consultar estado da perna | Resultado `unknown`; derivado permanece `terminated`; cópia já entregue não é declarada apagada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-P1 | ADR-061 | Banco de domínio disponível para o produtor | Mutação que aloca posição e grava Outbox | Transação única e commit único; publicação após commit; nenhum ACK antes do commit | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-N1 | ADR-061 | Autoridade `stale` ou `unknown` para o namespace | Mutação forte | Bloqueada; sem commit, sem publicação e sem `ack_class` de sucesso | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-N2 | ADR-061 | Inbox em ledger separado, projetor idempotente ainda não validado | Emitir ACK `materialized` | Recusado; máximo `persisted_local` até validação documental do projetor | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-B1 | ADR-061 | Restore de backup com Inbox em voo; fluxo independente autorizado existe | Retomar serviço | Operação dependente bloqueada até reconciliação; fluxo independente prossegue conforme sua política | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-F1 | ADR-061 | Crash entre commit e publicação | Retry do publicador | Mesmo evento, posição e digest publicados uma vez; nenhuma duplicata observável | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-F2 | ADR-061 | Inbox separado confirma; projetor falha após efeito e antes do marcador | Reexecutar o projetor | Estado `failed_retryable`/`pending`; retry somente com idempotência demonstrável; nenhum segundo efeito presumido | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-061-F3 | ADR-061 | Fence aplicado no efetor; escritor antigo ainda ativo | Escritor antigo tenta nova mutação; replay de evento com efeito externo | Efetor recusa a nova mutação; replay reconstrói projeção e não repete bridge, RF, pagamento ou notificação | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-P1 | ADR-062 | Política aprovada para a classe e a cópia do evento, Inbox e Outbox; prazo ou jurisdição podem faltar | Persistir e projetar | Com política completa, persistência aceita com `policy_id/version` e retenção aplicada por cópia; com prazo ou jurisdição ausentes, `param_not_configured` com falha fechada e sem retenção presumida | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-N1 | ADR-062 | Classe de evento sem política aprovada | Receber e persistir o evento | Falha fechada: Inbox não integra o evento ao prefixo, erro explícito e degradação declarada; `null` não é tratado como infinito | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-N2 | ADR-062 | Prazo de retenção de Inbox/dedupe não decidido (OPEN) | Consultar retenção da classe | Informa prazo não definido; nenhuma janela de dedupe presumida e nenhuma leitura além do retido é servida | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-B1 | ADR-062 | Política aprovada com prazo definido e jurisdição não definida para a cópia | Persistir Inbox/Outbox e consultar retenção da cópia | Retenção não aplicada para a dimensão não definida: `param_not_configured` reportado, nenhum prazo presumido e nenhum `null` tratado como infinito | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-F1 | ADR-062 | Prova histórica ou chave fora da janela de retenção aprovada | Replay de evento histórico | Prova tratada como desconhecida; replay dependente bloqueado e limitação declarada | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-062-F2 | ADR-062 | Tombstone recebido para um objeto | Tentar inferir exclusão física em todas as cópias | Recusado: tombstone é evento de domínio; nenhuma cópia é declarada eliminada sem recibo por destino | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-P1 | ADR-063 | Artefato de serviço e perfil de evento com SBOM, digest e assinatura válida; `generation` acima do piso | Instalar e aceitar eventos do perfil | Assinatura verificada, `generation` registrada; eventos aceitos | P | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-N1 | ADR-063 | Dependência nova no serviço de sync sem versão resolvida, fonte, licença, digest e análise de vulnerabilidade | Gerar o release | Release não gerado; serviço sem artefato distribuível | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-N2 | ADR-063 | Chave de assinatura histórica revogada após o commit do evento | Replay do evento antigo | Prova tratada como desconhecida: replay bloqueado até regra histórica aprovada; nenhuma assinatura isolada é aceita como prova de commit | N | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-B1 | ADR-063 | Piso de versão assinado; release atual com `generation` igual ao piso | Instalar e aceitar eventos | Aceito; piso e `generation` registrados e nenhum rebaixamento implícito de piso | B | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-F1 | ADR-063 | Release que reintroduz aceitação de evento sem prova de commit, com `generation` inferior ao piso | Tentar downgrade do serviço | Rejeitado pelo piso; versão atual preservada e motivo registrado | F | NOT_EXECUTED / NOT_ASSIGNED |
| S004-063-F2 | ADR-063 | Rollback de release autorizado e limitado no tempo | Aplicar o rollback | Aceito somente com auditoria e piso rebaixado por decisão registrada; sem isso, permanece rejeitado | F | NOT_EXECUTED / NOT_ASSIGNED |

Sem caso executável por decisão ausente, nesta SPEC: (1) janela numérica N/N-1 e perfis incluídos no v1; (2) escopo de `event_id`, fingerprint e bytes assinados (C07/C10); (3) formato wire de cursor e versão do mapeamento (B06/B12); (4) prova de corte, inventário de efetores e fence (C06/S004-L1); (5) janelas de dedupe e validade histórica de chave (C08/S004-L3); (6) predicado de proteção nó/zona e agregação por grupo (C08/SPEC-006); (7) nome wire de `replicated_to_authority` (D8) e do campo `confidentiality` (D9).

### Casos por requisito (P/N/B/F)

Uma linha por requisito da §1. Cada célula referencia caso S004-xx / S004-0xx já definido ou cria ID local descrito na célula; `n/a` só com motivo. Estado de todo caso: `NOT_EXECUTED`; ID formal: `NOT_ASSIGNED`. Cobertura de um subescopo não implica jornada integral do requisito.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-013 | S004-05 | S004-10 | S004-09 | S004-08, S004-11 | Prefixos separados por namespace; autoridade/topologia em SPEC-001/C06. |
| RNF-004 | S004-01 | S004-03, S004-15 | S004-06 | S004-02, S004-04 | Chave final, perfil sem epoch e retenção seguem OPEN em C06/C07/C08/C11. |
| RF-011 | S004-058-P1 | S004-058-N2 | S004-058-B1 | S004-04, S004-12 | Evidência por etapa; jornada de mensagem/SOS é de SPEC-006. |
| RNF-009 | S004-02 | S004-03 | S004-15 | S004-04, S004-13 | Recibo técnico não é ato humano; esquema wire e códigos em C07. |
| RF-026 | n/a — motivo: sem predicado de proteção nó/zona aprovado, não existe caso positivo de aceitação protegida; o comportamento positivo é a própria ausência de alegação | S004-12 | S004-058-N2 | S004-12 | Predicados e risco regional OPEN com SPEC-006/Nancy. |
| RNF-012 | S004-058-F1 | S004-11 | S004-14 | S004-10, S004-13 | Recuperação autorizada explícita; sem limiares aprovados. |
| RNF-017 | S004-057-P1 | S004-059-N2 | S004-09 | S004-10 | Após partição/revogação não reabre replay sem autorização atual e prova. |

Casos desta subseção são critérios documentais; nenhum foi executado ou ligado a teste existente.
