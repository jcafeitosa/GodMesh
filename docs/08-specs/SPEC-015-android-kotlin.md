# SPEC-015: cliente Android/Kotlin

**Versão:** DRAFT-0.4  
**Data:** 2026-11-17  
**Estado:** DRAFT / OPEN; não aprovada, não implementada e não validada.  
**Autor documental:** Chet Haase. **Revisor independente:** Alex Crichton, fronteira Android/offline/FFI.  
**Escopo:** cliente móvel Android, adaptadores Kotlin e suas fronteiras com núcleo Rust, serviços e APIs do sistema. Esta SPEC não escolhe binding, versões mínimas, bibliotecas ou orçamento quantitativo.

## 1. Requisitos e limites

Esta versão especifica o recorte Android proposto para RF-028, RF-029, RF-030, RF-031, RF-033, RF-034, RF-037, RF-042, RF-043 e RNF-016. IDs são cobertura candidata até Ada confirmar mapeamento na matriz. RF-028/029/030/031/033/034/037/042/043 constam aprovados em `docs/00-produto/requisitos.md`; RNF-016 e seus critérios de autoridade/ACK seguem dependentes de conciliação. RF-032, RF-036 e RNF-017 aparecem como interfaces de manual fallback, política Wi-Fi ou sync, não como cobertura integral desta SPEC.

A plataforma Android é aprovada como destino de produto, mas a seleção Kotlin/Jetpack Compose, binding Rust, versões Android, políticas por OEM e compatibilidade de API continuam decisões abertas na ADR-055 e matriz. Esta SPEC não autoriza implementação nem presume que recurso ou API funcione em todo aparelho. Ela descreve contratos esperados e planos futuros de validação.

## 2. Fontes e baseline

| Fonte | Localizador e uso | SHA-256 / estado |
|---|---|---|
| `docs/00-produto/requisitos.md` | RF-028–034, RF-037, RF-042–043; RNF-016 | `c099e505b0d8a0a2077851d596fae6761a14597fc3a8a2f19dad2794c3548664` |
| `docs/01-arquitetura/arquitetura-referencia.md` | plataformas e regras de prevalência; precedência normativa | `555bad39b874b7ba5c8c4cdb87ff5d7f20194afb7fc338a9a93029b829d7cb89` |
| `docs/06-planejamento/decisoes.md` | ADR-047, 053, 055; ordem e seleção tecnológica | `11321182f7b87fbfdd2c2f12dd3273216e64fc623727cf03413728954b2f8903` |
| `docs/06-planejamento/pendencias.md` | dependências de compatibilidade, políticas, energia e critérios | `45cd24d7a5a0a91d95e8affe73500a8a8025a2848716fd491003c7c8bae444c8` |
| `docs/06-planejamento/matriz-rastreabilidade.md` | cobertura, aceite e estado dos RF/RNF | `5b68c1a7ea8675bb27153dbaabd6cc50a367a6f0b6a2fe44483bdabe68397ea3` |
| `docs/01-arquitetura/aplicativo-operacao-normal.md` | estados, offline, permissões e prioridades | `019fa7c5556039bf94769f4b958fae1f633800a8fce82915c5a117c8451e5081` |
| `docs/01-arquitetura/cliente-mobile-sensores-conectividade.md` | DICE, capacidade, permissões, sensores e rotas | `4595020eb457cc71e6923c1988d35fb69bf35fd8099e23fb44b677ed40b7a63e` |
| `docs/01-arquitetura/live-presence.md` | LIVE-ARC-001 v1.0, 2026-11-15, subescopo Android | `1d3c38919656b049fd607c0ef1558c0c0a7e61dc5d7ee2f24446fa68af5e48d5` |
| `docs/03-integracoes/wifi-android-ios-roteadores.md` | RF-037, pedido de associação delegado ao SO | `39227fcc92d512b34563cccbc18331787db0d466de6938d9f1d2668332af31f1` |
| `docs/05-operacao/testes-app-live-presence.md` | casos planejados, sem resultados | `712f759ae0c5abcfeb481b33e71af74b159c85b0afc1ccee99a54d97f9c8d716` |
| `docs/05-operacao/testes-mobile-guardian.md` | cenários móveis Guardian e degradação | `2ded815192ae71e27e6f1fa7c8a8baf9eb02b8692c116b51b0f7731088448f14` |
| Contribuição de Chet | `SPECS-EM-ANDAMENTO/contribuicao-chet-haase.md`, rascunho autoral lido | `c3b7cddcffe4076e68795fcb034200befebe63b539f22972a5f2226843443cfe` |
| Contribuição de Alex | `SPECS-EM-ANDAMENTO/contribuicao-alex-crichton.md`, interface offline/FFI; R-CH-001 resolvido textualmente, Android permanece fora da SPEC Rust–Swift | `29c9d11ac89c132da5033776b34dfbf36e653d5033d281adc24cedc7dc298ed2` |
| Parecer de Chet | `SPECS-EM-ANDAMENTO/revisao-chet-alex-crichton.md`, R-CH-001 resolvido textualmente na versão Alex acima | parecer documental, sem aprovação |

