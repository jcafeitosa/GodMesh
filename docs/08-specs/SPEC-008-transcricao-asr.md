# SPEC-008 — Transcrição ASR autorizada por canal

**Versão:** DRAFT-0.6 · **Estado:** DRAFT/OPEN · **Data:** 2026-11-17  
**Autor documental:** Daniel Povey · **Revisor designado:** Bruce Schneier  
O parecer existente refere-se ao DRAFT-0.1. Revisão independente do DRAFT-0.2 pendente. Execução: `NOT_EXECUTED`. Metas quantitativas: **OPEN — a medir**.

**Requisitos diretamente cobertos:** RF-025; RNF-011. Cobertura destes requisitos é parcial enquanto as decisões abaixo permanecerem abertas. Esta SPEC não declara requisito aprovado por ela, implementação ou validação.

## Retenção da transcrição (ADR-065)

Transcrição derivada: **12 meses**, por cópia declarada, gatilho `expiry`, jurisdição BR/CO; revisão jurídica pendente. Tenant pode reduzir. Áudio de origem não é retido por padrão (ADR-065).

## Mudanças desta versão

### DRAFT-0.5 — critério observável no lugar de “conforme política”

`C008-062-F` deixa de exigir “bloqueio conforme política” e passa a nomear o código e o estado observáveis (`RETENTION_POLICY_MISSING` quando classe/cópia/prazo não resolvem, estado da cópia `unknown`/`pending` até evidência de disposição). Nenhum valor numérico, aprovação ou execução é criado.

### DRAFT-0.4 — casos por requisito e casos P desbloqueados (C3-1)

Acrescenta a subseção `### Casos por requisito (P/N/B/F)` ao final, com uma linha por requisito declarado (RF-025, RNF-011) e as quatro colunas preenchidas ou `n/a` justificado. Reescreve os casos P cujo esperado dependia de valor não decidido — janela N/N-1 do perfil de schema e prazo/gatilho da política de retenção de áudio e texto — para o comportamento com parâmetro ausente (`param_not_configured` → negar ou informar desconhecido), preservando cada caso e seu ID. Acrescenta o caso de borda `C008-061-B` (ADR-061), que faltava. Nenhum limiar, prazo ou WER é criado; nada é aprovado, implementado ou executado.

### DRAFT-0.3 — casos por decisão (resposta ao veto C3-1)

O crítico C3 vetou as SPECs por ausência de casos vinculados às decisões ADR-056…063. Esta versão acrescenta a seção “Casos por decisão (ADR-056…063)”, com uma linha por decisão aplicável a esta SPEC, cobrindo ao menos um caso negativo e um de falha/recuperação por decisão. Todo esperado é observável (código, estado ou campo); todo caso permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Parâmetro `OPEN` produz comportamento de negar/informar desconhecido, nunca número inventado. Nenhuma decisão é alterada e nada é aprovado ou executado por este delta.

### ADR-058 — Evidência por fronteira

Toda resposta de API carrega `ack_class`. Cada confirmação identifica emissor, evidência e escopo. API e UI só podem usar as seis classes abaixo. Tempo decorrido, ausência de erro ou sucesso em outra fronteira nunca promovem uma classe. Se a confirmação se perder, o resultado da etapa fica `unknown`.

| Classe | Emissor | Evidência exigida | Escopo confirmado |
|---|---|---|---|
| `transport_accepted` | Transporte receptor | Recibo da tentativa de envio | Aceitação naquele salto |
| `persisted_local` | Dispositivo ou nó que gravou | Registro de persistência do objeto | Objeto e versão naquele armazenamento |
| `replicated_to_authority` | Autoridade de destino | Confirmação da réplica pela autoridade | Objeto e versão naquela autoridade |
| `materialized` | Responsável pela projeção | Aplicação observada do evento à projeção | Projeção e versão identificadas |
| `delivered_to_device` | Dispositivo destinatário | Recibo do próprio dispositivo vinculado ao objeto | Entrega àquele dispositivo |
| `acknowledged_human` | Pessoa autenticada e autorizada | Ação explícita atribuível à pessoa e vinculada ao objeto | Reconhecimento daquela pessoa, para aquele objeto |

Materializar um segmento na timeline não comprova entrega ao dispositivo nem leitura humana.

### ADR-060 — Voz nativa, terminação e identidade

Na voz nativa, E2EE é o padrão entre endpoints GodMesh. As chaves de sessão SRTP derivam do contexto MLS. TLS, DTLS ou SRTP isoladamente não demonstram E2EE. Toda perna em que bridge, ASR ou gravação termina a proteção expõe `confidentiality=terminated` na API e na UI. A transcrição de uma perna com bridge terminadora conserva essa indicação e não pode alegar confidencialidade ponta a ponta.

ASR requer consentimento explícito por canal. Antes de expor áudio ao servidor, o usuário deve ver que habilitar ASR retira E2EE daquele canal. A ratificação de produto para ASR no servidor continua pendente. Somente prova de identidade vinculada à sessão permite atribuir fala a uma pessoa. Na sua ausência, registrar apenas a origem verificável.

## 1. Objetivo e limite

Definir o comportamento documental para processar áudio autorizado de uma fala PTT em texto, associar o resultado à timeline do canal e apresentar incerteza, falha, correções e retenção sem tornar o ASR dependência da voz.

O escopo inclui áudio de origem GodMesh e bridges somente quando autorizado; eventos parcial, final, corrigido e indisponível; identidade/proveniência; política por tenant/canal; privacidade, retenção e isolamento. Não define algoritmo/modelo, qualidade WER, schema final de GMP, base jurídica, política de canal aprovada, prazos, SLO nem implementação.

