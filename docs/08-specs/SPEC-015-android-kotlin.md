# SPEC-015: clientes GodMesh por *engine* de WebView e por capacidade declarada

**Versão:** DRAFT-0.5
**Data:** 2026-11-17
**Estado:** DRAFT / OPEN; não aprovada, não implementada e não validada.
**Autor documental:** Chet Haase (recorte Android WebView). **Revisor independente:** Alex Crichton, fronteira de comandos/eventos Tauri, offline e capacidade declarada.
**Nota de nome do arquivo:** o arquivo mantém o nome histórico `SPEC-015-android-kotlin.md`. Kotlin/Jetpack Compose **deixam de ser alvo de interface** pela ADR-068; esta SPEC passa a especificar a matriz de *engines* de WebView e as capacidades declaradas do cliente único web empacotado com Tauri 2.
**Escopo:** cliente único web (React + Astryx + Tailwind, ADR-067) empacotado com Tauri 2 para macOS, Windows, Linux, Android e iOS; contratos observáveis por *engine* de WebView e por **capacidade declarada**. Esta SPEC não escolhe versão de engine, versão de SO, framework de estado da UI, biblioteca de componentes fora do já decidido, plugin nativo concreto, nem orçamento quantitativo.

## 1. Requisitos e limites

Esta versão especifica o recorte por *engine* proposto para RF-028, RF-029, RF-030, RF-031, RF-033, RF-034, RF-037, RF-042, RF-043 e RNF-016. IDs são cobertura candidata até Ada confirmar mapeamento na matriz. RF-028/029/030/031/033/034/037/042/043 constam aprovados em `docs/00-produto/requisitos.md`; RNF-016 e seus critérios de autoridade/ACK seguem dependentes de conciliação. RF-032, RF-036 e RNF-017 aparecem como interfaces de manual fallback, política Wi-Fi ou sync, não como cobertura integral desta SPEC.

A ADR-068 é a decisão vigente: existe **uma única interface de produto**, web, e o núcleo Rust roda no processo do app como backend Tauri. A ADR-055 está **SUPERADA**. O que varia entre alvos não é o toolkit de UI, e sim a **engine de WebView** do sistema e as **capacidades** que o alvo declara. A unidade de especificação e de teste passa a ser `(engine, versão declarada, capacidade declarada)`, não `(plataforma, framework nativo)`.

Engines em escopo, sem versão fixada aqui: **WKWebView** (macOS e iOS), **WebView2** (Windows), **WebKitGTK** (Linux) e **Android WebView** (Android). Versões mínimas de engine, de SO e de Tauri; aparelhos de referência; e a matriz oficial de suporte por engine são OPEN e dependem de decisão registrada — esta SPEC não as inventa.

**Regra de arquitetura desta versão:** nenhuma tela conhece plataforma, sistema operacional ou engine. Tela consome apenas **capacidade declarada** e **estado** do núcleo, por contrato tipado. Código nativo só existe como plugin Tauri, com contrato próprio, dono e teste.

## 2. Fontes e baseline

| Fonte | Localizador e uso | SHA-256 / estado |
|---|---|---|
| `docs/00-produto/requisitos.md` | RF-028–034, RF-037, RF-042–043; RNF-016 | `327e259927dd99e3912aaf59e2d59e5b33afb636cf733b177a7a9c30c0b6ef82` |
| `docs/01-arquitetura/arquitetura-referencia.md` | plataformas e regras de prevalência; precedência normativa | `555bad39b874b7ba5c8c4cdb87ff5d7f20194afb7fc338a9a93029b829d7cb89` (hash registrado na versão anterior; não reconferido nesta rodada) |
| `docs/06-planejamento/decisoes.md` | ADR-047, 052, 053, 056–066, **ADR-068** (vigente), ADR-055 (SUPERADA, histórico) | `3351c5e5507b05fbd5acf0345cd4ac5581f4ac7949fe3866e6546b3b38ef0e93` |
| `docs/06-planejamento/pendencias.md` | dependências de compatibilidade, políticas, energia e critérios | `45cd24d7a5a0a91d95e8affe73500a8a8025a2848716fd491003c7c8bae444c8` (hash registrado na versão anterior) |
| `docs/06-planejamento/matriz-rastreabilidade.md` | cobertura, aceite e estado dos RF/RNF | `5b68c1a7ea8675bb27153dbaabd6cc50a367a6f0b6a2fe44483bdabe68397ea3` (hash registrado na versão anterior) |
| `docs/01-arquitetura/aplicativo-operacao-normal.md` | estados, offline, permissões e prioridades | `019fa7c5556039bf94769f4b958fae1f633800a8fce82915c5a117c8451e5081` |
| `docs/01-arquitetura/cliente-mobile-sensores-conectividade.md` | DICE, capacidade, permissões, sensores e rotas | `4595020eb457cc71e6923c1988d35fb69bf35fd8099e23fb44b677ed40b7a63e` (hash registrado na versão anterior) |
| `docs/01-arquitetura/live-presence.md` | LIVE-ARC-001 v1.0, 2026-11-15; subescopo de projeção por atividade | `1d3c38919656b049fd607c0ef1558c0c0a7e61dc5d7ee2f24446fa68af5e48d5` |
| `docs/03-integracoes/wifi-android-ios-roteadores.md` | RF-037, pedido de associação delegado ao SO | `39227fcc92d512b34563cccbc18331787db0d466de6938d9f1d2668332af31f1` (hash registrado na versão anterior) |
| `docs/05-operacao/testes-app-live-presence.md` | casos planejados, sem resultados | `712f759ae0c5abcfeb481b33e71af74b159c85b0afc1ccee99a54d97f9c8d716` (hash registrado na versão anterior) |
| `docs/05-operacao/testes-mobile-guardian.md` | cenários móveis Guardian e degradação | `2ded815192ae71e27e6f1fa7c8a8baf9eb02b8692c116b51b0f7731088448f14` (hash registrado na versão anterior) |
| `docs/09-equipe/registro-gaps.md` | gaps abertos e ação do CTO | `0d5c7f0237f6234c3cd336892a63d50f34e5de6878f0194a89156fa6a594b631` |
| `docs/08-specs/SPEC-013-experiencia-plataformas.md` | UX, acessibilidade e jornadas | `05a54268d0eb7f92e335360f5a92468d0612e6179e98a95b06e89f68dece87b9` (versão anterior à ADR-068; precisa de nova versão) |
| `docs/08-specs/SPEC-014-cliente-offline-ffi.md` | estado local e fronteira hoje descrita como FFI | `57804a79093b9b1e54e71c509ea88681f1d04c45a6bed587cfdc82f5574ff9ed` (versão anterior à ADR-068; ainda nomeia FFI Rust–Swift) |
| `docs/08-specs/SPEC-016-ios-swift.md` | contratos de offline, lifecycle e comandos/eventos Tauri do cliente | ver cabeçalho daquela SPEC (DRAFT-0.6) |
| Contribuição de Chet | `SPECS-EM-ANDAMENTO/contribuicao-chet-haase.md`, rascunho autoral lido | `c3b7cddcffe4076e68795fcb034200befebe63b539f22972a5f2226843443cfe` |
| Contribuição de Alex | `SPECS-EM-ANDAMENTO/contribuicao-alex-crichton.md`, interface offline/fronteira de núcleo | `29c9d11ac89c132da5033776b34dfbf36e653d5033d281adc24cedc7dc298ed2` |
| Parecer de Chet | `SPECS-EM-ANDAMENTO/revisao-chet-alex-crichton.md` | parecer documental, sem aprovação |

