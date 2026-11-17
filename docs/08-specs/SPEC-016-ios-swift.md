# SPEC-016 — cliente web offline, lifecycle e fronteira de comandos/eventos Tauri

**Versão:** DRAFT-0.8
**Estado:** DRAFT / OPEN. Não aprovada, implementada ou validada.
**Autor documental:** Alex Crichton (fronteira de comandos/eventos Tauri, estado offline e persistência). O recorte Swift/SwiftUI e iOS de Donny Wals passa a revisão de extensão nativa iOS.
**Revisão independente designada:** Donny Wals (extensão nativa iOS, WKWebView e Live Activities), por hash desta versão.
**Data:** 2026-11-17.
**Nota de nome do arquivo:** o arquivo mantém o nome histórico `SPEC-016-ios-swift.md`. Swift/SwiftUI **deixa de ser alvo de interface** pela ADR-068; esta SPEC passa a especificar o cliente web offline e a fronteira de **comandos e eventos tipados do Tauri** entre a UI web e o núcleo Rust no processo.

## 1. Objetivo e escopo

Esta SPEC define o comportamento documental do cliente GodMesh único (web, empacotado com Tauri 2 em macOS, Windows, Linux, iOS e Android) no que diz respeito a: capacidades e permissões, ciclo de vida, execução em segundo plano, dados locais protegidos, conectividade degradada, Guardian/SOS, energia, Live Presence e — como núcleo desta versão — a **fronteira de comandos e eventos tipados do Tauri** entre a UI web e o núcleo Rust que roda no mesmo processo.

O cliente informa o que o sistema operacional e o dispositivo permitem no momento, solicita apenas permissões necessárias a uma ação iniciada pelo usuário e apresenta estado local, estado remoto e incerteza separadamente. A disponibilidade de uma API, permissão ou plugin não garante execução contínua, transporte de rede ou entrega.

**Fora do escopo:** decisão de protocolo e sync durável, política global de autorização/retenção, contratos de SOS e ACK humano, implementação, escolha de esquema de banco, escolha de biblioteca de estado da UI, versões de engine, limiares energéticos e metas de desempenho. Esses temas permanecem dependências explícitas.

**Substituição de fronteira:** a fronteira anterior era FFI Rust↔Swift/Kotlin. Pela ADR-068 ela **não existe mais como caminho de produto**: a UI web e o núcleo Rust conversam por **comandos e eventos tipados do Tauri**, dentro do processo. Não há ponte manual, não há ABI de aplicação, não há binding de aplicação.

## 2. Requisitos rastreados

| ID | Cobertura pretendida nesta SPEC | Origem |
|---|---|---|
| RF-029 | Uso de sensores com origem, frescor e incerteza; indisponibilidade não inventa medida/direção | `docs/00-produto/requisitos.md` § requisitos Guardian, RF-029; `docs/01-arquitetura/guardian-localizacao-trajetoria.md` |
| RF-031 | Estado de rede observado com fonte e evidência; rede satelital não implica rota GodMesh utilizável | `docs/00-produto/requisitos.md` § requisitos Guardian/contingência; ADR-045 |
| RF-032 | Orientação manual quando o SO não permite automação; nenhum relato de alteração sem confirmação | `docs/00-produto/requisitos.md`; contrato `docs/07-contratos/conectividade-contingencia.md` |
| RF-033 | Redução de carga discricionária GodMesh e preservação de tarefas prioritárias sem controle de apps externos | `docs/00-produto/requisitos.md`; ADR-047 |
| RF-034 | Aplicativo utilizável offline, modos simultâneos e permissões/capacidades independentes | `docs/00-produto/requisitos.md`; `docs/01-arquitetura/aplicativo-operacao-normal.md` |
| RF-042 | Live Presence elegível com estados verdadeiros, privacidade em tela bloqueada e fallback | `docs/00-produto/requisitos.md`; ADR-052; `docs/01-arquitetura/live-presence.md` |
| RF-043 | Prioridade/energia adaptativa sem controlar aplicativos de terceiros | `docs/00-produto/requisitos.md`; `docs/01-arquitetura/aplicativo-operacao-normal.md` |
| RNF-016 | A projeção só informa fase/estado sustentado por evidência correspondente | `docs/00-produto/requisitos.md` § critérios não funcionais |

O registro da matriz propõe aceites comportamentais, mas mantém os casos como `NOT_ASSIGNED` e observação como `NOT_EXECUTED`. Esta SPEC não cria IDs de testes. A associação de RF-029 a ADR individual e os subescopos e contratos sem vínculo semântico seguem pendentes de confirmação no índice.

## 3. Autoridade, atores e fronteiras

- **Pessoa usuária:** inicia uma atividade, concede ou revoga permissões, escolhe audiência quando previsto e pode interromper o compartilhamento.
- **SO (iOS, Android, macOS, Windows, Linux):** controla autorizações, acesso a sensores, estado de execução, áudio de fundo e elegibilidade de superfícies do sistema. O app não contorna essas decisões.
- **UI web (uma só):** renderiza estado e capacidades; não conhece plataforma nem engine; não lê armazenamento protegido, não fala com o SO e não cria estado de domínio.
- **Shell Tauri 2:** hospeda a UI na engine do sistema, serializa a fronteira, aplica permissões de *capabilities* do Tauri e entrega comandos/eventos. É a única passagem entre UI e núcleo.
- **Núcleo Rust (no processo):** mantém domínio, estado local, outbox/inbox, sync e mídia conforme contratos próprios. É a autoridade do estado local; a UI nunca é autoridade.
- **Plugins nativos do Tauri / extensão iOS:** única superfície nativa; executam o que a web API não alcança (BLE, localização em background, áudio/PTT em background, push, Live Activities/Dynamic Island, NFC/biometria, atalhos, discador).
- **Serviços remotos:** emitem evidências autenticadas conforme GMP/contratos. O cliente não converte estado local em confirmação remota nem ACK técnico em atendimento humano.

Autoridade sobre permissão do dispositivo pertence ao sistema e à pessoa. Autoridade de tenant, consentimento remoto, retenção e revogação global depende dos contratos de identidade, segurança, domínio e sincronização. A leitura de dado local não prova que a autorização remota segue vigente.

## 4. Contrato de capacidade e permissões

Para cada capacidade que a jornada solicitar, o cliente deve representar separadamente suporte conhecido, autorização, disponibilidade operacional e limitação de background. A enumeração comum de estados (`ready`, `unavailable`, `permission_required`, `hardware_unsupported`, `background_limited`) é candidata da arquitetura DICE; enum final e mapping para domínio devem ser acordados com `SPEC-003` e com a versão nova de `SPEC-014` (ainda pendente, ver §14).