**Fora do escopo funcional desta SPEC:** SOS como decisão ou evento, reconhecimento automático de emergência, prioridade/arbitragem PTT, transporte de mídia, roteamento de bridge, autorização geral do GodMesh, UI específica por sistema operacional e persistência física. ASR nunca cria SOS definitivo nem substitui o evento SOS estruturado. Radia mantém a autoria de voz/PTT; Kim, de identidade/autorização; Graydon, do contrato GMP; Michael, de dados e cópias; Lea, da UX. Suas interfaces constam como dependências.

## 2. Fontes, precedência e versão do insumo

As fontes foram lidas em 2026-11-16. Arquitetura de referência e ADRs prevalecem segundo `docs/README.md`.

- `docs/00-produto/requisitos.md:32,48`: RF-025 e RNF-011. Os estados aprovados significam direção de produto, não implementação.
- `docs/01-arquitetura/transcricao-multimodal.md`: §§ Finalidade e comportamento; Regra obrigatória de atividade independente de ASR; Identificação correta do falante; Política por canal e privacidade; Pipeline ASR e tolerância à falha; Indicadores e testes; localização conforme ADR-041.
- `docs/01-arquitetura/midia-nativa.md`: §§ Fluxo PTT, Invariantes, Aceitação, Transcrição e chat por canal, SOS e prioridade PTT.
- `docs/07-contratos/transcricao-sos.md`: eventos de voz/transcrição, contratos iniciais e estados. O próprio documento os classifica como proposta não congelada.
- `docs/07-contratos/identidade-autorizacao.md:295,297` e falhas/decisões relacionadas: autorização depende de membership, finalidade, consentimento/ACL e atualidade; grant local não amplia escopo. Esses contratos são propostas documentais, não autorização específica de ASR aprovada.
- `docs/04-seguranca/retencao-revogacao.md:68–90,99`: revalidação antes da divulgação de cópia, distinção entre acesso, revogação e disposição; o mecanismo ASR ainda precisa de integração por cópia.
- `docs/04-seguranca/politica-localizacao.md:7,10`: regra de localização se aplica a transcrição e exportação; não define classificador perfeito para texto livre.
- `docs/06-planejamento/decisoes.md`: ADR-038 (transcrição e política), ADR-040 (timeline independente), ADR-041 (localização coarse no chat), ADR-053 (recursos opcionais cedem sob restrição). São direções aprovadas, não validações.
- `docs/06-planejamento/matriz-rastreabilidade.md`: RF-025/RNF-011 e gates de transcrição, segurança, cliente, sync e evidência. Caso e observado permanecem não atribuídos/não executados até registro e execução reais.
- `SPECS-EM-ANDAMENTO/contribuicao-daniel-povey.md`, SHA-256 `98d41e6a293eb3a44e769b1ba7931bb102c20c9efaad6079b0d6e69fe8384a55` (manifesto e hash conferidos em 2026-11-16), §§1–10: rascunho de origem, aceites propostos e respostas anteriores a Bruce.
- `SPECS-EM-ANDAMENTO/contribuicao-bruce-schneier.md`, SHA-256 `8a18dac0c45c4ad365112619399cfdf1f14c4c28dfb2932e98b00ec971923488`, §§14 e 16, consultado em 2026-11-16: ressalvas independentes sobre autorização ASR, revogação durante publicação, disposição por cópia e localização em texto livre. Bruce classificou esses pontos como parciais/OPEN; sua presença não fecha os achados.

## 3. Termos e invariantes

- **Fala observada:** ocorrência vinculada a `talk_session_id` e evidência de origem. A ocorrência não garante texto nem identidade civil.
- **Transcrição ASR:** texto derivado automaticamente; não é áudio original, prova incontestável ou decisão de segurança.
- **Parcial:** hipótese transitória, de melhor esforço; pode ser substituída conforme política e não conta como resultado definitivo.
- **Final:** resultado final daquela execução/sessão; não significa exatidão. Uma correção cria revisão ligada, sem sobrescrita silenciosa.
- **Corrigido:** revisão que conserva a relação com o final e registra autor/motivo conforme ACL e política aprovadas.
- **Indisponível:** texto não produzido ou não publicável; motivo representa a causa conhecida e não pode ser substituído por frase inventada.
- **Confiança/qualidade:** estimativa do modelo quando disponível. Não é identidade, probabilidade calibrada ou garantia. Sem método validado, não apresentar escore numérico como probabilidade.
- **Proveniência:** rede, origem, sessão, gateway, método de evidência e grau de garantia conhecidos. Diarização pode sugerir separação de vozes, mas não prova identidade.

Invariantes:

1. PTT/voz não depende de ASR para conceder, transmitir, liberar ou registrar a fala na timeline.
2. Cada resultado de ASR deriva de áudio permitido e permanece ligado a tenant, canal, `talk_session_id`, origem e revisão corretos.
3. Política, ACL, finalidade, consentimento explícito por canal e elegibilidade do processador são verificados antes da captura/exposição e revalidados antes de cada nova divulgação.
4. Falha, baixa confiança, idioma não detectado ou fonte não verificada nunca fabricam texto ou identidade.
5. Áudio, buffers, texto parcial/final/corrigido, índice e auditoria são classes de dados com retenção e acesso distinguíveis; política de texto não implica autorização de áudio.
6. Transcrição não determina SOS, entrega, atendimento humano ou prioridade de voz.
7. Dado de outro tenant, canal ou destinatário nunca se torna legível por busca, replay, exportação ou restore fora de ACL e política.

## 4. Atores, autoridade e decisão de autorização

**Atores:** participante PTT; controlador de canal; autoridade de política/tenant; serviço/worker ASR; destinatário autorizado da timeline; cliente; operador de bridge; processador externo, se aprovado; auditor sujeito à finalidade e ACL.

**Comportamento proposto:** antes de disponibilizar áudio a worker, verificar principal/sessão, grant ou origem autorizada, ACL do tenant/canal, modo de transcrição, prova de consentimento explícito por canal, permissão da rede externa e processador permitido. Predicado falso ou desconhecido nega captura/processamento. A timeline pode registrar a transmissão e o motivo `unavailable` quando sua própria política de metadados permitir.