A documentação oficial de engine por alvo (WKWebView, WebView2, WebKitGTK, Android WebView) e de Tauri 2 para mobile **não** foi conferida campo a campo nesta versão. Antes de implementação, verificar documentação oficial para as versões declaradas de engine e de Tauri e registrar versão, URL e data. As versões resolvidas em ADR-067 e ADR-068 são registro de resolução, não de instalação nem de compatibilidade validada.

## 3. Atores, autoridade e arquitetura

- **Pessoa usuária:** ativa funções, concede/nega permissões e configura exposição de notificações.
- **Interface web (uma só):** renderiza estado e capacidades; não conhece plataforma, engine nem sistema operacional; não cria prova de identidade, autorização de servidor, entrega ou atendimento humano.
- **Shell Tauri 2:** empacota a UI, hospeda a engine do sistema e expõe comandos/eventos tipados. É a única ponte entre UI e núcleo.
- **Núcleo Rust (no processo):** aplica lógica de domínio, estado local, sync e mídia. Não controla permissões, notificações, áudio de segundo plano, BLE ou ciclo de vida do processo; delega aos plugins.
- **Plugins nativos do Tauri:** única superfície nativa admitida (BLE/beacon, localização em segundo plano, sessão de áudio/PTT, notificações push, Live Activities/Dynamic Island, NFC/biometria, atalhos, discador). Cada plugin tem contrato tipado, dono e teste próprio.
- **Engine do SO:** define o que a UI consegue renderizar e executar. Divergência de API, mídia e desempenho entre engines é risco registrado na ADR-068.
- **Serviço/autoridade GodMesh:** emite eventos e confirmações sob contratos de domínio; ACK é limitado à camada e destino que confirma.
- **SO/OEM:** pode negar, atrasar, suspender ou encerrar trabalho conforme permissão, modo de energia, elegibilidade, política e versão. O aplicativo não promete disponibilidade contínua.

## 4. Estados, capacidades declaradas e transições

Capacidade por recurso usa os estados documentados pelo DICE: `ready`, `unavailable`, `permission_required`, `hardware_unsupported` e `background_limited`. Estado não observado permanece `unknown`; não mapear `unknown` a pronto ou negado. A projeção Live Presence usa `not_supported`, `permission_required`, `eligible`, `starting`, `active`, `stale`, `ending`, `ended` e `failed`. Estados locais, de rede, sync e atendimento permanecem separados.

**Capacidade declarada.** Cada alvo publica, por recurso, uma declaração de capacidade com: recurso, `engine` e versão observada, versão de SO, presença do plugin Tauri, estado DICE, fonte da observação e instante. A UI consome essa declaração; nunca consulta `navigator`, *user agent*, API de engine ou nome de plataforma para decidir comportamento de produto. Capacidade não declarada é tratada como `unknown`; ausência de declaração não autoriza presumir suporte nem presumir ausência.