A fonte Android atual de APIs não foi selecionada para esta DRAFT. A SPEC não nomeia API móvel específica como disponível ou aprovada. Antes de implementação, verificar documentação oficial para as versões Android aprovadas e registrar versão, URL e data.

## 3. Atores, autoridade e arquitetura

- **Pessoa usuária:** ativa funções, concede/nega permissões e configura exposição de notificações.
- **Aplicação Android:** coleta apenas capacidades necessárias, executa adaptadores de sistema e apresenta estado local. Não cria prova de identidade, autorização de servidor, entrega ou atendimento humano.
- **Núcleo Rust:** aplica lógica de domínio compartilhável por contrato FFI futuro. Não controla APIs Android, permissões, notificações ou ciclo de vida do processo.
- **Serviço/autoridade GodMesh:** emite eventos e confirmações sob contratos de domínio; ACK é limitado à camada e destino que confirma.
- **Android/OEM:** pode negar, atrasar, suspender ou encerrar trabalho conforme permissão, modo de energia, elegibilidade, política e versão. O aplicativo não promete disponibilidade contínua.

A camada Kotlin adapta lifecycle, permissões, sensores, rede, notificações, armazenamento protegido e associação Wi-Fi. O núcleo Rust recebe entradas validadas e devolve resultados tipados. Binding ainda OPEN; nenhuma estrutura de memória, ABI ou biblioteca é definida nesta revisão.

## 4. Estados e transições

Capacidade por recurso usa os estados documentados pelo DICE: `ready`, `unavailable`, `permission_required`, `hardware_unsupported` e `background_limited`. Estado não observado permanece `unknown`; não mapear `unknown` a pronto ou negado. A projeção Live Presence usa `not_supported`, `permission_required`, `eligible`, `starting`, `active`, `stale`, `ending`, `ended` e `failed`. Estados locais, de rede, sync e atendimento permanecem separados.

1. Ao abrir/retomar ou iniciar operação dependente, avaliar versão/capacidade, permissão, rádio, rede e restrições pertinentes. Registrar fonte e instante.
2. Solicitar somente permissão necessária, no contexto da ação visível. Recusa ou revogação bloqueia a operação dependente, não funções independentes.
3. Ao pausar, encerrar processo, reboot, force-stop, perder rede ou entrar em modo restrito, atualizar apenas o estado conhecido e interromper/suspender trabalho quando seguro. Não alegar continuidade durante ausência do processo.
4. Na retomada, reavaliar permissões, capacidades, autorização vigente conhecida, validade e estado autoritativo antes de nova leitura sensível, envio ou replay.
5. Uma gravação local confirmada significa apenas persistência local. Uma submissão significa tentativa. Só confirmação autenticada do destino indicado atualiza o estado remoto correspondente.

## 5. Contratos por função

### 5.1 Guardian, sensores e CrowdFind

Coletar localização e sensores apenas para função consentida e enquanto a autorização aplicável valer. Preservar fonte, horário, incerteza e frescor. Sensor ausente, permissão revogada ou background limitado suspende somente a função que depende dele e informa o motivo; não gerar falsa precisão, direção Wi-Fi ou SOS automático por check-in atrasado. Beacon CrowdFind permanece opcional, identificador rotativo e sem depender de redes proprietárias Apple/Google. Descoberta não autoriza conexão ou publicação.

### 5.2 Conectividade e energia

Classificar somente rotas realmente expostas pelo SO/serviço autorizado. Métrica de sinal não prova disponibilidade de Internet, serviço ou satélite. Se automação não estiver disponível, orientar ação manual e deixar o resultado desconhecido até confirmação. Sob rede ou bateria restritas, priorizar SOS/check-in/mensagens segundo política aprovada e reduzir primeiro cargas não essenciais do próprio app; manter acesso a comunicação pública de emergência do SO. Não declarar que rádio, operadora ou aplicativo de terceiros foi controlado. Valores, ordem fina, frescor e budgets quantitativos são OPEN.

### 5.3 Wi-Fi AutoConnect