**Lacuna material:** fontes descrevem política por canal, ACL, `disabled`/`opt_in`/`mandatory_with_notice`, consentimento/aviso, processadores permitidos e finalidade, mas não definem artefato de autorização ASR, emissor competente, validade, audiência, escopo por canal/sessão/processador/destino ou evidência de revogação. `mandatory_with_notice` não prova, sozinho, autorização para abrir áudio. A ADR-060 exige consentimento explícito por canal. O formato da prova e a ratificação de produto para ASR no servidor seguem pendentes.

**OPEN AUTH-1 — Autoridade e prova ASR.** Alternativas: referência de política/consentimento emitida pela autoridade do tenant/canal; autorização explícita por fala/sessão/processador; ou combinação de política permanente mais consentimento por participante/sessão quando exigido. Donos: usuário decide política e autoridade; Kim define principal, grant e revogação; Graydon define envelope/eventos; Daniel e Radia definem pontos de verificação no fluxo. Fechar quando emissor, aprovação/aviso, finalidade, audiência, processador, validade, revogação e evidências forem aprovados e testáveis, sem usar participação no canal como autorização implícita.

**OPEN AUTH-2 — E2EE e terminação.** ASR somente em endpoint com acesso legítimo ao áudio claro, ou ASR desativado. Nenhum serviço que receba áudio claro anuncia mídia E2EE inacessível a ele. Donos: Kim, Bruce, Radia e usuário. Fechar por operação com modelo de chaves, endpoint, consentimento e texto de garantia aprovado.

## 5. Fluxo funcional e estados

### 5.1 Processamento

1. Receber evento de início da fala e registrar timeline de voz com base na evidência disponível; não esperar ASR.
2. Avaliar AUTH-1/AUTH-2, ACL, política, origem, processador e capacidade. Negar se falso/desconhecido.
3. Entregar somente a sessão/segmento necessário ao worker autorizado. PCM temporário, se necessário, existe apenas no worker autorizado e segue limites e disposição aprovados.
4. Emitir parcial de melhor esforço, se permitido. Ao encerrar processamento, publicar final com idioma e estimativa de qualidade quando disponíveis, intervalo e proveniência.
5. Correção autorizada gera revisão nova ligada ao evento final. Não reescrever silenciosamente histórico.
6. Aplicar ACL e política independentemente a timeline, busca, exportação e cada cópia.

### 5.2 Estados propostos

`disabled_by_policy`, `consent_missing`, `processing`, `partial`, `final`, `corrected`, `processor_unavailable`, `processing_failed`, `audio_unintelligible`, `unsupported_source`, `e2ee_unavailable`, `interrupted`; quando a publicação é bloqueada após revogação, motivo e estado devem evitar sugerir entrega concluída. Esses nomes consolidam arquitetura e contrato inicial; Graydon decide enumeração, versionamento e compatibilidade. `final` não equivale a reconhecimento correto.

Transições conceituais: transmissão registrada → ASR indisponível por política/consentimento/origem/capacidade; ou `processing` → `partial` zero ou mais vezes → `final`; `final` → `corrected` apenas em revisão autorizada; falha/interrupção → `unavailable` ou `interrupted`. Evento duplicado/reordenado não cria nova entrada nem faz estado terminal regredir. Protocolo de ordenação e conflitos é dependência do GMP/eventos e permanece aberto.

### 5.3 Retentativa, falha e backpressure

Erro de codec, áudio inaudível, timeout, sobrecarga ou desconexão não deve interromper ou atrasar PTT por dependência síncrona de ASR. Mostrar motivo de indisponibilidade conhecido. Backpressure descarta parciais antes de comprometer voz. Retentativa só ocorre se a política permitir armazenamento temporário e a autorização continuar válida. Não reprocessar após revogação conhecida. Se a autorização não puder ser revalidada, bloquear processamento/divulgação de ASR e manter voz segundo sua política.

**OPEN FAIL-1:** decidir ordem entre revogação conhecida e parcial/final em voo, ponto de corte, interrupção/limpeza do buffer, filas e destino. Recomendo bloquear qualquer nova divulgação após revalidação negativa, sem afirmar cancelamento de cópia já recebida. Donos: usuário/Kim (política e autoridade), Graydon/Martin (ordenação e entrega), Radia (efeito no PTT), Michael (cópias). Fechar com casos antes/durante/depois da inferência, por destino, incluindo unknown quando não há confirmação.

## 6. Dados e contratos conceituais

### 6.1 Dados mínimos

Manter, conforme permitido: referência tenant/canal, `talk_session_id`, `event_id`, revisão, referência de speaker e nível de garantia, `origin_network`/`source_provenance`, processador autorizado, idioma quando conhecido, intervalos quando conhecidos, estado/motivo e referência da política aplicada. Texto só é projetado para leitores autorizados. Não requerer nome civil. Conteúdo e telemetria/logs devem ser minimizados; logs não carregam conteúdo ou áudio.

As seguintes entidades são apenas vocabulário existente, não schema aprovado: política de transcrição, entrada de timeline, segmento de transcrição, revisão, auditoria de processador e política/cópia de retenção. Persistência, índices, criptografia em repouso, migrações, semântica de tombstone e consistência física são de Michael e da SPEC de dados.

### 6.2 Eventos e interface

Eventos conceituais encontrados no contrato inicial: `gmp.voice.talk.started.v1`, `gmp.voice.talk.ended.v1`, `gmp.channel.timeline.voice.v1`, `gmp.transcript.partial.v1`, `gmp.transcript.final.v1`, `gmp.transcript.corrected.v1`, `gmp.transcript.unavailable.v1`. A API conceitual lista leitura de timeline e leitura/edição de política. Não há OpenAPI/proto congelado nem API verificada em código. A SPEC-003/GMP deve fixar envelope, assinatura, versionamento, tenant/channel scope, dedupe, ordenação, erros, idempotência e compatibilidade. Esta SPEC não institui novos endpoints nem enumeração definitiva.