1. Ao abrir/retomar ou iniciar operação dependente, avaliar capacidade declarada, motor de permissão, rádio, rede e restrições pertinentes. Registrar fonte e instante.
2. Solicitar somente permissão necessária, no contexto da ação visível. Recusa ou revogação bloqueia a operação dependente, não funções independentes.
3. Ao pausar, encerrar processo, reboot, force-stop, perder rede ou entrar em modo restrito, atualizar apenas o estado conhecido e interromper/suspender trabalho quando seguro. Não alegar continuidade durante ausência do processo.
4. Na retomada, reavaliar permissões, capacidades, autorização vigente conhecida, validade e estado autoritativo antes de nova leitura sensível, envio ou replay.
5. Uma gravação local confirmada significa apenas persistência local. Uma submissão significa tentativa. Só confirmação autenticada do destino indicado atualiza o estado remoto correspondente.
6. Ciclo de vida do WebView (suspensão de página, descarte de contexto, congelamento de aba, recarga da engine) é evento de superfície: não equivale a encerramento de atividade, a cancelamento concluído nem a perda de estado durável do núcleo.

## 5. Contratos por função

### 5.1 Guardian, sensores e CrowdFind

Coletar localização e sensores apenas para função consentida e enquanto a autorização aplicável valer. Preservar fonte, horário, incerteza e frescor. Sensor ausente, permissão revogada ou background limitado suspende somente a função que depende dele e informa o motivo; não gerar falsa precisão, direção Wi-Fi ou SOS automático por check-in atrasado. Beacon CrowdFind permanece opcional, identificador rotativo e sem depender de redes proprietárias Apple/Google. Descoberta não autoriza conexão ou publicação.

Localização em segundo plano e BLE dependem de **plugin nativo do Tauri** em cada alvo (ADR-068). Se o plugin não existir ou estiver indisponível no alvo, a capacidade é declarada `hardware_unsupported`/`unavailable`; nenhuma tela contorna isso com API web presumida. Permissões e limites de segundo plano pertencem ao SO e ao plugin, não à página.

### 5.2 Conectividade e energia

Classificar somente rotas realmente expostas pelo SO/serviço autorizado. Métrica de sinal não prova disponibilidade de Internet, serviço ou satélite. Se automação não estiver disponível, orientar ação manual e deixar o resultado desconhecido até confirmação. Sob rede ou bateria restritas, priorizar SOS/check-in/mensagens segundo política aprovada e reduzir primeiro cargas não essenciais do próprio app; manter acesso a comunicação pública de emergência do SO. Não declarar que rádio, operadora ou aplicativo de terceiros foi controlado. Valores, ordem fina, frescor e budgets quantitativos são OPEN.

A engine consome recurso mesmo quando a atividade é do núcleo: o custo de uma tela aberta não é o custo de um processo Rust. Comparações de energia, memória e CPU passam a ser **por engine**, como exige a verificação da ADR-068, e nenhum valor é fixado aqui.

### 5.3 Wi-Fi AutoConnect

O cliente solicita associação por mecanismo autorizado do alvo, sempre através de plugin Tauri quando a plataforma não expõe API web. Distingue `requested`, decisão/consentimento do sistema, associação observada, autenticação pelo controlador e Internet verificada segundo método contratual. O app nunca cria `ap_authenticated` por inferência local. Sem método e evidência de Internet, manter `unknown`. Não extrair credencial salva nem tratar SSID/BSSID como identidade de pessoa ou AP físico. Falha, cancelamento ou ausência de callback não equivale a associação.

### 5.4 Offline, fila e fronteira de comandos

`docs/06-planejamento/pendencias.md`, seção “Direções de produto transmitidas em 16/11/2026”, item 4 (linha 128 na versão consultada), registra por relato de Ada a direção “Autorização assinada e limitada”; a coordenação declara não ter conferido diretamente a resposta original. `PAUTA-UNICA-DECISOES-2026-11-16.md:7` declara a mesma origem indireta. Trato isso como direção relatada para orientar propostas, não como política técnica detalhada ou autorização de execução. O registro mantém OPEN as ações, duração, frescor, limites por operação e tratamento SOS. Esta SPEC não define um grant implementável: propõe que uma operação offline só possa ser enfileirada depois que o domínio autorizar sua classe e uma política aprovada definir grant, finalidade, escopo, validade e revogação. Até esses termos serem decididos, esta DRAFT não autoriza escrita offline. Estado `stale` ou `unknown` de autoridade bloqueia mutações fortes e operações que dependem de histórico não comprovado.

A fronteira UI↔núcleo é de **comandos e eventos tipados do Tauri** dentro do processo, não FFI manual nem ponte por plataforma. SPEC-016 fixa essa fronteira; esta SPEC apenas consome o resultado: estado durável distinto de estado em memória do WebView, cancelamento solicitado distinto de concluído, erro tipado e idempotência preservada. Falta de espaço, chave, corrupção ou falha de gravação não pode ser apresentada como sucesso. Na reconexão, revalidar grant, autorização, retenção e idempotência com a autoridade e o componente de sync. Não afirmar revogação global por revogação local offline.

### 5.5 Live Presence e SOS

Criar Live Update apenas para atividade em andamento, iniciada pela pessoa e sensível ao tempo, quando alvo, aparelho e permissões forem elegíveis. Em outros casos usar notificação apropriada ou fallback no app. Não usar atividade persistente para “online” ou eventos avulsos. Priorizar SOS visualmente sem apagar Guardian/PTT. Não recolocar atualização dispensada pela pessoa sem nova ação explícita. Não mostrar coordenadas precisas, credenciais ou auditoria Wi-Fi na tela bloqueada.

**Superfícies que exigem código nativo, por decisão da ADR-068:**