O cliente solicita associação por mecanismo autorizado do Android. Distingue `requested`, decisão/consentimento do sistema, associação observada, autenticação pelo controlador e Internet verificada segundo método contratual. O app nunca cria `ap_authenticated` por inferência local. Sem método e evidência de Internet, manter `unknown`. Não extrair credencial salva nem tratar SSID/BSSID como identidade de pessoa ou AP físico. Falha, cancelamento ou ausência de callback não equivale a associação.

### 5.4 Offline, fila e FFI

`docs/06-planejamento/pendencias.md`, seção “Direções de produto transmitidas em 16/11/2026”, item 4 (linha 128 na versão consultada), registra por relato de Ada a direção “Autorização assinada e limitada”; a coordenação declara não ter conferido diretamente a resposta original. `PAUTA-UNICA-DECISOES-2026-11-16.md:7` declara a mesma origem indireta. Trato isso como direção relatada para orientar propostas, não como política técnica detalhada ou autorização de execução. O registro mantém OPEN as ações, duração, frescor, limites por operação e tratamento SOS. Esta SPEC não define um grant implementável: propõe que uma operação offline só possa ser enfileirada depois que o domínio autorizar sua classe e uma política aprovada definir grant, finalidade, escopo, validade e revogação. Até esses termos serem decididos, esta DRAFT não autoriza escrita offline. Estado `stale` ou `unknown` de autoridade bloqueia mutações fortes e operações que dependem de histórico não comprovado.

Armazenamento local distingue durável e apenas em memória; falta de espaço, chave, corrupção ou falha de gravação não pode ser apresentada como sucesso. Na reconexão, revalidar grant, autorização, retenção e idempotência com a autoridade e o componente de sync. Não afirmar revogação global por revogação local offline. A SPEC-014 de Alex cobre contratos offline/FFI gerais e Rust–Swift; Android/Kotlin está fora do seu aceite até interface própria. Contrato Kotlin deve definir inputs/outputs, ownership/lifetime, concorrência, cancelamento, erros, shutdown, compatibilidade, persistência e replay.

### 5.5 Live Presence e SOS

Criar Live Update apenas para atividade em andamento, iniciada pela pessoa e sensível ao tempo, quando Android/aparelho e permissões forem elegíveis. Em outros casos usar notificação apropriada ou fallback no app. Não usar atividade persistente para “online” ou eventos avulsos. Priorizar SOS visualmente sem apagar Guardian/PTT. Não recolocar atualização dispensada pela pessoa sem nova ação explícita. Não mostrar coordenadas precisas, credenciais ou auditoria Wi-Fi na tela bloqueada.

Cada texto corresponde à evidência autenticada da fase e destino descritos. Mostrar `stale`, pendente ou não confirmado quando atualização falta ou expira. ACK técnico confirma apenas a camada indicada; `claimed` exige evento humano autorizado e resolução exige evento correspondente. Texto e deep link não convertem encaminhamento em assistência. Evento atrasado ou fora de ordem não deve regredir estado terminal sem regra autoritativa.

## 6. Falhas, negativas e recuperação

| Condição | Resultado exigido |
|---|---|
| Permissão negada/revogada | Bloquear função dependente, explicar motivo e alternativa; manter funções não dependentes.
| API/recurso ou hardware ausente | Expor `hardware_unsupported`/`unavailable`; não simular capacidade.
| Restrição de background/energia | Expor `background_limited`; adiar apenas trabalho permitido, sem polling permanente como garantia.
| Rede indisponível | Manter tentativa/fila local distinta de entrega; marcar frescor e estado remoto como pendente/desconhecido.
| Processo encerrado/reboot | Na retomada reconstruir somente estado persistido autorizado e revalidar credenciais/grants/validade; não afirmar que trabalho ocorreu no intervalo.
| Falha de gravação/chave/espaço | Não confirmar durabilidade; conservar em memória apenas se marcado não durável, sem descartar fila silenciosamente.
| Evento repetido/timeout | Usar chave idempotente do contrato; timeout fica com resultado desconhecido até consulta/reconciliação autorizada.
| Notificação dispensada/inelegível | Não repostar indevidamente; manter fallback dentro do app e status real.
| SOS sem confirmação | Mostrar apenas a fase local comprovada; não inferir entrega ou atendimento.

Logs e evidências excluem localização exata, tokens, credenciais, SSID/BSSID desnecessário e conteúdo de mensagem. Identificadores correlacionáveis devem seguir política de retenção/privacidade ainda a aprovar.

## 7. Compatibilidade e aplicabilidade