1. Ao iniciar ou retomar o app, consultar o estado de autorização e capacidade sem pedir permissões preventivamente.
2. Solicitar uma permissão somente quando a pessoa inicia uma ação que dela depende e após explicar a função afetada.
3. Se a pessoa negar, revogar ou restringir a permissão, marcar a capacidade dependente como indisponível/limitada e manter funções independentes utilizáveis.
4. Após retorno do app ao foreground, reavaliar estados que podem ter mudado em Ajustes/Configurações. Não inferir concessão pela mera abertura da tela de configurações.
5. Toda medição de localização leva origem/combinação, instante e qualidade/incerteza segundo o contrato Guardian. Medição ausente, atrasada ou sem autorização não vira coordenada atual.
6. Descoberta de peer ou visibilidade de rede não autoriza conexão, associação ou compartilhamento.
7. Capacidade declarada é por `(engine, versão observada, plugin presente)`. Ausência de declaração resulta em `unknown`, nunca em suporte presumido.

Fontes de plataforma sobre localização em background e suspensão de app seguem válidas por analogia, mas **não** foram reconferidas para o arranjo Tauri + WebView desta versão; ver §16 e §13.

## 5. Ciclo de vida e estados

O modelo abaixo é uma proposta da SPEC para o cliente, não um enum normativo global. Cada capacidade mantém seu próprio estado; atividades podem coexistir.

| Estado | Significado observável |
|---|---|
| `unknown` | app ainda não obteve evidência suficiente |
| `unsupported` | alvo/engine/dispositivo não expõe a capacidade necessária |
| `permission_required` | função bloqueada até ação de autorização |
| `ready` | capacidade e pré-condições atuais foram observadas |
| `background_limited` | execução ou atualização depende de retorno/limite do SO |
| `stale` | última evidência passou do prazo/condição definida pelo contrato; prazo ainda OPEN |
| `unavailable` | condição observada impede a ação no momento (inclui plugin ausente) |
| `ending` / `ended` | encerramento solicitado / observado conforme evidência disponível |
| `failed` | falha identificada com causa e camada; sem causa distinguível permanece `unknown` |

Eventos: iniciar, ir ao background, suspensão/interrupção, retomada, mudança de permissão, bloqueio/desbloqueio do dispositivo, alteração de rota, expiração/revogação de atividade, descarte/recarga da WebView e encerramento. O app revalida a autorização e o estado antes de iniciar coleta ou replay após retorno. Processo encerrado pelo SO não equivale a callback concluído, atividade encerrada ou dado remoto atualizado.

**Ciclo de vida do WebView é evento de superfície, não de domínio.** Suspensão de página, congelamento, descarte ou recarga da engine descartam memória da UI e podem interromper a assinatura de eventos. Isso não encerra atividade no núcleo, não cancela operação durável e não perde outbox. Após recarga, a UI reconstrói estado exclusivamente por comandos/eventos do núcleo; nenhum estado é inferido do DOM anterior.

## 6. Dados protegidos e fronteira de comandos/eventos Tauri

### 6.1 O que a fronteira é

A UI web não acessa armazenamento protegido, chaves, rede de domínio, sensores de segundo plano ou processos do SO. Tudo passa por comandos e eventos tipados do Tauri, no mesmo processo:

- **Comando (UI → núcleo):** requisição nomeada, com payload tipado, resultado tipado e erro tipado. Toda mutação de domínio é um comando.
- **Evento (núcleo → UI):** notificação tipada de mudança de estado ou de progresso, com identificador de correlação. Evento não é resposta de comando nem prova de entrega remota.
- **Capabilities do Tauri:** permissões de shell que limitam quais comandos/plugins a UI pode invocar. Permissão de shell não substitui permissão do SO nem autorização de domínio; as três são independentes.

O contrato tipado deve declarar, por comando e por evento: nome, versão, payload, erros possíveis, idempotência, se é mutação, ordem esperada, se pode ser reenviado após recarga da UI e o que acontece se a UI desaparecer no meio. **Formato de serialização, nomenclatura final, versionamento e geração de tipos são OPEN** — nenhum framework de estado de UI é escolhido aqui.

### 6.2 Contratos preservados

O que estava garantido na fronteira FFI continua garantido na fronteira de comandos/eventos, com a mesma semântica:

1. **Ownership e duração:** o núcleo é dono do estado; a UI recebe cópia serializada e nunca referência viva. Nenhum dado sobrevive na UI depois de recarga da engine, e o núcleo não depende de memória da UI.
2. **Cancelamento:** distinção explícita entre cancelamento **solicitado**, **observado pelo núcleo** e **concluído**; resultado após cancelamento é tipado (`cancelled`, `completed_before_cancel`, `unknown`), nunca duplo-sucesso.
3. **Concorrência e ordem:** cada comando declara sua política (serializado por escopo, idempotente, ou rejeitado se concorrente). Eventos carregam identificador de correlação e sequência lógica para a UI descartar duplicata e evento atrasado.
4. **Erro:** erro tipado com causa, camada e se é transitório; ausência de resposta não é sucesso; timeout permanece `unknown` até consulta/reconciliação.
5. **Shutdown:** encerramento do app não promete conclusão de operação em voo; o que existe é o estado durável e o resultado reconciliado na próxima execução.
6. **Durabilidade:** gravação confirmada significa apenas persistência local; estado em memória é marcado como não durável e nunca é apresentado como durável.

### 6.3 Dados locais protegidos

O app distingue dado persistido localmente, mudança em memória, item pendente, envio tentado, ACK técnico e confirmação humana. Gravação local bem-sucedida não significa envio, commit remoto ou sincronização completa. Falha de escrita não pode ser apresentada como persistência durável.

O armazenamento do núcleo deve aplicar classe de proteção e política de chave definidas por decisão aprovada, por classe de dado. O estado de bloqueio/desbloqueio e a proteção de arquivo por SO continuam relevantes; **nenhuma classe é selecionada nesta versão**. Quando arquivo/chave estiver indisponível, a UI informa que o dado não está acessível no momento, sem revelar conteúdo em cache de apresentação, e sem descartar fila ou declarar perda sem política/evidência. Após retomada, sincronização e replay obedecem `SPEC-014` (versão nova pendente) e `SPEC-004`: revalidar autorização, retenção e cursores; o cliente não cria mecanismo local de replay concorrente.

### 6.4 O que muda e o que NÃO muda

**Muda:**

- Não existe mais ponte manual Rust↔Swift↔Kotlin, nem ABI/binding de aplicação, nem duplicação de regra de negócio no cliente nativo. Regra de domínio existe **uma vez**, no núcleo Rust.
- A superfície nativa deixa de ser “código de app”; passa a ser plugin Tauri ou extensão iOS, com contrato tipado, dono e teste próprios.
- A UI não é mais um adaptador de plataforma: ela consome capacidade declarada. Nenhuma tela importa código nativo por plataforma.
- O custo de manter três implementações de adaptador vira custo de manter plugins por alvo, com matriz por engine (SPEC-015).