- **Live Activities e Dynamic Island (iOS)** exigem **extensão nativa iOS** (ActivityKit/WidgetKit); não são alcançáveis por WebView nem por JavaScript. A extensão é um alvo nativo separado, com contrato tipado com o núcleo, dono e teste próprios.
- **Live Updates/notificação persistente (Android)** exigem código nativo Android dentro de plugin Tauri, com o mesmo regime de contrato e teste.
- **Push** exige **plugin Tauri** de notificações e serviço de push por plataforma; sem plugin, a capacidade é declarada `unavailable` e o app usa fallback interno. Entrega de push não é entrega de domínio nem ACK.
- **Áudio/PTT em segundo plano** exige plugin nativo de sessão de áudio; sem ele, PTT contínuo não é entregável (ADR-068, risco 3).

Cada texto corresponde à evidência autenticada da fase e destino descritos. Mostrar `stale`, pendente ou não confirmado quando atualização falta ou expira. ACK técnico confirma apenas a camada indicada; `claimed` exige evento humano autorizado e resolução exige evento correspondente. Texto e deep link não convertem encaminhamento em assistência. Evento atrasado ou fora de ordem não deve regredir estado terminal sem regra autoritativa.

### 5.6 Acessibilidade na WebView

Acessibilidade é contrato de produto, não propriedade da engine. Requisitos mantidos:

1. Toda função acessível por teclado, leitor de tela e zoom do SO quando a engine expuser essas APIs; quando não expuser, é limitação declarada, não comportamento silencioso.
2. Rótulo, papel, ordem de foco e anúncio de mudança de estado descrevem o estado real (`stale`, `unknown`, pendente, negado), nunca sucesso presumido.
3. Nenhuma informação transmitida só por cor, animação ou ícone; alternativa textual obrigatória.
4. Divergência de implementação de acessibilidade entre engines é caso de teste por engine (§8), não presunção de equivalência.
5. Auditoria de acessibilidade em WebView por alvo é exigência da ADR-068 e continua `NOT_EXECUTED`.

Nenhuma tela importa componente de duas bibliotecas nem código nativo por plataforma (ADR-067, emenda); componentes só por `web/ui/`.

## 6. Falhas, negativas e recuperação

| Condição | Resultado exigido |
|---|---|
| Permissão negada/revogada | Bloquear função dependente, explicar motivo e alternativa; manter funções não dependentes.
| API/recurso ou hardware ausente | Expor `hardware_unsupported`/`unavailable`; não simular capacidade.
| Restrição de background/energia | Expor `background_limited`; adiar apenas trabalho permitido, sem polling permanente como garantia.
| Plugin Tauri ausente ou desabilitado no alvo | Expor `unavailable` com motivo do plugin; nenhuma tela tenta caminho alternativo nativo.
| Divergência de API/media entre engines | Declarar capacidade por engine; exibir limitação; não degradar silenciosamente nem trocar de caminho por *user agent*.
| Áudio em segundo plano negado ou interrompido pelo SO | PTT/áudio cessa ou fica `background_limited`; estado de transmissão exibido como não garantido; nada é apresentado como áudio entregue.
| WebView descartado/recarregado | Reconstruir UI a partir do estado do núcleo; nenhum estado durável é inferido do DOM; pendências preservadas.
| Rede indisponível | Manter tentativa/fila local distinta de entrega; marcar frescor e estado remoto como pendente/desconhecido.
| Processo encerrado/reboot | Na retomada reconstruir somente estado persistido autorizado e revalidar credenciais/grants/validade; não afirmar que trabalho ocorreu no intervalo.
| Falha de gravação/chave/espaço | Não confirmar durabilidade; conservar em memória apenas se marcado não durável, sem descartar fila silenciosamente.
| Evento repetido/timeout | Usar chave idempotente do contrato; timeout fica com resultado desconhecido até consulta/reconciliação autorizada.
| Notificação/push dispensado ou inelegível | Não repostar indevidamente; manter fallback dentro do app e status real.
| SOS sem confirmação | Mostrar apenas a fase local comprovada; não inferir entrega ou atendimento.

Logs e evidências excluem localização exata, tokens, credenciais, SSID/BSSID desnecessário e conteúdo de mensagem. Identificadores correlacionáveis devem seguir política de retenção/privacidade ainda a aprovar. Nenhum log é escrito pela UI em console de WebView.

## 7. Compatibilidade e aplicabilidade por engine

| Engine / alvo | Aplicabilidade desta SPEC | Limite declarado |
|---|---|---|
| WKWebView — macOS | Aplicável; capacidades nativas só por plugin Tauri (janela, atalhos, áudio, BLE conforme plugin existente). | Versões de engine e de macOS OPEN; comportamento de segundo plano não é igual ao de app nativo. |
| WKWebView — iOS | Aplicável; Live Activities/Dynamic Island exigem extensão nativa iOS; push exige plugin Tauri; localização/BLE/áudio em segundo plano exigem plugin. | Versões mínimas e entitlements OPEN; extensão nativa tem contrato próprio (SPEC-016). |
| WebView2 — Windows | Aplicável; capacidades nativas só por plugin Tauri. | Versão de runtime WebView2 e sua distribuição/atualização OPEN. |
| WebKitGTK — Linux | Aplicável; capacidades nativas só por plugin Tauri. | Distribuições e versões de WebKitGTK OPEN; diferenças de mídia e aceleração não medidas. |
| Android WebView | Aplicável; Live Updates, BLE, localização em segundo plano, áudio de PTT e push exigem código nativo em plugin Tauri. | Versões de WebView por OEM, fragmentação e política de energia OPEN. |
| Servidor Linux / Node / SDK | Não aplicável à interface de cliente; contratos de serviço permanecem necessários. | Sem projeção visual de cliente. |