| Plataforma/produto | Aplicabilidade desta SPEC |
|---|---|
| Android, Personal/Teams | Aplicável, respeitando funções contratadas e autorização por organização.
| Android, Node/Platform/SDK | Aplicabilidade limitada ao cliente/SDK Android autorizado; não define instalação de servidor Android.
| iOS e Apple desktop | Fora do adaptador desta SPEC; contratos compartilhados não substituem SPEC-016.
| macOS/Windows/Linux desktop, servidor Linux e web | Fora da implementação Android; só dependências de contratos comuns, conforme SPECs responsáveis.

Não se declara N/A global de requisito. Ada deve manter mapeamento de produto/plataforma e justificar células inaplicáveis na matriz. Versões mínimas, aparelhos/OEM de referência, ABIs e política de suporte são OPEN.

## 8. Aceite documental e casos planejados

O aceite desta DRAFT exige que cada requisito coberto tenha origem, pré-condição, estímulo, resultado observável, negativa, falha/recuperação, dados expostos, fonte de autoridade, plataforma, dono e critério de fechamento; dependências OPEN continuam atribuídas. Aceite documental não equivale a aprovação. Casos abaixo são planos, não testes executados. IDs: `NOT_ASSIGNED`; observado: `NOT_EXECUTED`.

| Caso planejado | Resultado esperado | Requisitos |
|---|---|---|
| Permissão concedida, negada e revogada durante Guardian | Recurso dependente inicia/para; funções independentes permanecem; status mostra fonte e motivo | RF-028/029, RNF-016 |
| Sensores indisponíveis ou frescor desconhecido | Sem falsa direção/precisão; estado desconhecido visível; nenhum SOS automático de atraso | RF-028/029 |
| CrowdFind opt-in e opt-out | Beacon opcional e rotativo; desativação impede nova emissão segundo política; sem rede proprietária | RF-030 |
| Mudança de rede, sem rota, rota restrita e satélite não confirmado | Só rotas comprovadas elegíveis; manual fallback quando necessário; sem alegar Internet/entrega | RF-031/032 |
| Restrição de bateria/rede com SOS e trabalhos discricionários | Política de prioridade aplicada sem cortar emergência do SO nem inventar orçamento energético | RF-033/043 |
| Inicialização offline, reconexão, grant expirado/revogado | Somente operação explicitamente offline-capaz e grant válido; estado durável/local separado de entrega; replay reconciliado | RF-034/043, RNF-017 |
| Associação Wi-Fi solicitada, negada, associada e sem confirmação | Estados do SO/controlador separados; sem senha extraída ou autenticação inferida | RF-037 |
| Live Update elegível/inelegível, stale, dispensa, expiração e processo encerrado | Inicia só elegível; fallback correto; sem repostagem após dispensa; retoma com revalidação | RF-042 |
| SOS local, encaminhado, ACK técnico, claim humano e resolução | Cada texto exige evidência autenticada da fase/destino; nenhum ACK vira atendimento | RF-042, RNF-016 |
| Corrupção/falta de espaço/chave indisponível/cancelamento FFI | Não declarar escrita durável; erro tipado; retry/recovery conforme contrato e sem duplicação | RF-043 |

Para validação futura, registrar versão/API Android, fabricante/modelo, instalação, estado do processo, permissão, modo de energia, rede, evento sanitizado, esperado/observado e evidência. Definir matriz suportada antes de teste. Não há limite numérico de consumo, latência, frescor ou bateria aprovado nesta SPEC.

## 9. Decisões OPEN