### 6.3 Identidade e idioma

Em GodMesh nativo, associar speaker pelo `TalkStart` autenticado/GMC e sessão conforme contrato de identidade. Em bridge, preservar `network_kind`, instância/sessão nativa e identificador reportado com assurance e proveniência; sem vínculo verificado, identificar gateway/origem, não GMC. Idioma pode ser desconhecido, múltiplo ou incerto; não inferir idioma/identidade como fato por uma pontuação não calibrada. Política de seleção de idioma, catálogo por modelo, transliteração e code-switching são OPEN e não devem ser inventados como capacidade atual.

## 7. Retenção, revogação e privacidade

Aplicar política versionada por tenant/canal/região que diferencia áudio bruto, buffer, parcial, final, correções, índice, auditoria, exportação, réplica e backup. Retenção de áudio e texto é independente. Proponho classes parametrizáveis efêmera, operacional e arquivo autorizado; nenhuma classe, finalidade, prazo ou limite está aprovada neste rascunho. Hospedagem e autogestão e regiões parametrizáveis são opções autorizadas pelo usuário; região inicial e valores permanecem abertos.

Expiração pode bloquear consulta antes de haver evidência de disposição em toda cópia. Bloqueio lógico de acesso e disposição física são resultados distintos. Qualquer estado de disposição deve ser atribuído a uma cópia identificável e a um tipo de objeto (áudio, buffer, parcial, final/correção, índice, auditoria, exportação, réplica ou backup); um estado de uma cópia não implica estado de outras cópias nem descarte global. Registrar por cópia status `pending`, `unknown` ou `confirmed` somente se o contrato comum de dados aprovar essa semântica; nesta SPEC são rótulos propostos, não enums vigentes. `confirmed` requer evidência ligada à cópia e à operação de disposição, com emissor/origem e instante observáveis, suficiente para a semântica que o contrato vier a definir; sem essa evidência, manter `unknown` ou `pending`. Esta proposta não define ainda o tipo de comprovante, autoridade emissora, granularidade do inventário ou garantia física. Replay/restore não reexpõe conteúdo após expiração/revogação; após restore, reconciliar a cópia restaurada com a política e estado de acesso antes de nova leitura. Bloqueio de acesso não prova apagamento físico, nem evento de revogação prova que destinatário externo eliminou cópia.

**OPEN RET-1 — Perfil, cópias e prova de disposição.** Alternativas de classe/finalidade e mecanismo de disposição regional requerem escolha do usuário, Bruce e Michael. Fechar com matriz de finalidade, prazo/limites, superfície/cópia, backup/restore, exportação, legal hold e evidência para estado confirmado ou desconhecido. A evidência planejada deve identificar, no mínimo, objeto/cópia e superfície abrangidos, ação/resultado observado, origem responsável e vínculo com a política/versão aplicada; o que isso permite afirmar (bloqueio lógico, remoção daquela cópia ou outra semântica aprovada) precisa ser explicitado. Um comprovante não se estende a outras cópias, exportações ou destinatários sem inventário e evidência próprios. Cenário de aceite: após expiração, consultar e tentar restaurar/reproduzir cada cópia inventariada; comprovar estado de acesso separadamente do estado de disposição, manter cópias sem prova como `unknown`/`pending`, e não apresentar sucesso local como descarte global. Donos: usuário decide perfil e política; Michael e Bruce definem inventário, evidência e semântica comum; Martin trata replay/restore; Lawrence avalia jurisdição quando definida. Não inventar jurisdição ou obrigação jurídica.

**OPEN PRIV-1 — Conteúdo sensível em texto livre.** Coordenadas/localização exata podem aparecer dentro de fala reconhecida, até sem campos de localização. A regra de localização se aplica à transcrição e exportação. Alternativas incluem classificar/redigir por política ou restringir a projeção quando sensibilidade não for classificável; não presumir classificador perfeito nem inferir anonimato do raio coarse. Donos: Bruce, Paul, Lea e Daniel. Fechar com regra para timeline, busca, índice, logs, exportação e estado não classificável, mais cenários sintéticos sem localização real.

## 8. Aplicabilidade de produto e plataformas

| Produto/plataforma | Aplicabilidade | Contrato/limite documental |
|---|---|---|
| Personal, Teams, Node, Platform/SDK | Aplicável quando o produto oferece canal de voz e política de transcrição | Isolamento, finalidade, ACL, retenção e auditoria permanecem por tenant/canal; Node não recebe todos os dados por padrão |
| Servidor Linux em contêiner | Aplicável a worker/controle servidor se a modalidade escolhida processar áudio no servidor | Região, terminação de mídia, processador e ciclo de cópias precisam decisão; hospedagem/autogerida aprovadas como modalidades, sem detalhar implantação |
| macOS arm64/x86_64, iOS | Aplicável para UX, permissões ou ASR local se esse desenho for aprovado | Não especifica frameworks ou API de captura; sem consentimento/permissão, ASR indisponível sem quebrar PTT |
| Windows/Linux desktop, Android | Aplicável para UX/permissões ou ASR local se aprovado | Respeitar capacidade real do SO e declarar indisponível; integrações locais dependem de SPECS de plataforma e validação futura |
| Web | Aplicável ao canal/timeline e origem autorizada; execução ASR pode ocorrer em servidor ou endpoint autorizado | O navegador não é presumido como worker contínuo/permanente; modelo de mídia e disponibilidade não são definidos aqui |

Nenhuma plataforma é excluída globalmente. A seleção de endpoint/processador e capacidade por sistema operacional é OPEN e requer revisão das SPECs de plataforma. UI deve marcar texto como automático, permitir estado indisponível, expor incerteza em linguagem compreensível e manter acessibilidade; rótulos e interação são da Lea.