Não se declara N/A global de requisito. Ada deve manter mapeamento de produto × engine e justificar células inaplicáveis na matriz. Versões mínimas de engine/SO, aparelhos de referência, política de suporte e matriz de plugins por alvo são OPEN.

## 8. Aceite documental e casos planejados

O aceite desta DRAFT exige que cada requisito coberto tenha origem, pré-condição, estímulo, resultado observável, negativa, falha/recuperação, dados expostos, fonte de autoridade, alvo/engine, dono e critério de fechamento; dependências OPEN continuam atribuídas. Aceite documental não equivale a aprovação. Casos abaixo são planos, não testes executados. IDs: `NOT_ASSIGNED`; observado: `NOT_EXECUTED`.

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
| Corrupção/falta de espaço/chave indisponível/cancelamento de comando | Não declarar escrita durável; erro tipado; retry/recovery conforme contrato e sem duplicação | RF-043 |

### Casos por engine

Casos documentais planejados, um por classe de alvo e por tipo. ID formal `NOT_ASSIGNED`; `Estado` em todos `NOT_EXECUTED`. "Alvo" nomeia a engine; as execuções futuras precisam declarar engine, versão, SO e build.

| ID local | Alvo | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S015-ENG-P1 | Todas as engines | Declaração de capacidade publicada com engine/versão/fonte | Abertura do app e de área dependente | UI mostra capacidade e estado vindos da declaração; nenhuma tela consulta *user agent* ou nome de plataforma; nenhuma versão presumida | P | NOT_EXECUTED |
| S015-ENG-N1 | Duas engines distintas | Recurso de mídia ou API com divergência entre engines | Uso da função dependente | Divergência exposta como limitação daquela engine; nenhuma troca de caminho por detecção de engine dentro da tela; nenhuma promessa de equivalência | N | NOT_EXECUTED |
| S015-ENG-N2 | Qualquer alvo | Revisão de código-fonte da UI | Varredura por import de módulo nativo, `#[cfg]` de plataforma, `user agent` ou import direto de shadcn | **Caso negativo:** nenhuma tela importa código nativo por plataforma; nativo aparece só como plugin Tauri ou extensão iOS com contrato, dono e teste; violação é falha de aceite | N | NOT_EXECUTED |
| S015-ENG-B1 | Android WebView e WebKitGTK | Suporte a áudio de fundo divergente entre as duas engines | Início de PTT e transição para segundo plano | Alvo sem suporte declara `background_limited` e não exibe áudio como garantido; alvo com suporte mantém estado real; nenhum limiar de latência inventado | B | NOT_EXECUTED |
| S015-ENG-B2 | WKWebView iOS | Aparelho sem extensão Live Activity instalada/atualizada | Solicitação de projeção de atividade elegível | Projeção não aparece; estado de domínio inalterado; fallback no app; nenhuma alegação de Dynamic Island | B | NOT_EXECUTED |
| S015-ENG-B3 | WKWebView iOS, Android WebView, WebView2 | Push sem plugin Tauri habilitado | Evento destinado a notificação | Capacidade `unavailable` declarada; nenhuma notificação simulada pela página; fallback interno e estado real | B | NOT_EXECUTED |
| S015-ENG-F1 | Duas engines | Permissão negada no meio da operação e WebView recarregado pelo SO | Retomada | Estado reconstruído do núcleo; permissão reavaliada; nenhuma pendência perdida nem apresentada como concluída; nenhum estado durável inferido do DOM | F | NOT_EXECUTED |
| S015-ENG-F2 | Todas as engines | Plugin nativo presente porém falhando (erro de inicialização) | Uso da capacidade | Erro tipado com causa e camada; capacidade `unavailable`; nenhuma tentativa silenciosa de caminho alternativo | F | NOT_EXECUTED |
| S015-ENG-F3 | Duas engines | Áudio interrompido pelo SO (chamada, foco de áudio) durante PTT | Retorno do foco | Transmissão exibida como interrompida/não garantida; nenhum ACK de áudio; retomada só com nova ação e novo estado observado | F | NOT_EXECUTED |
| S015-ENG-B4 | Todas as engines | Auditoria de acessibilidade por engine | Navegação por teclado e leitor de tela em três telas | Rótulos, papéis, ordem de foco e anúncios de estado corretos; limitação de engine declarada; auditoria registrada como `NOT_EXECUTED` até execução | B | NOT_EXECUTED |

Para validação futura, registrar engine e versão, SO e versão, build, plugin e versão, aparelho/modelo, instalação, estado do processo, permissão, modo de energia, rede, evento sanitizado, esperado/observado e evidência. Definir matriz suportada por engine antes de teste. Não há limite numérico de consumo, latência, frescor ou bateria aprovado nesta SPEC.

## 9. Decisões OPEN