| ID | Decisão, alternativas e risco | Dono proposto | Fechamento |
|---|---|---|---|
| O-01 | Kotlin/Compose e binding Rust: selecionar stack ou comparar alternativas; escolhas afetam lifecycle, acessibilidade e manutenção | Alan integra; usuário aprova direção | Fontes oficiais atuais, compatibilidade real documentada e decisão explícita; não selecionar por este rascunho |
| O-02 | minSdk/targetSdk, arquiteturas, OEMs e matriz de suporte | Produto/usuário, Chet | Cenários/cobertura e política de manutenção aprovados; registro em ADR/matriz |
| O-03 | APIs exatas por trabalho e versão Android | Chet | Documentação oficial versionada e elegibilidade confirmada para cada classe, sem assumir presença universal |
| O-04 | Grant offline: operações, validade, frescor, revogação e SOS | Margaret, Kim/Bruce, Alex/Martin, usuário decide política | Alternativas, ameaça, escopo, limite e eventos de autorização reconciliados; autorização assinada não basta sem semântica aprovada |
| O-05 | Estado/autoridade/frescor de eventos SOS e critério RNF-016 | Nancy, Kim, Kleppmann, Lea | Fases e evidências autenticadas definidas; aprovação de política SOS e revisão independente |
| O-06 | Retenção, backup, proteção de chaves, crash/restore e expurgo local | Michael, Bruce, Alex e Chet | Política por cópia/finalidade com método e evidência; preservar restrições legais ainda não decididas |
| O-07 | Prioridade entre SOS, Guardian, PTT e comunicação pública do SO | Nancy, Lea, Chet, produto | Regras por estado e fallbacks conciliados; não prometer controle de emergência externa |
| O-08 | Budget de bateria/memória, latência, frescor, frequência de sensores e duração de atividade | Chet/Donny propõem; Brendan método; usuário aprova metas | Método, baseline, aparelhos, carga e limites aprovados; sem inventar valores |
| O-09 | Notificações, tela bloqueada, canais e ações acessíveis | Lea, Chet, Nancy/Bruce | Classificação e consentimento por dado/estado definidos e revisados |
| O-10 | Contrato Kotlin/Rust de ownership, async/cancelamento, thread, erro e shutdown | Alex e Chet; Graydon para API | Artefato Android versionado e revisão independente; SPEC-014 não substitui binding Android |

## 10. Dependências, revisão e rastreabilidade

- Alex revisa esta SPEC no recorte offline, persistência e FFI Android. A revisão solicitada deve citar a versão e hash deste arquivo.
- Lea integra jornadas, acessibilidade e texto das permissões; Nancy revisa risco e semântica SOS; Kim/Bruce política de identidade, consentimento e retenção.
- Martin Kleppmann decide contratos de sync/replay; Michael Stonebraker dados e retenção; Graydon APIs/erros; Brendan método para observabilidade e medidas futuras; Donny compatibilidade móvel por interface comum/iOS. Alan integra conflitos e Ada confirma cobertura do índice.

| Requisito | Cobertura desta versão | Limite residual |
|---|---|---|
| RF-028 | ciclo de permissão/localização Guardian e atraso não cria SOS | política Guardian e dono de decisão pendentes |
| RF-029 | capacidade de sensores, origem, incerteza e frescor | método/fusão e thresholds não definidos |
| RF-030 | Beacon opt-in/rotativo sem rede proprietária | contrato CrowdFind e proteção de chaves por política |
| RF-031 | estado de rotas observadas e evidência satelital | compatibilidade e fonte por versão abertas |
| RF-033 | prioridade sob restrição e redução de trabalho discricionário | sem budgets aprovados |
| RF-034 | offline, modos simultâneos e funções independentes | cobertura de telas/jornada por Lea |
| RF-037 | pedido de associação pelo SO e estados distintos | contratos e callbacks por versão ainda a especificar |
| RF-042 | elegibilidade Live Update, fallback, privacidade, stale e SOS | decisão de produto/autoridade e matriz de OEMs abertas |
| RF-043 | energia adaptativa, prioridade, suspensão e retomada | metas, aparelhos e resultados ainda ausentes |
| RNF-016 | projeções só avançam com evidência de fase/destino | critérios normativos de ACK/claim permanecem em conciliação |

## 11. Estado e limites

DRAFT-0.2, OPEN. Esta versão integra a contribuição de Chet e fontes normativas em um contrato Android mais completo, mas deixa sem decisão os itens da tabela OPEN. O parecer de Alex é solicitado; sua existência futura não aprovará automaticamente a SPEC. A integração de Alan, a revisão de rastreabilidade de Ada e a aprovação humana são gates posteriores. Nenhum código, build, teste de produto, banco, serviço ou aparelho foi executado. A implementação continua não verificada.

## Mudanças desta versão — ADR-056, ADR-059 e ADR-060

**ADR-056 — compatibilidade:** declarar suporte por versão e perfil versionado de cliente. A janela é N/N-1 dentro do perfil publicado; os perfis concretos, versões e janela de calendário continuam OPEN até decisão de produto. Sem versão/feature crítica comum, rejeitar antes de qualquer efeito, sem downgrade silencioso. A UI apresenta incompatibilidade e versão/perfil requeridos quando a resposta do servidor fornecer esses dados; não repete mutação nem tenta fallback incompatível. Resultado sem resposta conclusiva permanece desconhecido.