## 9. Aceite e casos de teste planejados

Todos são casos planejados, sem IDs formais atribuídos e **não executados**. Para cada execução futura registrar versão desta SPEC, contrato/schema, tenant/canal/atores sintéticos, plataforma/região, política/ACL, precondição, estímulo, esperado/observado, evidência sanitizada e custódia conforme contrato de evidências. Critérios quantitativos permanecem OPEN até política/benchmark aprovados.

| Caso | Pré-condição e estímulo | Resultado esperado | Evidência futura |
|---|---|---|---|
| ASR autorizado | Política e autorização válidas, consentimento explícito do canal comprovado; fala PTT reconhecível | Timeline de voz independe do ASR; resultado parcial/final vincula sessão, speaker/origem e estado | Eventos ordenados, política aplicada, projeção pela ACL autorizada |
| Política desligada | Canal `disabled`; iniciar transmissão permitida | Não expor áudio ao worker nem gerar texto; timeline pode informar indisponibilidade sob política de metadados | Decisão de política e traço controlado sem envio de áudio |
| Consentimento/autorização ausente | Predicado falso ou unknown antes da captura | Falhar fechado para ASR, não publicar conteúdo, preservar somente metadado permitido | Predicados e negativa sanitizada |
| Processador/destino não autorizado | Autorização hipotética A; tentar B, exportação ou bridge não abrangida | Negar nova divulgação/processamento; não inferir autorização da participação no canal | Público/destino e versão da autorização, negativa por destino |
| Revogação durante inferência/publicação | Revogar entre captura, inferência, parcial, final e entrega ao próximo destinatário | Negar nova divulgação ao destino sem autoridade atual; voz não é interrompida; estado de cópia remota desconhecida até evidência | Ordem de eventos, confirmação por destino e cópia pendente/unknown; protocolo ainda OPEN |
| Isolamento entre tenants | Atores de tenant B tentam ler, buscar, corrigir, exportar ou replay de A | Negar sem revelar existência ou conteúdo; nenhuma projeção cruzada | Respostas e auditoria sanitizadas |
| ASR indisponível | Worker/serviço falha antes ou durante fala | PTT e timeline continuam; motivo indisponível; nenhum texto inventado | Eventos PTT e falha correlacionados sem dependência síncrona |
| Áudio inaudível/idioma incerto | Áudio sintético sem fala reconhecível ou ambíguo | Indisponível/incerto conforme contrato, sem fabricar texto/idioma/speaker | Estado e proveniência sem conteúdo indevido |
| Correção autorizada | Ator com ACL corrige final | Nova revisão ligada ao original com autor/motivo; sem sobrescrever silenciosamente | Encadeamento de revisão e autorização |
| Expiração/restore/replay | Expirar política aprovada; enumerar cópias/superfícies conhecidas e tentar buscar, restaurar ou repetir evento | Bloquear acesso conforme política; registrar disposição por cópia. Cópia sem evidência adequada permanece `unknown`/`pending`; confirmação de uma cópia não implica confirmação global; restore não reabre conteúdo antes da reconciliação | Versão da política, inventário com objeto/superfície, ação e resultado por cópia, origem/instante da evidência, consultas e tentativas de restore/replay; valores de retenção e semântica de confirmação permanecem OPEN |
| Localização em fala | Conteúdo sintético inclui coordenada precisa; percorrer projeção, índice, log e exportação | Aplicar política de localização; se conteúdo não for classificável, seguir decisão explícita, sem alegar filtro perfeito | Dados sintéticos e resultados por superfície |
| Backpressure | Carga definida por cenário aprovado | Descartar parciais antes de degradar voz; preservar estado final/motivo conforme recursos autorizados | Sinais de mídia e ASR sob carga; limites sem valor fixo nesta versão |
| Bridge sem identidade individual | Fonte externa autorizada sem ID de participante | Atribuir somente gateway/fonte escopada e `speaker_unverified`; nunca inventar GMC | Proveniência e assurance no evento/projeção |

Aceite documental exige contratos coerentes, decisões AUTH/FAIL/RET/PRIV e evidência planejada completa. Aceite técnico futuro exige SPEC aprovada, atribuição formal de casos, critérios/limites aprovados e execução autorizada; esta tabela não representa PASS.

## 10. Riscos, dependências e revisão independente

| Dependência | Entrega necessária para fechar | Responsável documental |
|---|---|---|
| Autoridade/autorização e revogação | Artefato, emissor, escopo, validade, frescor, destinatário e efeito em voo | Kim Cameron; decisão do usuário |
| PTT, concessão e mídia | Fala registrada independentemente de ASR; captura e revogação não degradam voz | Radia Perlman |
| GMP, eventos e ordering | Envelope, idempotência, revisão, ordem, motivo, erros e compatibilidade | Graydon Hoare; Martin Kleppmann para replay/sync |
| Dados e cópias | Isolamento, índices, retenção, backup/restore, expiração e evidência por cópia | Michael Stonebraker; Bruce Momjian na operação PostgreSQL |
| UX e plataformas | Incerteza, origem automática, acessibilidade, permissão e fallback por sistema | Lea Verou; Alex/Donny/Chet nos subescopos clientes |
| Conteúdo geográfico | Regra de transcrição, busca/exportação e estado não classificável | Paul Ramsey, Bruce Schneier, Lea Verou |
| SOS | Evitar inferência ASR como SOS e preservar ACK técnico/humano separado | Nancy Leveson |
| Capacidade/observabilidade | Métricas, admission control, thresholds e método sem comprometer voz | Brendan Gregg e Radia; parâmetros aprovados pelo usuário |