| ID | Decisão, alternativas e risco | Dono proposto | Fechamento |
|---|---|---|---|
| O-01 | ~~Kotlin/Compose e binding Rust~~ **Substituído pela ADR-068.** Permanece aberto apenas o regime de plugins nativos Tauri por alvo (quais recursos são plugin, quem é dono, contrato, teste) | Alan integra; usuário aprova direção | Lista de plugins por alvo com contrato e dono; decisão explícita registrada |
| O-02 | Versões mínimas de SO por alvo, aparelhos de referência e matriz de suporte | Produto/usuário, Chet | Cenários/cobertura e política de manutenção aprovados; registro em ADR/matriz |
| O-03 | Versões de engine por alvo (WKWebView, WebView2, WebKitGTK, Android WebView) e API exata por trabalho | Chet | Documentação oficial versionada por engine e elegibilidade confirmada para cada classe, sem assumir presença universal |
| O-04 | Grant offline: operações, validade, frescor, revogação e SOS | Margaret, Kim/Bruce, Alex/Martin, usuário decide política | Alternativas, ameaça, escopo, limite e eventos de autorização reconciliados; autorização assinada não basta sem semântica aprovada |
| O-05 | Estado/autoridade/frescor de eventos SOS e critério RNF-016 | Nancy, Kim, Kleppmann, Lea | Fases e evidências autenticadas definidas; aprovação de política SOS e revisão independente |
| O-06 | Retenção, backup, proteção de chaves, crash/restore e expurgo local | Michael, Bruce, Alex e Chet | Política por cópia/finalidade com método e evidência; preservar restrições legais ainda não decididas |
| O-07 | Prioridade entre SOS, Guardian, PTT e comunicação pública do SO | Nancy, Lea, Chet, produto | Regras por estado e fallbacks conciliados; não prometer controle de emergência externa |
| O-08 | Budget de bateria/memória por engine, latência, frescor, frequência de sensores e duração de atividade | Chet/Donny propõem; Brendan método; usuário aprova metas | Método, baseline, alvos, carga e limites aprovados; sem inventar valores |
| O-09 | Notificações, tela bloqueada, canais e ações acessíveis, incluindo push por plugin | Lea, Chet, Nancy/Bruce | Classificação e consentimento por dado/estado definidos e revisados |
| O-10 | ~~Contrato Kotlin/Rust~~ **Substituído pela ADR-068.** Permanece aberto o contrato de comandos/eventos Tauri (nomes, payloads, cancelamento, idempotência, erro) detalhado em SPEC-016 | Alex e Chet; Graydon para o contrato | Artefato versionado do contrato e revisão independente |
| O-11 | Extensão nativa iOS (Live Activities/Dynamic Island) e código nativo Android de Live Updates: empacotamento, distribuição, ciclo de vida e contrato com o núcleo | Donny (iOS), Chet (Android), Alex (contrato) | Contrato tipado, dono e teste por extensão; versões e entitlements documentados |
| O-12 | Acessibilidade por engine: cobertura de leitor de tela, teclado e zoom em WebView; método de auditoria | Lea | Método e três telas auditadas por alvo; resultado registrado; nenhum valor inventado |
| O-13 | Maturidade e versões do Tauri 2 mobile e dos plugins usados; política de atualização | Alan, Alex | Versões resolvidas com fonte e data, cobertura de plugins avaliada e decisão registrada |

## 10. Dependências, revisão e rastreabilidade

- Alex revisa esta SPEC no recorte capacidade declarada, comandos/eventos Tauri, offline e persistência. A revisão solicitada deve citar a versão e hash deste arquivo.
- Lea integra jornadas, acessibilidade e texto das permissões; Nancy revisa risco e semântica SOS; Kim/Bruce política de identidade, consentimento e retenção.
- Martin Kleppmann decide contratos de sync/replay; Michael Stonebraker dados e retenção; Graydon APIs/erros; Brendan método para observabilidade e medidas futuras; Donny revisão iOS/extensão nativa; Alan integra conflitos e Ada confirma cobertura do índice.
- `docs/01-arquitetura/live-presence.md` ainda nomeia Swift/ActivityKit/SwiftUI como mecanismo de projeção iOS. A revisão desse arquivo está **fora** do escopo de edição desta SPEC e permanece pendente com seu dono.

| Requisito | Cobertura desta versão | Limite residual |
|---|---|---|
| RF-028 | ciclo de permissão/localização Guardian e atraso não cria SOS | política Guardian e dono de decisão pendentes |
| RF-029 | capacidade de sensores, origem, incerteza e frescor | método/fusão e thresholds não definidos |
| RF-030 | Beacon opt-in/rotativo sem rede proprietária, por plugin nativo | contrato CrowdFind e proteção de chaves por política |
| RF-031 | estado de rotas observadas e evidência satelital | compatibilidade e fonte por versão/engine abertas |
| RF-033 | prioridade sob restrição e redução de trabalho discricionário | sem budgets aprovados |
| RF-034 | offline, modos simultâneos e funções independentes com uma única UI | cobertura de telas/jornada por Lea; viabilidade depende de plugins |
| RF-037 | pedido de associação pelo SO (via plugin) e estados distintos | contratos e callbacks por versão/engine ainda a especificar |
| RF-042 | elegibilidade Live Update, fallback, privacidade, stale e SOS; extensão nativa iOS obrigatória | decisão de produto/autoridade e matriz por engine abertas |
| RF-043 | energia adaptativa, prioridade, suspensão e retomada por engine | metas, alvos e resultados ainda ausentes |
| RNF-016 | projeções só avançam com evidência de fase/destino | critérios normativos de ACK/claim permanecem em conciliação |

## 11. Estado e limites