**NÃO muda:**

- **Predicados de grant offline da ADR-059:** operações enumeradas, negações explícitas, exigência de `epoch` comparável, frescor dentro do limite do perfil, ausência de revogação conhecida e anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. A fronteira nova não afrouxa nada disso.
- **Retenção da ADR-065:** prazo por classe e por cópia, política obrigatória, descarte verificado e registrado, fail-closed onde falta política. Nem o shell nem a UI criam exceção.
- **Taxonomia de ACK (ADR-058):** `ack_class` recebido é exibido como recebido, sem promoção por tempo, sucesso de camada inferior ou ausência de erro.
- **Confidencialidade (ADR-060 e emenda):** `confidentiality` por canal e por perna, herança obrigatória, proibição de rótulo E2EE sem prova.
- **Fronteira transacional (ADR-061):** estado + outbox na mesma transação; replay não reemite efeito externo.
- **Compatibilidade e atualidade (ADR-056/ADR-063):** N/N-1 por perfil e piso de `generation` verificado online.

## 7. Rede, conectividade e degradação

O app informa conectividade observada por serviço, rota e frescor. Deve distinguir rede local, internet observada, rota candidata e envio/ACK. A classificação e fonte de transporte seguem `SPEC-003` e `SPEC-007`; esta SPEC não estabelece enum/wire próprio.

- Rota celular, Wi-Fi de terminal satelital ou indicação de sistema não comprova acesso do GodMesh a serviço satelital nem entrega.
- Se uma troca de operadora, plano ou configuração externa não estiver disponível a API oficial, orientar a pessoa a agir manualmente; resultado permanece desconhecido até confirmação observável.
- Em rede restrita, reduzir primeiro tarefas discricionárias do próprio GodMesh, mantendo estado/encaminhamento prioritário possível e transparente. A ordem conceitual P0–P3 do APP-ARC-001 é proposta; prioridade concreta, empate, capacidade e orçamento são OPEN.
- Ausência de rota, timeout ou resposta parcial preserva pendência e estado incerto. Não marcar mensagem, SOS ou posição como entregue/atual.
- Na recuperação, validar autorização e expiração antes do replay comum e apresentar a transição apenas quando houver evidência da camada correspondente.
- A engine do WebView não é fonte de verdade de conectividade: sua API de rede pode reportar diferente do SO e diferente do serviço. Capacidade de rede é declarada, não deduzida de `navigator`.

## 8. Guardian, SOS e privacidade

Guardian só coleta dentro de sessão voluntária, audiência, propósito e validade autorizados. Revogação local impede novas leituras autorizadas; enquanto offline, o app não pode afirmar que servidor ou cópias remotas receberam a revogação. Atraso de check-in é atraso, não SOS automático (ADR-042).

SOS segue `SPEC-006` e contratos próprios. O cliente pode conservar intenção local e tentativa quando permitido, mas não contorna autorização do SO, não promete transmissão e não afirma que atendimento humano começou. Posição exata fica restrita à audiência autorizada conforme ADR-041 e política de localização; não vai à projeção comum de tela bloqueada.

Mínimos de privacidade da interface:

- não mostrar coordenadas precisas, credenciais, SSID/BSSID privados, identidade civil ou histórico de acesso em tela bloqueada;
- qualquer resumo Live Presence usa conteúdo redigido mínimo e deep link autenticado;
- proteger estado stale, desconhecido, local, técnico e humano sem colapsar estados;
- minimizar registros de capacidade e transição; não armazenar identificadores de rede desnecessários;
- preferências de privacidade podem reduzir exposição, mas não convertem o app em agente sempre ativo;
- a UI não escreve log em console de WebView; evidência vem do núcleo, sanitizada.

Política detalhada de classes de dado, retenção, destruição de chave, backup e revogação continua sob `SPEC-020`, `SPEC-002`, `SPEC-005` e a versão nova de `SPEC-014`.

## 9. Live Presence

Live Presence continua opcional por atividade em andamento e por elegibilidade do alvo. Não criar atividade persistente para mera presença “online”. Antes de iniciar, o cliente consulta elegibilidade/autorizações disponíveis e preserva fallback no app. Uma atividade desativada, inelegível ou descartada não altera o estado de domínio.

Pela ADR-068, a projeção exige **código nativo por alvo**, sempre atrás de plugin ou extensão com contrato tipado:

- **Live Activities e Dynamic Island (iOS):** extensão nativa iOS (ActivityKit/WidgetKit), conteinerizada no app Tauri; recebe do núcleo apenas o payload de projeção; não tem regra de domínio própria.
- **Live Updates/notificação persistente (Android):** código nativo Android em plugin Tauri, com o mesmo regime de contrato e teste.
- **demais alvos:** notificação do SO por plugin, ou fallback dentro do app. Não simular Live Activity em tela web.

Dados da projeção: identificador opaco, tipo, estado, prioridade, resumo redigido, momento da atualização, validade quando definida e referência ao evento de origem. Não inclui segredo, coordenada precisa ou token de acesso. `staleDate` só é usado quando houver regra temporal aprovada; prazos continuam OPEN.

| Entrada/evento | Exibição esperada |
|---|---|
| Evento autenticado de SOS ativo | Prioridade visual; texto informa estado técnico exato, sem alegar socorro/claim humano não comprovado |
| Guardian válido | Resumo, check-in/frescor conforme evidência; sem posição precisa |
| PTT ativo | Estado da comunicação conforme evento; sem alegar áudio entregue quando há somente transmissão local |
| Sem evento recente / stale date vencida | Marcar estado desatualizado; não repetir como atual |
| Permissão/suporte inelegível, extensão ausente ou atividade encerrada pelo SO | Fallback no app/notificação permitida; preservar estados das demais atividades |
| Evento duplicado ou atrasado | Deduplicação/ordem conforme contrato GMP e sync; não ressuscitar encerramento sem regra aprovada |

O contrato GodMesh de precedência concorrente e eventos atrasados continua OPEN. `priority` interno do GodMesh não é promessa de controle da ordem visual do sistema.

## 10. Critérios de aceite documental

Estes critérios definem resultado que o futuro teste deverá observar. Eles não são resultados de execução.