**Revisão Bruce §14/§16, versão observada:** quatro achados anteriores permanecem rastreáveis: (1) autorização específica vs aviso/participação e origem da direção; (2) revogação/publicação em voo; (3) expiração e disposição por cópia; (4) localização em texto livre. A revisão canônica atual acrescentou AUTH-1/AUTH-2, FAIL-1, RET-1 e PRIV-1, sem fechar os quatro achados. A direção original de autorização segue relato indireto via Ada, sem artefato ou autoridade normativa verificáveis. Parecer usado: SHA-256 da contribuição Bruce registrado na §2. Esta anotação não substitui nova revisão desta versão e hash.

**Revisor designado:** Bruce Schneier. Solicitar confronto desta versão/hash, especialmente AUTH-1/AUTH-2, FAIL-1, RET-1 e PRIV-1. Até revisão sobre esta versão, o revisor não atestou a SPEC canônica. Autor deve integrar o parecer no próximo delta, preservando autoria exclusiva e estados OPEN.

### Histórico do DRAFT-0.1 — parecer de Bruce e resposta do autor

Objeto revisto por Bruce em 2026-11-16: SHA-256 `8c82c95869f61e2b9e51ffbbf0976ff62d8c3a22bbae6b88260e768bc5d85f3b`. Parecer em `SPECS-EM-ANDAMENTO/contribuicao-bruce-schneier.md`, §19, SHA-256 `1df327535c28b34ed8fa184b3a4d05030d8c7e520e2eefbabcf1abd398628eae`. Bruce não encontrou conflito normativo confirmado; classificou cinco pontos materiais como pendentes:

| Achado | Resposta incorporada nesta versão | Estado |
|---|---|---|
| AUTH-1 — origem/alcance da autorização e autoridade por sujeito, finalidade, sessão, processador/destino, validade/revogação | §4 explicita que o mandato segue relato indireto e sem artefato, emissores e escopo demonstrados. Casos para ausente, expirado, revogado e destino divergente estão em §9; conteúdo não é autorizado pela existência da SPEC. | OPEN; precisa confirmação da fonte/alcance pelo usuário/coordenação e contrato Kim/Graydon. |
| AUTH-2 — endpoint de confiança e ciclo de chaves/metadados | §4 condiciona processamento a endpoint legítimo e proíbe alegação E2EE inacessível ao terminador. | OPEN; definir participantes, chave, metadados, retirada e terminação por operação. |
| FAIL-1 — concorrência de revogação com chunks/publicação, cancelamento e buffer | §§5.3 e 9 estabelecem bloquear divulgação nova, distinguir estado desconhecido e manter PTT; eventos em voo, ordem e confirmação por destino seguem explicitamente abertos. | OPEN; contrato e evidência por etapa/destino pendentes. |
| RET-1 — estado de acesso e disposição separados por objeto/cópia; restore | §7 separa bloqueio lógico de disposição física, qualifica estados por cópia como proposta e exige reconciliação antes da leitura após restore. | OPEN; semântica comum, inventário, autoridade, legal hold, exportações e prova ainda exigem Michael/Martin/Bruce. |
| PRIV-1 — conteúdo literal ou indireto sensível em texto livre | §7 e os casos de §9 incluem texto de localização literal/não classificável e superfícies de busca/log/exportação; não presume classificador perfeito ou anonimato. | OPEN; decisão por destino e política de conteúdo não classificável pendentes de Bruce/Paul/Lea/usuário. |

Os quatro achados históricos (autorização, revogação, expiração por cópia, localização em texto livre) se cruzam com estes cinco temas, mas não são reduzidos a três. Esta resposta corrige a contagem e registra encaminhamentos, sem declarar os achados fechados. Bruce identificou limites adicionais sobre dimensões dos estados de disposição; §7 já impede interpretar `confirmed` como eliminação global, mas definição compartilhada por domínio continua OPEN.

## 11. Decisões e critério de prontidão

Esta SPEC permanece DRAFT/OPEN até resolver ou aceitar explicitamente todas as decisões materiais abaixo; texto não equivale a escolha:

1. **AUTH-1 — autorização ASR:** fonte/autoridade original da direção relatada; principal emissor, artefato, escopo, validade e prova de revogação. Usuário decide política; Kim/Graydon/Daniel/Radia definem contratos.
2. **AUTH-2 — E2EE e processador:** endpoint permitido e terminação por operação; sem falsa promessa E2EE. Donos: usuário, Kim, Bruce, Radia.
3. **FAIL-1 — revogação em voo:** ponto de corte e concorrência, buffer, publicação parcial/final, filas e confirmação por destino. Donos: Kim, Graydon, Martin, Michael, Radia e usuário.
4. **RET-1 — perfis e cópias:** classes/finalidades, parâmetros regionais, retenção de cada superfície, backup/restore, legal hold e evidência de disposição. Donos: usuário, Bruce, Michael; Lawrence avalia jurisdição após ela ser definida.
5. **PRIV-1 — texto livre sensível:** comportamento para localização e conteúdo não classificável em cada destino. Donos: Bruce, Paul, Lea e Daniel.
6. **QUAL-1 — confiança e línguas:** método/calibração, idioma/ruído suportados, limites de apresentação e ação permitida por estado. Donos: Daniel, Lea, Radia; evidência e limites precisam ser aprovados antes de prometer desempenho.
7. **CAP-1 — capacidade/QoS:** população/carga, recursos, latência, backpressure, janela e critérios quantitativos por cenário. Donos: Brendan/Grady/Radia; usuário aprova metas. Nenhum número foi criado nesta SPEC.
8. **GMP-1 — contratos e compatibilidade:** schema, ordenação, idempotência, enums, correção, erros e versões. Dono: Graydon, com Martin/Michael.

**Condição de fechamento:** cada decisão deve ter fonte/autoridade, alternativa escolhida ou limite aprovado, impacto por produto/plataforma, dono, dependência e critério de validação documental. Parecer de Bruce e conciliação de interfaces registrados; matriz aponta RF-025/RNF-011 para esta SPEC e casos formais atribuídos. Aprovação explícita da SPEC é gate separado. Execução, desempenho, qualidade ASR, compatibilidade real, apagamento e conformidade jurídica permanecem não validados e dependem de autorização posterior.