DRAFT-0.5, OPEN. Esta versão substitui o recorte “por plataforma nativa” pelo recorte “por *engine* de WebView e por capacidade declarada”, conforme ADR-068, e mantém sem decisão os itens da tabela OPEN. A revisão de Alex é solicitada; sua existência futura não aprovará automaticamente a SPEC. A integração de Alan, a revisão de rastreabilidade de Ada e a aprovação humana são gates posteriores. Nenhum código, build, teste de produto, banco, serviço, plugin, extensão nativa ou aparelho foi executado. A implementação continua não verificada.

## Mudanças desta versão — ADR-068 (DRAFT-0.4 → DRAFT-0.5)

**Decisão aplicada:** ADR-068, aprovada pelo usuário em 17/11/2026, com a ADR-055 marcada **SUPERADA**. Cliente único web (React 19 + Astryx + Tailwind, ADR-067) empacotado com **Tauri 2** em macOS, Windows, Linux, iOS e Android; núcleo Rust no processo como backend Tauri; plugins nativos como única superfície nativa.

1. **Unidade de especificação e teste:** “por plataforma nativa” → “por *engine* de WebView e por capacidade declarada” (WKWebView macOS/iOS, WebView2 Windows, WebKitGTK Linux, Android WebView). Nenhuma versão de engine ou limiar é fixada.
2. **Nenhuma tela conhece plataforma:** a UI consome apenas capacidade declarada e estado; consultar *user agent*, nome de plataforma ou API de engine para decidir produto é proibido e vira caso negativo (§8, `S015-ENG-N2`).
3. **Contratos preservados:** estados DICE (`ready`, `unavailable`, `permission_required`, `hardware_unsupported`, `background_limited`, `unknown`), estados de Live Presence, regra de que `unknown` não vira pronto nem negado, separação entre estado local/remoto/ACK, tela bloqueada sem dados precisos e prioridade de SOS permanecem como estavam.
4. **Acrescentado:** Live Activities/Dynamic Island exigem **extensão nativa iOS**; Live Updates Android exigem código nativo em plugin; **push exige plugin Tauri**; áudio/PTT em segundo plano, BLE e localização em background também exigem plugin (ADR-068, risco 3). Sem plugin, capacidade é `unavailable`/`background_limited` declarada.
5. **Acrescentado:** seção de acessibilidade na WebView como contrato de produto, com auditoria por engine.
6. **Acrescentado:** casos por engine (divergência de API, áudio em segundo plano, permissão negada, plugin falhando, extensão ausente) e caso negativo de tela importando código nativo por plataforma.

**Impacto por requisito:**

- **RF-034** (app normal, modos simultâneos, offline): deixa de depender de toolkit nativo e passa a depender de **uma** UI e de plugins por alvo. Risco muda de “quatro bases de UI” para “plugins e engines divergentes”; a cobertura de jornada continua de Lea (SPEC-013, precisa de nova versão).
- **RF-043** (energia adaptativa, comunicação essencial): custo e comportamento passam a ser medidos **por engine**, e áudio/PTT em segundo plano passa a ser condição de entrega via plugin nativo. Metas continuam OPEN.
- **RF-042** (Live Presence): a projeção iOS deixa de ser alcançável só por SwiftUI; exige extensão nativa iOS e contrato tipado com o núcleo. Android exige código nativo em plugin. Fallback no app permanece para os demais alvos. Nenhum requisito de RF-042 é rebaixado.

**Nota de escopo:** SwiftUI e Kotlin/Jetpack Compose **deixam de ser alvo** desta SPEC. O nome do arquivo é histórico e não indica a stack. A ADR-067 mantém-se vigente para a interface web; nenhum framework de estado de UI é escolhido aqui.

## Mudanças anteriores — ADR-056, ADR-059 e ADR-060

**ADR-056 — compatibilidade:** declarar suporte por versão e perfil versionado de cliente. A janela é N/N-1 dentro do perfil publicado; os perfis concretos, versões e janela de calendário continuam OPEN até decisão de produto. Sem versão/feature crítica comum, rejeitar antes de qualquer efeito, sem downgrade silencioso. A UI apresenta incompatibilidade e versão/perfil requeridos quando a resposta do servidor fornecer esses dados; não repete mutação nem tenta fallback incompatível. Resultado sem resposta conclusiva permanece desconhecido.

**ADR-059 — offline:** únicas operações autorizadas: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) mensagem a canal já autorizado; (4) observação local de localização; (5) PTT apenas em rede local com grant de floor válido. Negar sempre localização precisa divulgada, mutação federada, criar/alterar grant, mudança de identidade, transmissão RF e transcrição remota. Exigir epoch comparável, frescor dentro do limite do perfil, nenhuma revogação conhecida e anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. Número de frescor desconhecido até medição.

**ADR-060 — confidencialidade:** expor `confidentiality` por canal e por perna; bridge, ASR e gravação que terminem a proteção são marcados `terminated`. Não rotular E2EE sem prova por canal/perna. Não exibir posição precisa ou segredo em tela bloqueada.

Revisões citadas são de versões anteriores; revisão independente desta versão/hash continua pendente. Execução permanece `NOT_EXECUTED`.

## Casos por decisão (ADR-056…063)