1. **Permissão negada/revogada:** com duas funções, uma dependente e outra independente, negar/revogar a permissão da primeira marca apenas essa capacidade como limitada/indisponível; a segunda permanece utilizável; novo acesso não ocorre sem reautorização.
2. **Retorno de background:** suspender e retomar durante atividade de localização não causa estado atual sem nova evidência; o app mostra valor anterior com idade/estado correspondente ou desconhecido, reavalia a autorização e só então processa nova leitura.
3. **Bloqueio e dados protegidos (parcial/OPEN):** falta definir classes de dados e política de acesso; portanto, não há ainda resultado esperado completo para leitura/escrita/replay em cada estado de bloqueio. Depois da decisão aprovada, testar cada célula da matriz; confirmar que o comportamento segue a classe e que a UI não revela payload inacessível nem afirma perda. Este critério não pode ser considerado atendido enquanto a decisão sobre classes de dados e proteção no §13 estiver OPEN.
4. **Offline e persistência (parcial/OPEN):** abrir sem rede mostra estado offline e dados locais autorizados; envio pendente não vira entregue; erro de armazenamento não vira confirmação de persistência. O contrato de comandos/eventos (§6) precisa estar versionado com erros, cancelamento e idempotência declarados; este critério não prova que a fronteira está completa.
5. **Revogação offline:** revogar Guardian desconectado interrompe novas leituras locais e marca entrega remota como não confirmada; reconexão só altera a situação remota após evidência definida pelos contratos de sync/autorização.
6. **Rota:** indicação de rede ou satélite sem prova de transporte do serviço não marca rota GodMesh utilizável; falha de rota não é inferida de falta de telemetria.
7. **Orientação manual:** quando ação externa não for permitida por API, app mostra instrução adequada e status sugerido/desconhecido; não afirma abertura de configurações ou mudança de operadora sem observação/ação confirmada.
8. **Priorização:** sob limitação de bateria/rede, reduzir apenas tarefas próprias não essenciais conforme política aprovada e manter estado de serviços por capacidade; não encerrar serviço do SO nem inferir que SOS será entregue.
9. **Live Presence stale:** ao atingir a stale date definida, conteúdo deixa de ser tratado como atual; fallback permanece e eventos recebidos preservam fase/destino comprovados.
10. **Coexistência:** PTT + Guardian + SOS mantêm estado individual; SOS pode receber destaque sem apagar atividade anterior; claims e resolução humana exigem eventos de autoridade próprios.
11. **Dados bloqueados:** tela bloqueada não revela os campos proibidos na seção 8, inclusive em estado stale/offline.
12. **Fronteira sem duplicação:** nenhuma regra de domínio (grant, retenção, ACK, confidencialidade) existe fora do núcleo Rust; a UI apenas exibe estado recebido. Tela que importe código nativo por plataforma, consulte engine ou reimplemente regra reprova a revisão.
13. **Recarga da WebView:** descartar/recarregar a engine durante operação pendente reconstrói a UI a partir do núcleo, sem perda de outbox e sem promoção de estado.
14. **Cross-platform:** execução desses critérios em outras plataformas não é atribuída a esta SPEC; contratos por engine estão em SPEC-015. Servidor Linux/macOS não se aplica ao cliente. Não aplicabilidade não exclui interação via APIs/serviços que o cliente consome.

Casos de teste formais: `NOT_ASSIGNED`. Execução: `NOT_EXECUTED`. Nenhum limiar numérico foi definido sem política ou medição.

## 11. Casos planejados sem IDs

1. Autorizar Guardian em foreground, iniciar atividade e ir ao background; registrar autorização, estado da capacidade, origem/instante da localização, evento enviado e apresentação.
2. Negar e depois revogar autorização enquanto app roda; confirmar transição da capacidade, ausência de nova leitura e continuidade de funções independentes.
3. Suspender/interromper app com atualização pendente, retomar com e sem rede, verificar stale, fila, idempotência e distinção entre persistido, tentado e ACK.
4. Bloquear aparelho antes do primeiro desbloqueio, após desbloqueio e após bloquear novamente; executar leitura/escrita/replay por classe somente depois que política de proteção for decidida.
5. Perder rota controlada mantendo permissão, restaurar rota e verificar que o app não confundiu falha de transporte com revogação, sem serviço observador inexistente.
6. Mostrar Live Activity elegível e desativada; atualizar, expirar, receber evento atrasado e coexistir com SOS; confirmar stale e fallback, inclusive com extensão ausente.
7. Aplicar limite de energia e rede de fixture autorizada; verificar apenas classes de tarefa que a política final aprovar, sem medir ou alegar economia nesta SPEC.
8. Forçar falha de persistência e cancelamento de comando; observar diferença entre erro, cancelamento solicitado, cancelamento efetivo e operação durável, quando o contrato do §6 estiver versionado.
9. Recarregar/descartar a WebView com comando em voo e evento pendente; verificar resultado tipado, ausência de duplicação e reconstrução de estado.

Cada caso requer build, versão/alvo, engine, fixture, permissões, fonte de evidência e ambiente definidos por responsáveis antes da execução. Nenhum foi executado; IDs só serão atribuídos pelo índice de Ada após definição formal.

## 12. Compatibilidade e aplicabilidade

| Alvo | Aplicabilidade |
|---|---|
| WKWebView iOS | Escopo principal da projeção Live Presence; versão mínima, aparelhos, entitlements e extensão nativa OPEN. |
| WKWebView macOS | Aplicável aos mesmos contratos de estado; capacidades nativas só por plugin. |
| WebView2 Windows / WebKitGTK Linux | Aplicável aos contratos de estado; capacidades e mídia por plugin; versões OPEN. |
| Android WebView | Aplicável; Live Updates, BLE, background e áudio exigem código nativo em plugin; fragmentação por OEM OPEN. |
| Servidor Linux | Não hospeda cliente. Continua sujeito a SPECs de serviço e integração no escopo geral. |

As afirmações sobre APIs de plataforma citadas nas versões anteriores foram conferidas em documentação oficial em 2026-11-16 **para o arranjo nativo anterior**. Elas não foram reconferidas para Tauri 2 + WebView + plugins nesta versão; a reconferência é pré-requisito de aprovação (§15).

## 13. Decisões OPEN