**ADR-059 — offline:** únicas operações autorizadas: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) mensagem a canal já autorizado; (4) observação local de localização; (5) PTT apenas em rede local com grant de floor válido. Negar sempre localização precisa divulgada, mutação federada, criar/alterar grant, mudança de identidade, transmissão RF e transcrição remota. Exigir epoch comparável, frescor dentro do limite do perfil, nenhuma revogação conhecida e anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. Número de frescor desconhecido até medição.

**ADR-060 — confidencialidade:** expor `confidentiality` por canal e por perna; bridge, ASR e gravação que terminem a proteção são marcados `terminated`. Não rotular E2EE sem prova por canal/perna. Não exibir posição precisa ou segredo em tela bloqueada.

- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- **casos por requisito e casos P desbloqueados (C3-1):** subseção `Casos por requisito (P/N/B/F)` ao final, uma linha por requisito coberto; `S015-056-P1`, `S015-059-P1` e `S015-062-P1` reescritos para o comportamento com parâmetro ausente (`param_not_configured`), sem inventar janela, frescor ou prazo; novo `S015-061-B1` para a borda da ADR-061.

Revisões citadas são de versões anteriores; revisão independente desta versão/hash continua pendente. Execução permanece `NOT_EXECUTED`.


## Casos por decisão (ADR-056…063)