Casos documentais planejados. ID formal: `NOT_ASSIGNED`. `Estado` em todos: `NOT_EXECUTED`. Estados citados são os desta SPEC §4 (`ready`, `unavailable`, `permission_required`, `hardware_unsupported`, `background_limited`, `unknown`). Parâmetro OPEN resulta em negar/exibir desconhecido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S015-056-P1 | ADR-056 | Cliente em par N/N-1 cuja janela não tem valor aprovado | Chamada de API e de sessão | `compatibility=param_not_configured` exposto na resposta e na tela; `profile_version` recebido é exibido; nenhuma data de suporte nem degradação declarada; nenhum valor de janela presumido | P | NOT_EXECUTED |
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
| S015-063-P1 | ADR-063 | Pacote assinado do cliente (por alvo) com SBOM e `generation` igual ou superior ao piso | Instalação limpa/atualização | Verificação aprovada antes de aplicar; versão, digest e `generation` registrados e exibidos | P | NOT_EXECUTED |
| S015-063-N1 | ADR-063 | Release com `generation` inferior ao piso (versão vulnerável) | Tentativa de instalar atualização anterior | Rejeitada antes de aplicar; versão atual mantida; motivo `generation_below_floor` exibido | N | NOT_EXECUTED |
| S015-063-F1 | ADR-063 | Assinatura inválida ou chave revogada na lista verificada pelo cliente | Aplicação do artefato | Rejeição com erro tipado; nenhuma execução do artefato; nenhum canal alternativo usado | F | NOT_EXECUTED |
| S015-063-B1 | ADR-063 | Atualização N-1 dentro da janela, mas abaixo do piso | Instalação da atualização | Rejeitada; piso prevalece sobre a janela; registro de auditoria do motivo | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** versões mínimas de engine/SO e aparelhos (O-02/O-03) permanecem OPEN, então não há caso de comportamento por versão concreta de engine ou aparelho. Budgets de bateria/frescor (O-08) e o contrato de comandos/eventos Tauri (O-10) impedem casos positivos com limite numérico ou semântica de cancelamento definida.

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §1 com cobertura candidata. Células citam casos existentes (tabela `Casos por decisão`, `Casos por engine` e §5) ou ID novo descrito na Nota; prefixo `S015-R###` agrupa os quatro tipos do mesmo requisito. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. RF-032, RF-036 e RNF-017 são interfaces/lacunas no §1 e não recebem linha própria aqui.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-028 | S015-R028-P | S015-R028-N | S015-R028-B | S015-R028-F | P: convite/prazo/check-in exibidos só com evento do domínio. N: atraso de check-in não vira SOS. B: prazo de sessão expirado sem evento de encerramento. F: revogação durante background encerra só a função dependente. |
| RF-029 | S015-R029-P | S015-060-N1 | S015-R029-B | S015-R029-F | P: leitura de sensor com fonte, horário, incerteza e frescor. B: sensor presente mas sem permissão → `permission_required`, não `hardware_unsupported`. F: sensor ausente/revogado suspende só a função e informa motivo; nenhuma direção Wi-Fi fabricada. |
| RF-030 | S015-R030-P | S015-R030-N | S015-R030-B | S015-061-F2 | P: Beacon opcional com identificador rotativo e opt-in, por plugin nativo. N: descoberta não autoriza conexão nem publicação. B: emissão bloqueada por `background_limited` mantém `unknown`, sem alegar anúncio. F: sem rede proprietária, fluxo próprio continua e não há fallback para Find My/Find Hub. |
| RF-031 | S015-R031-P | S015-060-N1 | S015-056-B1 | S015-R031-F | P: classe de conectividade com fonte e instante. N: métrica de sinal ou SSID não afirma internet, serviço ou satélite. F: API ausente deixa estado `unknown` e orienta ação manual (§5.2). |
| RF-033 | S015-059-P1 | S015-059-N1 | S015-059-B1 | S015-R033-F | F: sob bateria/rede restritas, carga `bulk` cede primeiro e comunicação de emergência do SO permanece acessível; valores de orçamento OPEN (§5.2). |
| RF-034 | S015-R034-P | S015-R034-N | S015-R034-B | S015-061-F1 | P: áreas e modos simultâneos alcançáveis com estado local disponível na UI única. N: capacidade ausente limita só a função dependente. B: force-stop/reboot ou recarga da WebView durante uso mantém estado conhecido sem alegar continuidade (S015-ENG-F1). |
| RF-037 | S015-R037-P | S015-R037-N | S015-R037-B | S015-R037-F | P: `requested` → decisão do SO → associação → autenticação → internet, cada etapa só com evidência. N: ausência de callback não é associação. B: usuário recusa o diálogo do SO. F: falha de autenticação não cria `ap_authenticated` local. |
| RF-042 | S015-R042-P | S015-R042-N | S015-ENG-B2 | S015-061-F2 | P: Live Update só para atividade em andamento elegível, com extensão nativa quando exigida. N: atualização dispensada não é recolocada sem nova ação. B: alvo/aparelho sem Live Activities/Update usa notificação/fallback (S015-ENG-B3). F: `stale` exibido quando falta atualização; SOS priorizado sem apagar Guardian/PTT. |
| RF-043 | S015-R043-P | S015-058-N1 | S015-058-B1 | S015-ENG-F3 | P: prioridade preservada com estado real por classe de ACK. B: interrupção de áudio pelo SO exibe transmissão não garantida. Metas de energia e ordem fina OPEN (§5.2); nenhum caso mede SLO. |
| RNF-016 | S015-058-P1 | S015-058-N1 | S015-058-B1 | S015-058-F1 | Rótulo/estado nunca excede a evidência recebida; `ack_class` exibido como recebido (ADR-058). |