## Casos por decisão (ADR-056…063)

Casos planejados, sem ID formal: `Estado` é sempre `NOT_EXECUTED` e o ID formal é `NOT_ASSIGNED` em todas as linhas. `Tipo`: **P** positivo, **N** negativo, **B** borda, **F** falha/recuperação. Cada decisão aplicável tem ao menos um caso N e um F. Onde o parâmetro é `OPEN`, o esperado é negar ou informar desconhecido — nunca número, limiar, prazo ou código inventado. Estes casos complementam §9 e não a substituem.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| C008-056-P | ADR-056 | Produtor de transcrição no perfil N e consumidor no perfil N-1; **janela de suporte não declarada** (`OPEN` — Graydon/GMP-1) | Publicar `gmp.transcript.final.v1` e projetar no consumidor N-1 | Esperado reformulado com parâmetro ausente: evento recusado com `param_not_configured`; nenhum segmento projetado e nenhum campo exibido como preservado. Com a janela decidida, vale o esperado positivo original sem novo ID | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-056-N | ADR-056 | Consumidor sem a feature crítica (campo de revisão/ligação ao `final`) exigida pelo evento | Entregar o evento ao consumidor | Evento rejeitado antes da projeção; nenhuma entrada criada na timeline; incompatibilidade declarada, sem downgrade silencioso | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-056-B | ADR-056 | Segmento com campo adicional desconhecido do leitor | Ler e regravar o segmento | Campo desconhecido preservado; número de campo não reutilizado; campo removido marcado `reserved` | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-056-F | ADR-056 | Versão de schema não declarada no perfil do canal (`OPEN` — Graydon/GMP-1) | Publicar e tentar projetar transcrição | Evento tratado como incompatível: rejeitado e estado `unavailable`/`unknown`; nenhum texto parcial apresentado como resultado | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-057-P | ADR-057 | Timeline do canal com segmentos em partição conhecida e `authority_epoch` vigente | Consultar a timeline com cursor versionado da tupla completa | Segmentos daquela partição/epoch devolvidos; cursor com versão declarada e vínculo a principal, escopo, filtros, perfil e retenção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-057-N | ADR-057 | Cursor de timeline construído sem `partition` | Apresentar o cursor à leitura | Cursor rejeitado com erro explícito; nenhum segmento devolvido; nenhuma tradução para outra forma | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-057-B | ADR-057 | Parcial e final do mesmo canal em partições distintas, com mesma `seq` | Consultar as duas partições | Dois registros distintos devolvidos, sem colapso; `partial` não é apresentado como estado terminal | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-057-F | ADR-057 | Cursor emitido antes de restore/troca de `authority_epoch` | Reapresentar o cursor após a troca | Rejeição fechada por epoch incomparável; nenhum texto reexposto; `dispatch_epoch`/`event_sequence` não tratados como o mesmo tipo | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-058-P | ADR-058 | Segmento projetado na timeline com evidência de aplicação | Consolidar a etapa de materialização | Resposta com `ack_class=materialized` e evidência da projeção/versão; UI não afirma entrega ao dispositivo nem leitura humana | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-058-N | ADR-058 | Etapa declarada como entrega a dispositivo | Resposta traz apenas `materialized` | UI não promove: exibe “publicado na timeline” e “entrega não confirmada” em campos distintos; nenhum texto de leitura ou atendimento | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-058-B | ADR-058 | Resposta de API de transcrição sem `ack_class` | Consumir a resposta | Resposta inválida; estado da etapa `unknown`; nenhum segmento exibido como confirmado (nome do código `OPEN` em GMP-1) | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-058-F | ADR-058 | Confirmação de materialização perdida após o commit da projeção | Reconsultar o resultado e repetir a operação com a mesma identidade | Resultado `unknown`, nunca sucesso; repetição não cria segunda entrada de timeline; classe histórica preservada com sua evidência | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-059-P | ADR-059 | PTT offline autorizado por grant delimitado; transcrição remota não está entre as ações enumeradas | Transmitir fala offline e solicitar transcrição | Timeline de voz registrada localmente; ASR em estado `unavailable`/`disabled_by_policy`; nenhum áudio sai do dispositivo | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-059-N | ADR-059 | Grant offline válido apenas para PTT/ações enumeradas | Pedir transcrição remota sob esse grant | Pedido negado como ação não enumerada, sem envio de áudio e sem efeito; motivo registrado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-059-F | ADR-059 | Autorização não revalidável na reconexão (estado `stale`/`unknown` ou revogação conhecida) | Retomar processamento após reconectar | Nenhum processamento nem divulgação nova; estado `consent_missing`/`unavailable` exibido; PTT e timeline de voz preservados | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-059-B | ADR-059 | Revogação conhecida chega durante o buffer de áudio | Concluir a inferência em curso | Nova divulgação bloqueada por destino; nenhum reprocessamento; cópia já recebida mantida como `unknown` até evidência (FAIL-1 aberto) | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-060-P | ADR-060 | ASR executado no endpoint que possui o áudio claro, sem perna servidor de ASR | Gerar transcrição local | Texto marcado como automático; nenhuma terminação indevida declarada; nenhuma alegação de E2EE além do que o perfil comprova | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-060-N | ADR-060 | ASR configurado em servidor com consentimento explícito por canal | Habilitar ASR no canal | Canal e derivados expõem `confidentiality=terminated` na API e na UI; aviso de retirada de E2EE exibido antes de expor áudio; alegação ponta a ponta rejeitada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-060-N2 | ADR-060 | Áudio de bridge com transcodificação alimentando ASR | Publicar transcrição desse áudio | Transcrição herda `confidentiality=terminated` com terminador identificado; nenhum texto prometendo E2EE ou anonimato | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-060-B | ADR-060 | Segmento, índice ou exportação sem metadado de confidencialidade | Consultar cada artefato derivado | Todos tratados como `terminated` (falha fechada); ausência declarada; nenhuma suposição de proteção por cache ou versão anterior | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-060-F | ADR-060 | Metadado perdido em restore de cópia de transcrição | Consultar a cópia restaurada | Cópia apresentada como `terminated` com ausência de metadado declarada; nenhum derivado promovido a protegido | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-061-P | ADR-061 | Worker ASR com transação única e `outbox` para o evento `final` | Publicar o resultado final | `final` persistido e evento de outbox na mesma transação; publicação após commit; `ack_class` máximo `materialized` com prova de projeção | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-061-N | ADR-061 | Outbox com evento `final` pendente de publicação | Executar replay do outbox | Projeção reconstruída sem reexecutar inferência no worker nem chamar processador externo; nenhuma nova revisão criada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-061-N2 | ADR-061 | Segmentos recebidos em Inbox/ledger separado da projeção | Consolidar a recepção do segmento | ACK máximo `persisted_local`; `materialized` não emitido; UI não exibe transcrição aplicada | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-061-B | ADR-061 | Duplicata do mesmo `event_id` de segmento no outbox (retry do publicador) | Executar replay do outbox | Uma única entrada de timeline e uma única revisão; duplicata descartada pela chave idempotente e registrada; nenhuma inferência reexecutada | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-061-F | ADR-061 | Crash entre o commit do `final` e sua publicação | Reiniciar o worker | `final` recuperado e publicado uma única vez; nenhuma revisão duplicada; estado de autoridade `stale`/`unknown` bloqueia publicação forte até reconciliação | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-062-P | ADR-062 | Política com finalidade, classe e cópia declaradas para áudio, parcial e final, mas **prazo e gatilho não decididos** (`OPEN RET-1`) | Gerar e persistir transcrição | Esperado reformulado com parâmetro ausente: objetos gravados com a política identificada e `retention_policy_ref` presente; prazo e gatilho reportados como `param_not_configured`, sem expiração automática anunciada; retenção de áudio e de texto registrada de forma independente | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-062-N | ADR-062 | Nenhuma política aprovada para a classe de texto/áudio daquela cópia | Persistir transcrição ou áudio | Persistência negada com erro explícito; nenhum texto ou áudio gravado, indexado ou exportado; motivo informado sem interromper o PTT | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-062-B | ADR-062 | Cópia em backup/exportação com classe própria e política distinta | Expurgar a cópia primária | Estado por cópia registrado separadamente; cópia sem confirmação permanece `unknown`/`pending`; nenhuma disposição global declarada | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-062-F | ADR-062 | Expiração vencida e disposição física sem evidência | Consultar, restaurar e repetir o evento após a expiração | Consulta negada com `RETENTION_POLICY_MISSING` enquanto a classe/cópia/prazo não resolverem, antes de haver evidência de disposição; restore/replay não reexpõe conteúdo antes da reconciliação; estado da cópia permanece `unknown`/`pending` até recibo de descarte por destino | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-063-P | ADR-063 | Pacote do worker/modelo ASR com assinatura válida, SBOM por release e `generation` igual ou superior ao piso | Carregar o worker e processar uma fala autorizada | Worker carregado após verificação; estado `processing` → `final`; versão e `generation` registradas na evidência | P | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-063-N | ADR-063 | Artefato de modelo com `generation` inferior ao piso assinado | Carregar o modelo | Modelo não carregado; estado `processor_unavailable`; nenhum processamento e nenhum texto gerado | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-063-N2 | ADR-063 | Artefato de modelo adulterado, com assinatura inválida | Carregar o modelo | Rejeição antes de qualquer execução; nenhum modelo carregado; falha registrada sem expor conteúdo de áudio | N | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-063-F | ADR-063 | Chave de assinatura comprometida e lista de revogação publicada | Carregar artefato assinado pela chave revogada | Lista verificada no cliente/host e artefato rejeitado; worker permanece `processor_unavailable` até reemissão de chave | F | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |
| C008-063-B | ADR-063 | Distribuição de modelo sem SBOM ou assinatura declarados (`OPEN` — pipeline não definido) | Tentar carregar o modelo | Artefato tratado como não distribuível: não carregado, estado `processor_unavailable` com motivo explícito; nenhuma exceção silenciosa | B | NOT_EXECUTED; ID formal `NOT_ASSIGNED` |