Casos documentais planejados. ID formal: `NOT_ASSIGNED`. `Estado` em todos: `NOT_EXECUTED`. Estados citados são os desta SPEC §4 (`ready`, `unavailable`, `permission_required`, `hardware_unsupported`, `background_limited`, `unknown`). Parâmetro OPEN resulta em negar/exibir desconhecido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S015-056-P1 | ADR-056 | Cliente Android em par N/N-1 cuja janela não tem valor aprovado | Chamada de API e de sessão | `compatibility=param_not_configured` exposto na resposta e na tela; `profile_version` recebido é exibido; nenhuma data de suporte nem degradação declarada; nenhum valor de janela presumido | P | NOT_EXECUTED |
| S015-056-N1 | ADR-056 | Servidor rejeita versão/feature incompatível antes do efeito | Tentativa de ação pelo cliente | Nenhuma mutação repetida; tela exibe incompatibilidade e versão/perfil requeridos; sem fallback incompatível | N | NOT_EXECUTED |
| S015-056-B1 | ADR-056 | Janela de suporte OPEN | Consulta de versão mínima | UI exibe `window=not_decided`; nenhuma data de suporte inventada | B | NOT_EXECUTED |
| S015-056-F1 | ADR-056 | Resposta de compatibilidade ausente ou ilegível | Ação em curso | Estado `unknown`; nenhuma mutação apresentada como aceita; fila local mantida pendente | F | NOT_EXECUTED |
| S015-057-P1 | ADR-057 | Fila/inbox local com tupla canônica completa | Replay após reconexão | Itens enviados na ordem canônica; cursor vinculado a principal/escopo/filtros/perfil/retenção; nenhum item colapsado | P | NOT_EXECUTED |
| S015-057-N1 | ADR-057 | Cursor ou item de fila sem `partition` ou com epoch incomparável | Tentativa de replay | Rejeição com erro de cursor; item não traduzido; fila não avança no item | N | NOT_EXECUTED |
| S015-057-F1 | ADR-057 | `authority_epoch` mudou durante a partição | Replay do item antigo | Falha fechada; item marcado `unknown`; nenhum item posterior ultrapassa a lacuna | F | NOT_EXECUTED |
| S015-057-B1 | ADR-057 | Dois eventos com mesma `seq` em partições distintas | Exibição/replay | Ambos preservados com posições distintas; nenhuma deduplicação por chave truncada | B | NOT_EXECUTED |
| S015-058-P1 | ADR-058 | Gravação local confirmada de check-in/mensagem | Envio durante partição | Classe `persisted_local` com estado pendente; nenhum campo de entrega preenchido | P | NOT_EXECUTED |
| S015-058-N1 | ADR-058 | Live Update/notificação exibido sem recibo do destino | Tentativa de exibir `claimed` ou entrega | `claimed` exige evento humano autorizado; sem recibo, `delivered_to_device` não é preenchido | N | NOT_EXECUTED |
| S015-058-F1 | ADR-058 | Timeout de envio; notificação já exibida | Reconciliação posterior | Resultado `unknown` exibido; nenhuma repostagem de notificação por inferência; nenhum estado terminal regredido | F | NOT_EXECUTED |
| S015-058-B1 | ADR-058 | Destino sem suporte a confirmação | Consulta de estado | `confirmation_unavailable`/`unknown` exibido; entrega não fabricada | B | NOT_EXECUTED |
| S015-059-P1 | ADR-059 | Grant vigente; operação enumerada (check-in, mensagem a canal autorizado, observação local, SOS local enfileirado, PTT em rede local com floor); sem limite de frescor aprovado | Execução offline | `grant=valid` e `authorization=unknown` exibidos em dimensões separadas com `freshness_limit=param_not_configured`; nada é enfileirado e nenhuma entrega é afirmada; nenhum limiar embutido no cliente | P | NOT_EXECUTED |
| S015-059-N1 | ADR-059 | Offline | Tentativa de divulgar localização precisa ou de mutação federada | Negado com motivo na tela; nada enfileirado | N | NOT_EXECUTED |
| S015-059-F1 | ADR-059 | `stale`/`unknown` de autoridade, relógio incerto, epoch anterior ou snapshot restaurado | Nova operação condicional | Negação com estado desconhecido exibido; anti-rollback bloqueia o grant; nada persistido na fila | F | NOT_EXECUTED |
| S015-059-B1 | ADR-059 | Rede local sem grant de floor | Tentativa de PTT | PTT não inicia; `permission_required`/negado por floor exibido; nenhum prazo de frescor inventado | B | NOT_EXECUTED |
| S015-060-P1 | ADR-060 | Canal com perna terminadora (bridge, ASR, gravação) | Abertura do canal | `confidentiality=terminated` exibido no detalhe e propagado aos derivados locais | P | NOT_EXECUTED |
| S015-060-N1 | ADR-060 | Tela bloqueada/notificação | Renderização de resumo de canal terminado | Nenhuma coordenada precisa, credencial, SSID/BSSID ou segredo exibido; nenhum rótulo E2EE | N | NOT_EXECUTED |
| S015-060-F1 | ADR-060 | Metadado `confidentiality` ausente na cópia local | Consumo do conteúdo | Tratado como `terminated`; ausência não suprida por cache nem por versão anterior | F | NOT_EXECUTED |
| S015-060-B1 | ADR-060 | Gravação local de áudio por perna que termina proteção | Reprodução/derivação de transcrição local | Derivado marcado `terminated`; nenhuma alegação de E2EE no arquivo | B | NOT_EXECUTED |
| S015-061-P1 | ADR-061 | Persistência local + outbox na mesma transação | Mutação offline permitida | Estado e evento commitados juntos; confirmação local antes de exibir pendente; publicação após commit | P | NOT_EXECUTED |
| S015-061-N1 | ADR-061 | Evento já efetivado, mesmo `event_id` reentregue | Replay | Nenhum efeito externo reemitido (notificação, bridge, rádio); estado terminal preservado | N | NOT_EXECUTED |
| S015-061-F1 | ADR-061 | Processo encerrado, force-stop ou reboot no meio da operação | Retomada do app | Reconstrução apenas do persistido autorizado; nenhuma alegação de trabalho ocorrido no intervalo; ausência de duplicação | F | NOT_EXECUTED |
| S015-061-F2 | ADR-061 | Autoridade `stale`/`unknown` | Mutação forte | Bloqueio com estado explícito; função independente preservada; nenhuma escrita aceita | F | NOT_EXECUTED |
| S015-061-B1 | ADR-061 | Transação local com `outbox` commitado e publicação pendente | Consultar a operação antes do publish | Estado `pendente local` com prova de commit; `materialized` e efeito externo ausentes; leitor não publica nem promove classe | B | NOT_EXECUTED |
| S015-062-P1 | ADR-062 | Política por classe existente para a coleta, com prazo não decidido | Coleta e gravação | Gravação aceita com `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo exibido como aprovado; logs e evidência sem localização exata | P | NOT_EXECUTED |
| S015-062-N1 | ADR-062 | Classe sem política aprovada | Tentativa de persistir histórico/log correlacionável | Falha fechada com erro explícito; nenhum registro persistido; degradação declarada na UI | N | NOT_EXECUTED |
| S015-062-F1 | ADR-062 | Política expirada; cópias incluem storage, cache, logs e backup | Gatilho de descarte | Descarte por cópia registrado; cópia sem confirmação fica pendente; nenhuma remoção global alegada | F | NOT_EXECUTED |
| S015-062-B1 | ADR-062 | Hold com autoridade e revisão | Consulta comum ao dado retido | Dado retido sob prazo próprio; acesso comum não reativado | B | NOT_EXECUTED |
| S015-063-P1 | ADR-063 | APK/App Bundle assinado, com SBOM e `generation` igual ou superior ao piso | Instalação limpa/atualização | Verificação aprovada antes de aplicar; versão, digest e `generation` registrados e exibidos | P | NOT_EXECUTED |
| S015-063-N1 | ADR-063 | Release com `generation` inferior ao piso (versão vulnerável) | Tentativa de instalar atualização anterior | Rejeitada antes de aplicar; versão atual mantida; motivo `generation_below_floor` exibido | N | NOT_EXECUTED |
| S015-063-F1 | ADR-063 | Assinatura inválida ou chave revogada na lista verificada pelo cliente | Aplicação do artefato | Rejeição com erro tipado; nenhuma execução do artefato; nenhum canal alternativo usado | F | NOT_EXECUTED |
| S015-063-B1 | ADR-063 | Atualização N-1 dentro da janela, mas abaixo do piso | Instalação da atualização | Rejeitada; piso prevalece sobre a janela; registro de auditoria do motivo | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** minSdk/targetSdk, OEMs e APIs por versão (O-02/O-03) permanecem OPEN, então não há caso de comportamento por versão ou aparelho concreto. Budgets de bateria/frescor (O-08) e o contrato Kotlin/Rust (O-10) impedem casos positivos com limite numérico ou semântica de cancelamento definida.

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §1 com cobertura candidata. Células citam casos existentes (tabela `Casos por decisão` e §5) ou ID novo descrito na Nota; prefixo `S015-R###` agrupa os quatro tipos do mesmo requisito. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. RF-032, RF-036 e RNF-017 são interfaces/lacunas no §1 e não recebem linha própria aqui.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-028 | S015-R028-P | S015-R028-N | S015-R028-B | S015-R028-F | P: convite/prazo/check-in exibidos só com evento do domínio. N: atraso de check-in não vira SOS. B: prazo de sessão expirado sem evento de encerramento. F: revogação durante background encerra só a função dependente. |
| RF-029 | S015-R029-P | S015-060-N1 | S015-R029-B | S015-R029-F | P: leitura de sensor com fonte, horário, incerteza e frescor. B: sensor presente mas sem permissão → `permission_required`, não `hardware_unsupported`. F: sensor ausente/revogado suspende só a função e informa motivo; nenhuma direção Wi-Fi fabricada. |
| RF-030 | S015-R030-P | S015-R030-N | S015-R030-B | S015-061-F2 | P: Beacon opcional com identificador rotativo e opt-in. N: descoberta não autoriza conexão nem publicação. B: emissão bloqueada por `background_limited` mantém `unknown`, sem alegar anúncio. F: sem rede proprietária, fluxo próprio continua e não há fallback para Find My/Find Hub. |
| RF-031 | S015-R031-P | S015-060-N1 | S015-056-B1 | S015-R031-F | P: classe de conectividade com fonte e instante. N: métrica de sinal ou SSID não afirma internet, serviço ou satélite. F: API ausente deixa estado `unknown` e orienta ação manual (§5.2). |
| RF-033 | S015-059-P1 | S015-059-N1 | S015-059-B1 | S015-R033-F | F: sob bateria/rede restritas, carga `bulk` cede primeiro e comunicação de emergência do SO permanece acessível; valores de orçamento OPEN (§5.2). |
| RF-034 | S015-R034-P | S015-R034-N | S015-R034-B | S015-061-F1 | P: áreas e modos simultâneos alcançáveis com estado local disponível. N: capacidade ausente limita só a função dependente. B: force-stop/reboot durante uso mantém estado conhecido sem alegar continuidade. |
| RF-037 | S015-R037-P | S015-R037-N | S015-R037-B | S015-R037-F | P: `requested` → decisão do SO → associação → autenticação → internet, cada etapa só com evidência. N: ausência de callback não é associação. B: usuário recusa o diálogo do SO. F: falha de autenticação não cria `ap_authenticated` local. |
| RF-042 | S015-R042-P | S015-R042-N | S015-063-B1 | S015-061-F2 | P: Live Update só para atividade em andamento elegível. N: atualização dispensada não é recolocada sem nova ação. B: versão/aparelho sem Live Updates usa notificação/fallback. F: `stale` exibido quando falta atualização; SOS priorizado sem apagar Guardian/PTT. |
| RF-043 | S015-R043-P | S015-058-N1 | S015-058-B1 | S015-061-F1 | P: prioridade preservada com estado real por classe de ACK. Metas de energia e ordem fina OPEN (§5.2); nenhum caso mede SLO. |
| RNF-016 | S015-058-P1 | S015-058-N1 | S015-058-B1 | S015-058-F1 | Rótulo/estado nunca excede a evidência recebida; `ack_class` exibido como recebido (ADR-058). |