| Questão | Responsável proposto | Alternativas e consequência | Fechamento necessário |
|---|---|---|---|
| Classes de dados e proteção no bloqueio/primeiro desbloqueio | Kim, Bruce, Michael; produto decide finalidade | Maior proteção pode impedir fila/replay após bloqueio; acesso mais amplo amplia dados disponíveis em background. Não escolher nível nesta SPEC. | Matriz dado × estado × read/write/replay aprovada e compatível com retenção/backup; fontes de plataforma reconferidas. |
| Contrato de comandos/eventos Tauri: nomes, payloads, versão, erros, idempotência, cancelamento, ordem e geração de tipos | Alex, Graydon; revisão Donny (iOS) e Chet (Android) | Comandos grossos reduzem superfície e acoplam; comandos finos multiplicam contratos e chamadas. Serialização e codegen não decididos. | Artefato versionado e revisado por hash; casos de cancelamento, timeout e recarga da UI cobertos. |
| Estado do WebView versus estado do núcleo após recarga/descarte | Alex, Lea | Reconstruir por evento pode perder transição; reconsultar estado custa chamadas. Nenhuma opção escolhida. | Regra por comando/evento: quem reconstrói, com qual consulta inicial, e como duplicatas são descartadas. |
| Sync e revogação offline | Martin Kleppmann, Kim; dependências Michael/Graydon | Revalidar, rejeitar ou retirar item da fila conforme autoridade; sem regra pode vazar ou descartar operação. | Política e transições reconciliadas em SPEC-004/SPEC-002 e mapeadas nesta SPEC. |
| Frescor, expiração e cadência de Guardian/Live Presence | Margaret, Nancy, Lea e responsáveis de domínio | Prazo/cadência altera energia e risco de conteúdo obsoleto; não fixar número sem evidência. | Política aprovada por atividade e destinatário; contrato inclui origem, tempo e estado stale. |
| Eventos concorrentes, replay e precedência SOS/PTT/Guardian | Nancy, Martin Kleppmann, Graydon; produto | Ordenação local/servidor e prioridade de UI podem divergir. | Contrato define fonte/ordem e casos de evento atrasado, duplicado, revogado e resolvido; validar sem colapsar ACK técnico/humano. |
| Transporte e estado de rota | Vint, Graydon, Brendan | Rota observada pode não transportar serviço; status desconhecido é mais honesto que inferência. | Fontes/versões e método de observação registrados por campo; sem depender de telemetria perturbada. |
| Prioridade e energia adaptativa por engine | Margaret, Lea, Brendan, Alex e Donny | Perfil estático vs adaptativo; custo por engine sem medição é desconhecido. | Política por serviço e método futuro reproduzível aprovados; limiares continuam OPEN até dados autorizados. |
| Extensão nativa iOS (Live Activities/Dynamic Island): empacotamento, contrato, ciclo de vida e distribuição | Donny, Alex | Extensão separada adiciona alvo de build e superfície de contrato; sem ela RF-042 não é entregável no iOS. | Contrato tipado, dono, teste e versões/entitlements documentados. |
| Versões mínimas de SO/engine e matriz de compatibilidade | Donny, Chet, Alan | Adotar só engines/APIs compatíveis com alvos decididos; sem downgrade silencioso. | Confirmar versões e documentação oficial para Tauri 2, engines e cada plugin antes de aprovar. |

## 14. Dependências e divergências

- **SPEC-014 (cliente offline e fronteira):** ainda em versão que descreve **FFI Rust–Swift** e nomeia binding. Pela ADR-068 exige nova versão; a substituição por comandos/eventos Tauri está **fora** do escopo de edição desta SPEC e permanece pendente com seu autor. Enquanto isso, esta SPEC é a descrição vigente da fronteira para o cliente web.
- **SPEC-015 (Chet):** matriz por engine de WebView e capacidade declarada; é a contraparte desta SPEC em capacidade e acessibilidade.
- **SPEC-006, Nancy:** autorização, estados e ACK de SOS; não declarar atendimento humano por estado de UI.
- **SPEC-004, Martin Kleppmann:** outbox/inbox, cursores, replay, idempotência e revogação em reconexão.
- **SPEC-002 e SPEC-020, Kim/Bruce:** identidade, autorização, privacidade, chaves e retenção.
- **SPEC-003, Graydon; SPEC-007, Radia/Vint:** eventos e transporte; estado de rota é evidência separada de entrega.
- **SPEC-012, Margaret; SPEC-013, Lea:** jornadas, texto, acessibilidade, fallback e prioridade visual. `SPEC-013` também precisa de nova versão pela ADR-068.
- **SPEC-018, Brendan:** sinais, evidência e metodologia; revisão INT-ADA-001 não prova que observadores existem.
- **`docs/01-arquitetura/live-presence.md`:** ainda nomeia Swift/ActivityKit/SwiftUI como mecanismo de projeção iOS; atualização pendente com seu dono, fora do escopo desta edição.

Não encontrei conflito direto entre as fontes normativas listadas e as ressalvas integradas. Direções de plataforma e produto são aprovadas em ADRs, porém detalhes de contrato, ciclo de vida, compatibilidade, prioridades de eventos, retenção e limites seguem OPEN. O registro de uma ressalva não a fecha.

## 15. Critério de fechamento da SPEC

Esta SPEC só pode avançar para revisão/aprovação quando: (a) IDs e subescopos forem confirmados no índice; (b) o contrato de comandos/eventos Tauri do §6 estiver versionado e revisado por hash, com erros, cancelamento, ordem e idempotência declarados, ou sua permanência OPEN for justificada como não bloqueadora; (c) alvos, engines e versões (Tauri 2, WebView, plugins, extensão iOS) estiverem documentados com fonte e data; (d) políticas de Guardian, SOS, energia, privacidade e revogação forem conciliadas; (e) casos por requisito e por engine tiverem pré-condição, estímulo, resultado, evidência prevista, dependência e ID central atribuído, incluindo a matriz de proteção para considerar completo o caso 3; (f) a nova versão de SPEC-014 existir e não contradizer a fronteira aqui descrita. Mesmo atendidos, esses itens não significam implementação, build, teste ou validação.

## 16. Fontes

### Documentação do projeto

- `docs/README.md` §§ regras de prevalência e plataformas. Arquitetura de referência e ADRs prevalecem.
- `docs/01-arquitetura/arquitetura-referencia.md` §§ arquitetura de cliente e princípios de capacidade (`555bad39b874b7ba5c8c4cdb87ff5d7f20194afb7fc338a9a93029b829d7cb89`, registrado na versão anterior).
- `docs/06-planejamento/decisoes.md` ADR-041, 042, 045, 047, 052, 056–066, **ADR-067 e ADR-068**; ADR-055 SUPERADA (`3351c5e5507b05fbd5acf0345cd4ac5581f4ac7949fe3866e6546b3b38ef0e93`).
- `docs/00-produto/requisitos.md` RF-029, RF-031–034, RF-042–043 e RNF-016 (`327e259927dd99e3912aaf59e2d59e5b33afb636cf733b177a7a9c30c0b6ef82`).
- `docs/01-arquitetura/aplicativo-operacao-normal.md` §§ 3–8 (`019fa7c5556039bf94769f4b958fae1f633800a8fce82915c5a117c8451e5081`).
- `docs/01-arquitetura/live-presence.md` §§ 1–5 (`1d3c38919656b049fd607c0ef1558c0c0a7e61dc5d7ee2f24446fa68af5e48d5`); `docs/07-contratos/live-presence.md`.
- `docs/09-equipe/registro-gaps.md` (gaps abertos, `0d5c7f0237f6234c3cd336892a63d50f34e5de6878f0194a89156fa6a594b631`).
- `docs/06-planejamento/pendencias.md` e `docs/06-planejamento/matriz-rastreabilidade.md` §§ compatibilidade, lifecycle, armazenamento e critérios de validação (hashes registrados na versão anterior, não reconferidos).