### Casos por requisito (P/N/B/F)

Uma linha por requisito declarado nesta SPEC. IDs são locais a este documento; toda linha permanece `NOT_EXECUTED` com ID formal `NOT_ASSIGNED`. Célula `n/a` traz o motivo. Caso marcado “reformulado” tem esperado escrito para o parâmetro `OPEN` ausente (`param_not_configured` → negar ou informar desconhecido); quando o valor for decidido, o esperado positivo original pode ser restaurado sem criar novo ID. Complementam §9 e os casos por decisão, sem substituí-los.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-025 | C008-RF025-P1 (novo) | C008-059-N, C008-060-N, C008-060-N2 | C008-056-B, C008-060-B | C008-059-F, C008-061-F | Timeline, marcadores parcial/final/correção e identidade externa preservada; WER e idioma não são prometidos (QUAL-1) |
| RNF-011 | C008-062-P (reformulado) | C008-062-N | C008-062-B | C008-062-F | Classes de áudio, parcial, final, índice e auditoria têm retenção própria; AUTH-1/AUTH-2, FAIL-1 e RET-1 seguem `OPEN` |

Casos novos, descritos em uma linha: **C008-RF025-P1** — política e consentimento explícito do canal válidos, fala PTT reconhecível: parcial e final vinculados a tenant, canal, `talk_session_id` e origem, com idioma/qualidade somente quando disponíveis, e timeline de voz independente do ASR.