### Documentação oficial consultada

- ADR-068 registra a consulta a `v2.tauri.app` (início, pré-requisitos, plugins barcode, biometric, NFC, geolocation, notifications, deep linking) em 17/11/2026. **Esta SPEC não reconferiu** a documentação de Tauri 2 nem as páginas Apple de Core Location, `FileProtectionType` e ActivityKit citadas nas versões anteriores; a reconferência é pré-requisito do §15. Nenhuma versão de engine, plugin ou SO é afirmada aqui.

## Mudanças desta versão — ADR-068 (DRAFT-0.5 → DRAFT-0.6)

**Decisão aplicada:** ADR-068, aprovada pelo usuário em 17/11/2026, supera a ADR-055. Cliente único web com Tauri 2; núcleo Rust no processo exposto por **comandos/eventos tipados**; plugins nativos e extensão iOS como única superfície nativa.

1. **Fronteira substituída:** FFI Rust↔Swift/Kotlin → comandos e eventos tipados do Tauri (§6). Não há ponte manual, não há ABI de aplicação, não há duplicação de regra de negócio no cliente.
2. **Contratos mantidos:** estados e transições (§5), cancelamento solicitado/observado/concluído, lifecycle e retomada, persistência protegida e distinção durável × memória, erro tipado, idempotência e replay, acessibilidade, tela bloqueada e Live Presence (§9).
3. **Declarado o que muda:** uma UI, um núcleo, adaptadores nativos viram plugins/extensão; custo de manter três implementações vira custo de manter plugins por alvo; capacidade passa a ser declarada por engine.
4. **Declarado o que NÃO muda:** predicados de grant offline da **ADR-059** e retenção da **ADR-065** ficam intactos, junto de ACK (ADR-058), confidencialidade (ADR-060 + emenda), transação/outbox (ADR-061) e compatibilidade/atualidade (ADR-056/ADR-063).
5. **Live Presence:** Live Activities/Dynamic Island exigem **extensão nativa iOS**; Live Updates Android exigem código nativo em plugin; **push exige plugin Tauri**; sem plugin a capacidade é `unavailable` e há fallback no app.
6. **Acrescentado:** casos por engine (divergência de API, áudio em segundo plano, permissão negada, plugin falhando, extensão ausente), caso de recarga da WebView e **caso negativo de tela importando código nativo por plataforma** (§10 critério 12 e §17 `S016-ENG-N2`).

**Impacto por requisito:**

- **RF-034** (offline, modos simultâneos): passa a depender de uma UI única + núcleo no processo + plugins; o estado local deixa de ser duplicado no cliente nativo. Continua não coberto integralmente aqui — jornada é de SPEC-012/013.
- **RF-043** (energia adaptativa): o custo passa a ser medido por engine e por plugin; áudio/PTT em segundo plano depende de plugin nativo. Metas seguem OPEN.
- **RF-042** (Live Presence): projeção iOS/Android deixa de ser alcançável por UI nativa de app; exige extensão iOS ou plugin Android com contrato tipado. Nenhum requisito é rebaixado.

**Nota de escopo:** SwiftUI e Kotlin **deixam de ser alvo**; o nome do arquivo é histórico. Nenhum framework de estado de UI é escolhido nesta SPEC.

## Mudanças anteriores — ADR-056, ADR-059 e ADR-060

**ADR-056 — compatibilidade:** suporte declarado por versão e perfil versionado de cliente, com janela N/N-1 dentro do perfil publicado. Perfis concretos e janela de calendário permanecem OPEN até decisão de produto. Servidor rejeitando versão/feature incompatível deve fazê-lo antes de qualquer efeito e sem downgrade silencioso; a UI informa incompatibilidade e requisito de versão/perfil somente com dados fornecidos pela resposta. Não repetir mutação incompatível; falta de resposta conclusiva é desconhecido.

**ADR-059 — offline:** únicas operações autorizadas: (1) abrir incidente SOS local e enfileirar somente com F1/F2/F3 demonstrados — sem raiz de confiança a operação é negada e nada é enfileirado (ADR-064); (2) check-in de sessão Guardian existente; (3) mensagem a canal já autorizado; (4) observação local de localização; (5) PTT apenas em rede local com grant de floor válido. Negar sempre localização precisa divulgada, mutação federada, criar/alterar grant, mudança de identidade, transmissão RF e transcrição remota. Exigir epoch comparável, frescor dentro do limite do perfil, ausência de revogação conhecida e anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. Frescor numérico desconhecido até medição.

**ADR-060 — confidencialidade:** expor `confidentiality` por canal e por perna; bridge, ASR e gravação que terminem a proteção são marcados `terminated`. Não rotular E2EE sem prova para canal/perna. Tela bloqueada não exibe posição precisa nem segredo.

Revisões citadas são de versões anteriores; revisão independente desta versão/hash continua pendente. Execução permanece `NOT_EXECUTED`.

## Casos por decisão (ADR-056…063)

Casos documentais planejados. ID formal: `NOT_ASSIGNED`. `Estado` em todos: `NOT_EXECUTED`. Estados citados são os desta SPEC §5 (`unknown`, `unsupported`, `permission_required`, `ready`, `background_limited`, `stale`, `unavailable`, `ending`/`ended`, `failed`). Parâmetro OPEN resulta em negar/exibir desconhecido.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S016-056-P1 | ADR-056 | App cujo par N/N-1 não tem janela aprovada; permissões já concedidas | Ação que chama a API | `compatibility=param_not_configured` exibido no detalhe; `profile_version` recebido é mostrado; capacidade permanece `ready` sem declaração de suporte nem de fim de suporte; nenhum valor de janela presumido | P | NOT_EXECUTED |
| S016-056-N1 | ADR-056 | Servidor rejeita versão/feature incompatível antes do efeito | Nova tentativa de mutação incompatível | Mutação não repetida; tela exibe incompatibilidade com versão/perfil requeridos; nenhum downgrade silencioso | N | NOT_EXECUTED |
| S016-056-B1 | ADR-056 | Janela de calendário OPEN | Consulta de suporte/versão | Exibe `window=not_decided`; nenhuma data de fim de suporte | B | NOT_EXECUTED |
| S016-056-F1 | ADR-056 | Timeout de compatibilidade sem resposta conclusiva | Ação em curso | Estado `unknown` exibido; nenhum selo de sucesso; fila local permanece pendente | F | NOT_EXECUTED |
| S016-057-P1 | ADR-057 | Fila/inbox local com tupla canônica e cursor do cliente | Replay após retorno de background | Itens enviados na ordem canônica; cursor preservado com principal/escopo/filtros/perfil/retenção | P | NOT_EXECUTED |
| S016-057-N1 | ADR-057 | Cursor sem `partition` ou de epoch incomparável | Replay/consulta | Rejeição com erro de cursor; nada traduzido; lista não é exibida como atual | N | NOT_EXECUTED |
| S016-057-F1 | ADR-057 | `authority_epoch` mudou durante suspensão do app | Replay do cursor antigo | Falha fechada; itens marcados `unknown`; nenhuma lacuna saltada | F | NOT_EXECUTED |
| S016-057-B1 | ADR-057 | Dois eventos com mesma `seq` em partições distintas | Exibição da timeline | Ambos visíveis com posição própria; nenhuma duplicata nem colapso | B | NOT_EXECUTED |
| S016-058-P1 | ADR-058 | Gravação local confirmada de check-in ou mensagem | Envio durante partição | Classe `persisted_local` exibida como pendente; nenhum campo de entrega preenchido | P | NOT_EXECUTED |
| S016-058-N1 | ADR-058 | Live Activity atualizada localmente, sem recibo do destino | Tentativa de exibir entrega ou claim humano | `delivered_to_device` não preenchido; `claimed` exige evento humano autorizado; texto permanece técnico e exato | N | NOT_EXECUTED |
| S016-058-F1 | ADR-058 | Envio com confirmação perdida; atividade já exibida | Reconciliação após retorno ao foreground | Resultado `unknown` exibido; nenhuma regressão de estado terminal; nenhuma repostagem por inferência | F | NOT_EXECUTED |
| S016-058-B1 | ADR-058 | Destino sem recibo disponível | Consulta de estado | `confirmation_unavailable`/`unknown` exibido; entrega não fabricada por ausência de erro | B | NOT_EXECUTED |
| S016-059-P1 | ADR-059 | Grant vigente; operação enumerada, com sessão Guardian existente, sem limite de frescor aprovado | Check-in ou observação local offline | `grant=valid` e `authorization=unknown` em dimensões separadas com `freshness_limit=param_not_configured`; estado persistido separado de envio; nada enfileirado e nenhuma entrega afirmada | P | NOT_EXECUTED |
| S016-059-N1 | ADR-059 | Offline | Tentativa de divulgar localização precisa, mutação federada, criar/alterar grant, mudança de identidade ou transcrição remota | Negado com motivo exibido; nada enfileirado; nenhuma exceção presumida para SOS | N | NOT_EXECUTED |
| S016-059-F1 | ADR-059 | `stale`/`unknown` de autoridade, relógio incerto, epoch anterior ou snapshot restaurado com grant válido | Nova operação condicional | Negação com estado desconhecido exibido; anti-rollback bloqueia o grant; operação não persistida | F | NOT_EXECUTED |
| S016-059-B1 | ADR-059 | Limite de frescor OPEN; PTT em rede local sem grant de floor | Operação condicional e tentativa de PTT | Operação negada com `freshness_limit=not_decided`; PTT não inicia | B | NOT_EXECUTED |
| S016-060-P1 | ADR-060 | Canal cuja cadeia inclui bridge, ASR ou gravação | Abertura do canal e do detalhe | `confidentiality=terminated` exibido na API e na UI; derivados locais herdam o valor | P | NOT_EXECUTED |
| S016-060-N1 | ADR-060 | Tela bloqueada; canal terminado | Renderização de Live Activity e notificação | Nenhuma coordenada precisa, credencial, SSID/BSSID ou identidade civil; nenhum rótulo E2EE | N | NOT_EXECUTED |
| S016-060-F1 | ADR-060 | Metadado `confidentiality` ausente na cópia local | Consumo do conteúdo | Tratado como `terminated`; ausência não suprida por cache ou versão anterior | F | NOT_EXECUTED |
| S016-060-B1 | ADR-060 | Arquivo protegido inacessível no bloqueio do aparelho | Leitura de derivado em background | Payload não revelado no resumo de apresentação; estado exibido inacessível, sem alegar perda | B | NOT_EXECUTED |
| S016-061-P1 | ADR-061 | Persistência local e outbox na mesma transação | Mutação offline permitida | Estado e evento commitados juntos; confirmação de gravação antes de exibir pendente; publicação após commit | P | NOT_EXECUTED |
| S016-061-N1 | ADR-061 | Atividade já `ended`; evento duplicado ou atrasado | Replay/deduplicação | Nenhum efeito externo reemitido; encerramento não ressuscitado; estado terminal preservado | N | NOT_EXECUTED |
| S016-061-F1 | ADR-061 | App encerrado pelo SO durante operação em voo | Retomada do app | Apenas estado persistido é reconstruído; nenhuma afirmação de trabalho no intervalo; nenhuma duplicação | F | NOT_EXECUTED |
| S016-061-F2 | ADR-061 | Autoridade `stale`/`unknown` | Mutação forte | Bloqueio com estado explícito; funções independentes preservadas; nenhuma escrita aceita | F | NOT_EXECUTED |
| S016-061-B1 | ADR-061 | Transação local com `outbox` commitado e publicação pendente | Consultar a operação antes do publish | Estado `pendente local` com prova de commit; `materialized` e efeito externo ausentes; leitor não publica nem promove classe de ACK | B | NOT_EXECUTED |
| S016-062-P1 | ADR-062 | Política por classe existente para a coleta, com prazo e gatilho da ADR-065 | Coleta e gravação | Gravação aceita com `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo ou gatilho exibido como aprovado; minimização de registros de capacidade e transição mantida | P | NOT_EXECUTED |
| S016-062-N1 | ADR-062 | Classe sem política aprovada | Tentativa de persistir histórico ou dado correlacionável | Falha fechada com erro explícito; nenhum dado persistido; UI declara indisponibilidade | N | NOT_EXECUTED |
| S016-062-F1 | ADR-062 | Política expirada; cópias incluem arquivo protegido, cache, backup e derivados de projeção | Gatilho de descarte | Descarte por cópia registrado; cópia sem confirmação pendente; nenhuma remoção global alegada | F | NOT_EXECUTED |
| S016-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão | Consulta comum ao dado retido | Dado retido sob prazo próprio; acesso comum não reativado | B | NOT_EXECUTED |
| S016-063-P1 | ADR-063 | Pacote assinado do cliente com SBOM e `generation` igual ou superior ao piso | Instalação limpa/atualização | Verificação aprovada antes de executar; versão, digest e `generation` registrados e exibidos | P | NOT_EXECUTED |
| S016-063-N1 | ADR-063 | Artefato com `generation` inferior ao piso (versão vulnerável) | Tentativa de instalar versão anterior | Rejeitada; versão atual mantida; motivo `generation_below_floor` exibido; nenhuma execução | N | NOT_EXECUTED |
| S016-063-F1 | ADR-063 | Assinatura inválida ou chave revogada na lista verificada | Aplicação do artefato | Rejeição com erro tipado; nenhum fallback; estado `failed` com causa e camada | F | NOT_EXECUTED |
| S016-063-B1 | ADR-063 | Atualização N-1 dentro da janela, mas abaixo do piso | Instalação da atualização | Rejeitada; piso prevalece sobre a janela; motivo e auditoria registrados | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** classes de dado e proteção no bloqueio (matriz §13 OPEN) impedem caso positivo de leitura/escrita/replay por classe — os casos de bloqueio aqui verificam apenas ausência de revelação. Frescor e stale date de Guardian/Live Presence seguem OPEN, então nenhum caso usa prazo temporal aprovado. Versões mínimas de engine/SO seguem OPEN, então não há caso por versão concreta.

## Casos por engine (ADR-068)

Casos documentais planejados, um por classe de alvo. ID formal `NOT_ASSIGNED`; `Estado` em todos `NOT_EXECUTED`. Execuções futuras precisam declarar engine, versão, SO, build e plugin.

| ID local | Alvo | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S016-ENG-P1 | Todas as engines | Contrato de comandos/eventos versionado e capacidade declarada publicada | Abertura do app após recarga da WebView | UI reconstrói estado por comando de consulta inicial + eventos; duplicatas descartadas por correlação; nenhum estado durável inferido do DOM | P | NOT_EXECUTED |
| S016-ENG-N1 | Duas engines distintas | Comando cuja API subjacente diverge entre engines (ex.: mídia, foco, armazenamento) | Uso da função dependente | Divergência declarada como limitação daquela engine; nenhuma tela detecta engine nem troca de caminho; nenhuma promessa de equivalência | N | NOT_EXECUTED |
| S016-ENG-N2 | Qualquer alvo | Revisão da árvore da UI | Varredura por import nativo, `#[cfg]` de plataforma, checagem de *user agent*, uso de API do Tauri fora do módulo de fronteira, ou regra de grant/retenção/ACK reimplementada na UI | **Caso negativo:** nenhuma tela importa código nativo por plataforma e nenhuma regra de domínio vive fora do núcleo; violação reprova a revisão | N | NOT_EXECUTED |
| S016-ENG-N3 | Todas as engines | Permissão negada no meio de operação em curso | Tentativa de continuar a função dependente | Comando retorna erro tipado `permission_required`; UI mantém funções independentes; nada enfileirado por engano | N | NOT_EXECUTED |
| S016-ENG-B1 | Android WebView e WebKitGTK | Suporte a áudio de segundo plano divergente | PTT inicia e app vai ao background | Alvo sem suporte: `background_limited`, áudio não apresentado como garantido; alvo com suporte: estado real; nenhum limiar inventado | B | NOT_EXECUTED |
| S016-ENG-B2 | WKWebView iOS | Extensão Live Activity não instalada ou inelegível | Projeção de atividade elegível | Extensão não inicia; estado de domínio inalterado; fallback no app; nenhuma alegação de Dynamic Island | B | NOT_EXECUTED |
| S016-ENG-B3 | Todas as engines | Push sem plugin Tauri habilitado | Evento destinado a notificação | Capacidade `unavailable`; nenhuma notificação simulada pela UI; estado real e fallback interno | B | NOT_EXECUTED |
| S016-ENG-F1 | Duas engines | Comando em voo e WebView recarregado/descartado | Retomada da UI | Resultado tipado consultado/reconciliado; nenhuma duplicação de mutação; outbox preservado; nenhum estado terminal inventado | F | NOT_EXECUTED |
| S016-ENG-F2 | Todas as engines | Plugin nativo falhando na inicialização | Uso da capacidade correspondente | Erro tipado com causa e camada; capacidade `unavailable`; nenhum caminho alternativo silencioso | F | NOT_EXECUTED |
| S016-ENG-F3 | Duas engines | Áudio interrompido pelo SO (chamada, perda de foco) durante PTT | Retorno do foco de áudio | Transmissão exibida como interrompida/não garantida; nenhum ACK de áudio; retomada só com nova ação | F | NOT_EXECUTED |

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §2. Células citam casos da tabela `Casos por decisão`, `Casos por engine` ou ID novo descrito na Nota; prefixo `S016-R###` agrupa os quatro tipos do mesmo requisito. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. Versões mínimas de engine/SO seguem OPEN, portanto nenhum caso usa versão concreta.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-029 | S016-R029-P | S016-ENG-N3 | S016-R029-B | S016-060-B1 | P: leitura de sensor com origem, frescor e incerteza via plugin. N: indisponibilidade não inventa medida nem direção. B: sensor presente com permissão negada → `permission_required`, nunca `unsupported`. F: revogação suspende só a função dependente com motivo. |
| RF-031 | S016-R031-P | S016-R031-N | S016-R031-B | S016-056-F1 | P: estado de rede com fonte e instante. N: satélite detectado não implica rota GodMesh utilizável. B: IPv6 e IPv4 coexistem com resultados independentes. F: API indisponível deixa `unknown` e orienta ação manual. |
| RF-032 | S016-R032-P | S016-R032-N | S016-R032-B | S016-R032-F | P: orientação manual apresentada quando automação não existe. N: abrir configurações não comprova ativação. B: usuário dispensa a orientação sem efeito. F: permissão negada/API ausente → instrução e estado não verificado. |
| RF-033 | S016-059-P1 | S016-059-N1 | S016-059-B1 | S016-R033-F | F: carga discricionária cede primeiro e comunicação de emergência do SO permanece acessível; orçamento e ordem fina OPEN. |
| RF-034 | S016-ENG-P1 | S016-ENG-N3 | S016-ENG-B3 | S016-061-F1 | P: áreas e modos simultâneos utilizáveis offline com estado local vindo do núcleo. N: capacidade ausente limita só a função dependente. B: plugin ausente mantém fallback declarado. F: encerramento pelo SO durante uso mantém estado conhecido sem alegar continuidade. |
| RF-042 | S016-R042-P | S016-060-N1 | S016-ENG-B2 | S016-061-F2 | P: Live Activity só para atividade em andamento elegível, por extensão/plugin. N: tela bloqueada não expõe posição precisa nem segredo. B: sem Live Activities/Update exigíveis → notificação/fallback (S016-ENG-B3). F: `stale`/`failed` exibidos sem converter falha de superfície em falha do domínio. |
| RF-043 | S016-R043-P | S016-058-N1 | S016-058-B1 | S016-ENG-F3 | P: prioridade preservada com estado real por classe de ACK. B: interrupção de áudio pelo SO exibe transmissão não garantida. Metas de energia OPEN; nenhum caso mede SLO energético. |
| RNF-016 | S016-058-P1 | S016-058-N1 | S016-058-B1 | S016-058-F1 | Projeção não anuncia fase/estado sem evidência; classe exibida é a recebida (ADR-058). |