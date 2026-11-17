# SPEC-013 — Experiência e superfícies por plataforma

**Versão:** DRAFT-0.4 · **Estado:** DRAFT / OPEN · **Autora:** Lea Verou · **Revisão independente designada:** James Bach (pendente desta versão/hash) · **Data:** 2026-11-17

Esta SPEC define comportamento observável de UX, estados acessíveis e Live Presence com as fontes disponíveis. Não declara decisão aprovada, API implantada, implementação, teste ou homologação. A cobertura direta comprovada nesta rodada é RF-042 e RNF-016; jornada operacional geral e as referências a RF-033/RF-034 são dependências, conforme limites abaixo.

## 1. Rastreabilidade e fontes

| Requisito / escopo | Evidência de origem | Cobertura nesta versão |
|---|---|---|
| RF-042 | `docs/00-produto/requisitos.md:76`; `docs/01-arquitetura/live-presence.md` §§1–5; `docs/07-contratos/live-presence.md` | Coberto parcialmente: estados, projeção, elegibilidade, privacidade e cenários propostos; falta decisão de proveniência/frescor e homologação por plataforma |
| RNF-016 | `docs/00-produto/requisitos.md:84`; RF-042; contrato Live Presence | Coberto parcialmente: rótulo não pode exceder evidência; falta ligação por rótulo/fase a eventos de domínio definidos |
| RF-034 (dependência) | `docs/00-produto/requisitos.md:68`; `docs/01-arquitetura/aplicativo-operacao-normal.md` §§1–6 | Não reivindicado como aceite próprio: jornada e estados gerais dependem de produto (Margaret) e clientes (Alex/Donny/Chet) |
| RF-033 (dependência) | `docs/00-produto/requisitos.md:62`; `docs/01-arquitetura/conectividade-contingencia-satelite.md` §§1–8; contrato de contingência | Não reivindicado como aceite próprio: prioridade sob link restrito e permissões dependem de serviço, produto e especialistas móveis |

Fontes comuns conferidas: `docs/README.md` (prevalência e limites), `docs/00-produto/visao.md`, `docs/01-arquitetura/arquitetura-referencia.md`, `docs/06-planejamento/decisoes.md` (ADR-047, 052–055), `docs/06-planejamento/pendencias.md`, `docs/06-planejamento/matriz-rastreabilidade.md`, mais `docs/01-arquitetura/aplicativo-operacao-normal.md` §§1–6 e referências relacionadas. Entradas de projeto: `AGENTS.md`, `.agents/AGENTS.md` e perfil Lea; protocolo em `SPECS-EM-ANDAMENTO/CONSOLIDACAO-CANONICA.md`.

Contribuições consultadas como propostas, não normas: `contribuicao-lea-verou.md` SHA-256 `7227af2035594ea3190e38738436a7916d92d7bf9c36bbb411d7dd0fa4be2d3e`; parecer `contribuicao-james-bach.md` SHA-256 `9c7da745a1fe3b9ed5f21542e967e9efb03092c32d9630a280e4619cc91642e9`; `contribuicao-chet-haase.md` SHA-256 `c3b7cddcffe4076e68795fcb034200befebe63b539f22972a5f2226843443cfe`; `contribuicao-donny-wals.md` SHA-256 `d9f22f920222343d50125a193e9e6ed78c8bb4c8718e13f23fc56d85bd7b0b6a`. Os pareceres de James apontam casos ainda esquemáticos; esta versão os detalha como cenários planejados, mantendo versões, parâmetros, eventos e IDs de casos em OPEN/NOT_ASSIGNED.

## 2. Objetivo, limites e plataformas

Pessoas autorizadas reconhecem a fase de uma atividade contínua em andamento e sua atualidade sem depender de abrir o app, quando a superfície do sistema operacional comportar. O app principal conserva detalhes e estado acessíveis. Projeção Live Presence não inicia “GodMesh online”; mensagem avulsa e AP detectado continuam fora da superfície persistente.

RF-042 nomeia iOS/Android. Esta SPEC não estende esse requisito à web ou desktop. O SPEC-013, entretanto, reserva contratos de interação para essas plataformas quando seus requisitos e superfícies estiverem confirmados. Até lá, web e desktops macOS/Windows/Linux mantêm o comportamento próprio já definido para o aplicativo e não simulam ActivityKit/Live Updates. Servidor Linux, Node e Platform/SDK não recebem um requisito de UI nesta peça. N/A refere-se à projeção Live Presence, não ao produto inteiro.

| Plataforma | Escopo Live Presence documentado | Decisão de execução ainda OPEN |
|---|---|---|
| iOS | ActivityKit/WidgetKit; Dynamic Island somente quando hardware e visualização forem compatíveis; tela bloqueada nos demais casos | versões, entitlement, permissões, condições de encerramento e fallback homologados; Donny |
| Android | Live Updates em dispositivos/versões elegíveis; notificação adequada ou app nas demais | versão/dispositivo, permissão, dispensa, foreground/background e fallback; Chet |
| Web | sem requisito RF-042 nem superfície persistente definida | ação/eventos de página, foco/teclado, notificações e background só entram após decisão de produto/plataforma |
| macOS, Windows, Linux desktop | sem requisito RF-042 nem superfície persistente definida | notificação nativa, janela minimizada, foco/teclado e preferência local exigem especificação de plataforma antes de aceite |
| Servidor Linux / Node / SDK | não aplicável à projeção visual de cliente | contratos de serviço/evento permanecem necessários; pertencem aos donos de serviço/protocolo |

Não seleciono frameworks ou novas APIs para plataformas sem decisão documentada. As capacidades mobile acima são referências da arquitetura e ADR-052, não prova de que funcionem em versões ou aparelhos específicos.

## 3. Atores, autoridade e contrato de dados

- **Iniciador/participante:** pessoa autorizada que inicia SOS, sessão Guardian, PTT/chamada ou provisionamento Wi-Fi.
- **Observador local:** consulta resumo no dispositivo e abre detalhes autenticados.
- **Autoridade de estado:** o domínio proprietário do evento, não a superfície visual do SO. A UI só exibe a fase sustentada por evento autenticado/autorizado e atual; origem desconhecida ou evidência insuficiente resulta em `unknown`/pendente ou ausência da projeção, nunca em confirmação.
- **Contrato de entrada documentado:** proposta `gmp.live_presence.updated.v1`, campos mínimos `activity_id`, `kind`, `state`, `priority`, `summary_redacted`, `last_update_at`, `expires_at`; arquitetura prevê ainda `source_event_id`, deep link e metadados do ciclo de vida. O contrato não é endpoint implantado.
- Tipos candidatos já documentados: `guardian`, `ptt`, `sos`, `wifi_provisioning`. Estados comuns do contrato: `starting`, `active`, `stale`, `ending`, `ended`, `failed`. Arquitetura acrescenta estados de elegibilidade (`not_supported`, `permission_required`, `eligible`); esclarecer a separação entre elegibilidade local e estado de evento é OPEN.
- Proibidos na projeção: coordenadas precisas, tokens, SSID/BSSID, identidade civil (`gmi_id`) e dados de registro de acesso. `activity_id` não concede consulta sem autenticação/ACL.

## 4. Máquina de apresentação e semântica

A camada cliente mantém dois planos: **estado do domínio**, vindo do contrato de serviço, e **estado da superfície**, que descreve capacidade/autorização/renderização do SO. A falha da superfície não muda por si só o resultado do domínio.

| Estado visível / situação | Evidência mínima exigida | Interação e limite |
|---|---|---|
| Não iniciado | nenhuma | não mostrar atividade persistente |
| Solicitado / pendente | ação local registrada; confirmação remota ausente | informar que o pedido foi feito localmente e não está confirmado |
| Iniciando | evento de início elegível e autorizado | anunciar início apenas como fase iniciada; não implicar envio/entrega |
| Ativo | evento atual da autoridade do domínio para fase ativa | resumo redigido, prioridade aprovada e último update; não inferir ACK ou atendimento |
| Stale / desatualizado | prazo de frescor aprovado excedido ou validade expirou | mostrar última atualização segura; não representar como atividade atual; manter caminho de atualização no app |
| Encerrando | evento terminal em processamento, se fase realmente existir no contrato | não declarar encerrado até evidência de encerramento definida |
| Encerrado | evento de encerramento autenticado e terminal no domínio | linguagem especifica cancelamento, falha, expiração ou resolução; `ended` genérico não significa entrega/sucesso |
| Falha | erro de fase/autoridade com escopo identificado | nomear o que falhou e recuperação possível; não converter falha de notificação em falha SOS/PTT |
| Sem suporte / permissão / dispensado | observação de capability/autorização local ou opção do usuário | superfície ausente; manter detalhes no app e fallback somente se permitido; não afirmar que SO continuará mostrando |
| Desconhecido | telemetria/fonte não suficiente | apresentar desconhecido ou indisponível; nunca concluir sucesso a partir de ausência de erro |

`stale`, timestamp e terminalidade exigem política temporal e causal definida. Não há limiar neste rascunho. A semântica final deve harmonizar o enum da arquitetura com o contrato de serviço antes de declarar aceite completo.

## 5. Regras de apresentação por atividade

| `kind` | Pré-condição para projeção proposta | Fases que não se podem inferir | Fallback |
|---|---|---|---|
| `sos` | incidente explícito iniciado e atividade elegível, autenticada/autorizada | `active` não prova entrega; ACK técnico não prova `claimed`; `claimed` não prova resolução | estado pendente/stale no app; abrir detalhes seguros; localização precisa somente a responsáveis autorizados pelo contrato SOS |
| `guardian` | acompanhamento temporário e consentido ativo | check-in atrasado não vira SOS; última localização não vira posição atual | stale com frescor; expiração/revogação encerra somente quando evento/autoridade confirmar |
| `ptt` | transmissão/chamada atual e grant válido | intenção de transmitir não é TX; fim local sem confirmação não afirma fim remoto; bridge não verificada não é identidade comprovada | indicar perda de atualidade; sincronizar status pelo contrato; manter canal e identidade apenas sob política |
| `wifi_provisioning` | pedido de provisionamento elegível iniciado pelo usuário | pedido não prova autorização do SO, associação, autenticação ou internet | etapa observada, aprovação necessária ou erro; não persistir projeção como status conectado |
| mensagem, AP detectado, “online” | nenhuma | não converter presença ou evento avulso em atividade contínua | notificação normal/estado no app, conforme política existente |

Quando há simultaneidade, ADR-053 e arquitetura colocam SOS ativo em prioridade visual; o estado principal das demais atividades continua acessível no app. Critério concreto de coexistência exige comparar estado-fonte e projeções sob a mesma autoridade/versão; a superfície nativa e affordance não são garantidas uniformes.

## 6. Jornada, permissão e recuperação

1. **Entrada:** validar usuário/ação explícita, categoria, autorização do domínio, capacidade e preferência. Mostrar somente o estado solicitado localmente até existir evento de fase confirmado.
2. **Início/atualização:** consumir evento autorizado e deduplicado; relacionar `source_event_id` ao estado. Se falta versão/autoridade, não promover o resumo.
3. **Permissão requerida/negada/revogada:** indicar qual superfície está indisponível, opção suportada para abrir ajustes quando aplicável e alternativa dentro do app. Não fazer alterações automáticas no SO nem culpar o domínio pelo bloqueio local.
4. **SO dispensa ou não renderiza:** respeitar a decisão. Estado principal permanece disponível no app; fallback em notificação depende de permissões/preferências/política aplicável.
5. **Offline/rede degradada:** distinguir último estado conhecido, `stale` e pendência local. Nunca mostrar “entregue”/“online” baseado em fila local, tentativa de envio ou continuidade da projeção.
6. **Volta à conectividade:** reconciliar com fonte autorizada, validar permissão/capacidade e aplicar ordenação/terminalidade. Não ressuscitar estado expirado ou terminal por evento antigo.
7. **Abertura do deep link:** link é ponte de navegação, não autorização; revalidar sessão, ACL e estado atual antes de conteúdo ou ação sensível.
8. **Cancelamento/expiração/resolução:** encerrar somente na fase descrita pelo contrato. Para SOS, confirmação técnica, humana e resolução são fases distintas; para Guardian, revogação/expiração não implica SOS.

## 7. Privacidade e acessibilidade observáveis

### Privacidade

- Em tela bloqueada e notificações: suprimir coordenadas precisas, identidade civil, SSID/BSSID, credenciais, tokens e registro de visitantes. Preferência do usuário pode reduzir conteúdo adicional; o app não promete controlar capacidades de privacidade do SO.
- Validar autorização no momento de abrir detalhe; deep link não contém segredo nem contorna autenticação/ACL.
- Telemetria ou resumo sem fonte suficiente não deve revelar dados privados nem produzir estado positivo.

### Acessibilidade

- Estado e ação têm nome/propósito acessível; estado nunca depende exclusivamente de cor, forma ou animação.
- Dentro do app: navegação por teclado/foco onde aplicável; ordem de foco previsível e restaurada após abrir/fechar detalhe; leitor de tela recebe mudança de fase essencial, stale, falha e motivo de fallback.
- Texto ampliado e contraste não podem ocultar status/ação. Movimento não essencial pode ser reduzido; nenhuma mudança essencial depende de animação.
- Evitar anúncio repetitivo a cada atualização; frequência e agrupamento de anúncios são OPEN e devem ser testados com leitores de tela das plataformas.
- Cada erro/negativa apresenta explicação, estado atual conhecido, ação de recuperação disponível e alternativa manual segura quando automação/capability não existir.
- Para Live Activity/Update, conteúdo e semântica acessíveis dependem do SO; se não forem verificáveis para determinada capability, cenário fica bloqueado/inconclusivo, nunca declarado acessível por inferência.

## 8. Aceite comportamental e cenários planejados

Os cenários são planos de teste, não executados. Sem IDs atribuídos, usar rótulos provisórios `NOT_ASSIGNED`; observado `NOT_EXECUTED`. A versão exata do OS/aparelho, entitlement/capability, configuração de privacidade e permissões precisam ser registrados antes de execução. Sem esses dados, classificar `BLOCKED` ou `INCONCLUSIVE`, não PASS.

| Caso planejado | Pré-condição/estímulo | Esperado verificável | Invariante/negativo |
|---|---|---|---|
| LP-SOS-normal | SOS iniciado; alimentar separadamente cada evento de fase do contrato SOS | label e destino correspondem ao evento autenticado; app e superfície mantêm mesma fase | não promover pedido local a entregue/claimed/resolvido; prova ausente => pendente/unknown |
| LP-Guardian-normal | sessão consentida ativa; emitir atualização/check-in permitido | categoria e última atualização correspondem à fonte | posição antiga não é presente; atraso não gera SOS |
| LP-PTT-normal | grant/canal válido; iniciar, transmitir e terminar com eventos de domínio | UI separa sessão e TX conforme evidência | sem grant ou fim confirmado, não afirmar transmissão ativa/encerrada |
| LP-WiFi-normal | provisionamento iniciado por usuário; callbacks/etapas apenas quando fonte comprovada | distinguir solicitado, aprovação SO, associação, autenticação e internet verificada | pedido/permite conexão não equivale a conectado |
| LP-permissão | sem permissão, negada e revogada como condições separadas | superfície não é afirmada ativa; motivo e caminho alternativo disponíveis | não altera configuração do usuário; domínio preserva estado independente |
| LP-capability | plataforma/release sem Live Activity/Update elegível | fallback app/notificação permitido e estado principal preservado | ausência de superfície não significa falha do fluxo |
| LP-stale | ausência de atualização em torno do prazo aprovado | transição para `stale`, timestamp seguro e mensagem não atual | sem valor aprovado, execução fica blocked/inconclusive; stale nunca indica atividade atual |
| LP-offline-recovery | perder rede após último estado, gerar ação pendente, reconectar | UI separa estado local, fila, sync e confirmação; reconcilia fonte atual | nunca declarar envio/entrega pela gravação local; terminal antigo não ressuscita |
| LP-duplicate-order | duplicar evento e depois entregar versões fora de ordem/atrasadas | estado final segue regra causal aprovada e fonte correlacionada | duplicata não promove fase; anterior não reabre terminal |
| LP-terminal | cancelar, expirar, falhar e resolver nas categorias em que contrato prevê cada evento | fase e razão exibidas conforme registro | `ended` genérico não prova sucesso/entrega; evento de classe errada não fecha atividade |
| LP-lock-screen | conteúdo reduzido, tela bloqueada, notificação e deep link após reautenticação | nenhuma coordenada precisa/segredo/ID civil aparece; detalhe solicita auth/ACL | preferência/SO mais restritivo é respeitado; screenshot não substitui controle de autorização |
| LP-accessibility | estados normal, stale, permissão negada, falha, ação e concorrência com leitor de tela/texto grande/contraste/redução de movimento | nome, finalidade, atualização importante e recuperação são percebidos sem cor ou motion essencial | foco não se perde em fluxo do app; anúncios não repetitivos conforme política ainda a aprovar |
| LP-concurrency | SOS e Guardian/PTT simultâneos; aplicar atualização a cada fonte | superfície SOS prioritária quando fase ativa aprovada; app permite consultar outras; comparar versão de estado de ambas | prioridade não apaga, encerra ou falsifica outro estado; divergência entre projeção e app é falha do aceite |

Para cada caso futuro, James Bach revisa roteiro com precondição, versão/capability, ação concreta, dados de fixture sintéticos, evento de origem, observado esperado na superfície e app, evidência correlacionável, pós-condições e resultado `PASS/FAIL/BLOCKED/INCONCLUSIVE`. Fixtures e IDs ainda não existem; não coletar dados reais de emergência/localização em especificação documental.

## 9. Decisões OPEN, donos e alternativas

| OPEN | Alternativas e consequência | Dono proposto | Critério de fechamento |
|---|---|---|---|
| Evidência/autoridade por fase | contrato do domínio como autoridade versus nova camada explícita de prova; duplicação pode divergir | donos SOS (Nancy), Guardian (Margaret), PTT (Radia), Wi-Fi (Jouni), Graydon/GMP | label, evento/versão, autoridade, destinatário e comportamento sem prova documentados para cada fase |
| frescor, clock, `expires_at` ausente e stale | regra comum ou específica por categoria; nenhum limiar é aprovado | produto e donos de domínio/sync (Kleppmann) | política, fonte de tempo, tolerância, expiração e reconciliação decididas; limites sem parâmetro são inconclusivos |
| enum elegibilidade vs estado de atividade | unificar enum ou separar estados locais de superfície e domínio | Graydon + Donny/Chet + Lea | enum, transições e responsabilidade iguais em contrato e interface |
| versão/capability e fallback | Live Surface, notificação permitida ou somente app | Donny iOS, Chet Android | matriz de versão/entitlement/política oficialmente homologada, sem afirmar suporte universal |
| prioridade e convivência SOS | SOS ativo como topo visual; definir fase ativa e acesso às atividades paralelas | produto/Nancy + Lea/Donny/Chet | observáveis de prioridade, estados concomitantes acessíveis e fonte comparável definidos |
| conteúdo bloqueado e preferência | resumo genérico por padrão versus conteúdo resumido configurável; conteúdo detalhado só autenticado | usuário/produto + privacidade | política por superfície, preferências, campos e resultado do deep link decididos |
| atualização acessível | anúncio por toda atualização versus apenas mudanças de fase relevantes | Lea + revisores iOS/Android | frequência/agrupamento e acessibilidade definidos, sem perda de mudança crítica |
| escopo UX RF-033/034 | integração das jornadas ou subespecificações pelos respectivos donos | Margaret, Chet/Donny, Lea como interface | índice de Ada atribui responsabilidades e vincula as jornadas, sem declarar cobertura por RF-042 |

## 10. Dependências e critérios de liberação documental

Dependem desta SPEC: contratos de fase/evento de SOS, Guardian, PTT e provisionamento; autoridade, idempotência, sequência, frescor e reconciliação; matriz Android/iOS por capability; política de privacidade; aceites dos domínios. Revisão da versão DRAFT-0.1 de James é **parcial**, conforme parecer: confirma adequação do recorte, porém casos e mapeamentos continuam em aberto. Esta edição foi expandida após o parecer; uma nova revisão precisa citar hash desta versão.

Para elevar de DRAFT: resolver OPENs ou mantê-los com dono/evidência/alternativas/consequência/critério, revisar casos detalhados com James, reconciliar eventos com autores de plataforma e domínios, e rastrear RF-042/RNF-016 no índice de Ada. RF-033/034 e jornadas UX restantes só serão cobertos após confirmação dos respectivos donos. A aprovação desta SPEC é gate separado e permanece pendente. Não há execução de produto, teste de aparelho, implementação ou validação.

## 11. Registro da revisão independente

**Parecer de James Bach:** `SPECS-EM-ANDAMENTO/contribuicao-james-bach.md`, seção “Revisão independente — SPEC-013 DRAFT-0.1”, contra SHA-256 `7a9d509f41f87d51d05f0854f7eb76f963d9603ce89a0f417559ee36b740a8cc`. Revisão focal de RF-042/RNF-016, eventos/autoridade, casos, privacidade/acessibilidade e plataformas.

**Resultado:** sem contradição direta identificada com RF-042/RNF-016 ou com as fontes Live Presence consultadas. James considerou o rascunho substancial e transparente sobre cobertura parcial; não o aprovou. Restrições observadas:

1. Mapeamento por fase/categoria para evento versionado, autoridade e destino continua parcial/OPEN.
2. Os 13 cenários são planejados, ainda sem fixtures, passos reproduzíveis, build/release e evidência correlacionável; `NOT_ASSIGNED`/`NOT_EXECUTED` permanecem corretos.
3. Matriz de versão/aparelho/entitlement/capability e fallback acessível por plataforma não está homologada; `N/A` deve ser justificado por combinação.
4. Privacidade/acessibilidade ainda dependem de decisões para anúncio/agrupamento e observação com leitor de tela e configurações de SO.
5. RF-033/RF-034 permanecem dependências e não se tornam cobertura desta SPEC.

O parecer é evidência documental sobre a versão/hash citado, não comprova comportamento de dispositivo nem validação de produto. Como este registro altera o arquivo, a versão corrente requer confirmação do revisor para o novo hash antes de qualquer gate que dependa de revisão desta versão.

## 12. RF-034 — jornada de operação normal em desktop (complemento)

**Estado:** adição documental DRAFT/OPEN; critérios e casos planejados, não executados. RF-034 segue como requisito de produto aprovado na origem; integração desktop nesta SPEC ainda requer revisão de Margaret e Alex e revisão independente atualizada. Este complemento não muda estado de RF-034 nem valida a aplicação.

**Fonte e fronteira:** `docs/00-produto/requisitos.md:68` requer as áreas Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS, modos simultâneos e offline. `docs/01-arquitetura/aplicativo-operacao-normal.md:7–19,83–85` descreve intenção, conteúdo e aceites de operação offline, simultaneidade, SOS e independência de permissões. A visão `docs/00-produto/visao.md:18–20` e ADR-055 incluem macOS universal arm64/x86_64, Windows e Linux desktop sem demonstrar API ou função implementada. SPEC-012 §§3–6 trata contexto/Guardian e OPEN-01/04/05; SPEC-014 §§5–8,10,12 (DRAFT-0.1, SHA-256 `96df15483c1b2741d5c3e865ebea2a5a5bee4dd9e311c522eb3969c9b60b1bac`) declara núcleo offline/persistência/FFI e fronteiras OPEN. A SPEC-013 especifica observabilidade UX, não estado autoritativo local nem contrato de sync. Margaret confirmou os fatos de produto e OPENs em mensagem 6935a21e-19a5-4a46-bb2e-ff64c717ea1d; Alex confirmou estado local/FFI e limites em mensagem 37122cf4-63c0-4d64-b9b7-29db6cbbf428. São contribuições documentais, não decisões ou aceite.

### 12.1 Jornada desktop

1. **Abrir Início:** não bloquear a superfície principal aguardando rede. Apresentar conectividade, sessões e avisos por serviço, com frescor e fonte disponíveis; evitar selo agregado de “tudo conectado”.
2. **Navegar áreas:** Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS permanecem alcançáveis na superfície suportada. Área sem capacidade reconhecida explica limitação específica, mantendo áreas independentes utilizáveis.
3. **Uso concorrente:** PTT, Guardian e conectividade degradada podem coexistir; SOS recebe prioridade visual conforme atividade válida sem apagar estado anterior de Guardian/PTT. A prioridade não promove tentativa local a incidente entregue.
4. **Offline:** apresentar dados locais autorizados, idade/estado de frescor e itens pendentes separadamente; não marcar mensagens/eventos pendentes como enviados, entregues, sincronizados ou reconhecidos. Após reconexão, refletir somente reconciliação observada pela fronteira de estado autorizada.
5. **Permissão/capacidade:** quando feature depende de permissão ou capacidade indisponível, informar função afetada e alternativa acessível; negar/revogar uma permissão não deve desabilitar telas/funções sem dependência documentada.
6. **Ação sensível:** antes de acionar SOS, revogar Guardian, iniciar compartilhamento ou abrir dados restritos, apresentar escopo/destinatário e resultado esperado quando a decisão do domínio exigir confirmação; após ação, mostrar estado retornado pelo contrato, não pressuposto da interface.

### 12.2 Matriz de desktop e fronteiras de plataforma

| Plataforma | Base do produto | Interação UX coberta | Limites / OPENs |
|---|---|---|---|
| macOS Apple Silicon arm64 | App desktop universal com SwiftUI consta no `docs/README.md` e ADR-055 | teclado/foco, áreas, navegação de estados, redução de movimento e alternativas a ação indisponível | APIs de permissão/background, comportamento em janela minimizada/bloqueada, capacidades locais e versão mínima não determinados nesta SPEC |
| macOS Intel x86_64 | mesma modalidade universal indicada documentalmente | mesmas jornadas, com aplicabilidade e estados descritos sem presumir paridade de capability | validar diferença por capability/OS; nenhuma execução/hardware verificada |
| Windows desktop | plataforma de produto no README/ADR-055; seleção de interface/tecnologia permanece aberta onde não aprovada | áreas e estados via superfície que vier a ser aprovada; navegação de teclado/foco, recuperação e feedback de serviço | não escolher stack nem APIs; capacidades/permissões/background e leitor de tela a homologar |
| Linux desktop | plataforma de produto no README/ADR-055 | mesmas jornadas; reconhecer função não suportada e preservação de áreas independentes | não presumir paridade com servidor Linux; stack, notificações, capabilities e background permanecem OPEN |
| Web | parte do escopo integral, mas RF-034 não define superfície/jornada web específica | nenhuma interação nova é presumida; requisito web e comportamento por jornada ficam rastreáveis como OPEN | não excluir web do produto; produto define escopo, contexto de permissão, abas/foco, notificação, background e recuperação antes de aceite web |

As áreas são intenção de navegação do produto, não decisão sobre composição de janelas, abas, rotas ou framework. Não inferir que localização, Bluetooth, Wi-Fi, sensores, notificações ou background estão disponíveis. Um cenário sem confirmação de capability deve resultar em `unknown`/`unavailable` e função limitada, não assumir suporte.

### 12.3 Critérios comportamentais e negativos

| Cenário planejado | Pré-condição / estímulo | Esperado na UI e no estado | Negativo / recuperação |
|---|---|---|---|
| Navegação normal | cliente abre com capacidades declaradas; selecionar cada área nomeada | cada área alcançável e mostra, de forma separada, conectividade, disponibilidade local, autorização, estado de sync, frescor e capacidade daquela função | erro em uma área não deve substituir todas por falha genérica; a área independente continua acessível; `unknown` não pode virar sucesso |
| Inicialização offline | sem rede; há e não há cache local autorizado em subcasos distintos | Início abre; disponibilidade local diferencia ausente, não durável em memória, persistido e inacessível; autorização e frescor continuam visíveis separadamente | nenhum item pendente vira entregue; escrita local falha sem alegar durabilidade; mutação offline ocorre somente se domínio, grant e política aprovarem; reconexão depende de SPEC-014/004/005 |
| Modos simultâneos | PTT e Guardian ativos e conectividade degradada; depois incidente SOS válido | estados permanecem identificáveis; SOS recebe prioridade sem apagar estado de Guardian/PTT; detalhe de cada atividade continua acessível | iniciar SOS não encerra Guardian/PTT; atraso Guardian não vira SOS |
| Permissão negada/revogada | permissão/capacidade requerida é ausente, negada ou revogada, separadamente | identificar área afetada, motivo conhecido e alternativa suportada; funções independentes mantidas | não afirmar ação de sistema concluída; sem leitura autorizada após revogação observada |
| Capacidade ausente | sensor/recurso requerido não suportado ou desconhecido | explicar limitação da plataforma e limitar somente função dependente | não transformar desconhecido em permissão negada nem falha global; não inferir API disponível |
| Mensagem sem entrega | criar mensagem em estado local pendente e manter sem confirmação do contrato | texto e indicador refletem persistido local, pendente/tentado, confirmado pelo destino ou `unknown` em campos separados; estado muda apenas em evento da fase respectiva | gravação local/FFI não é entrega; sem ACK não pode aparecer como enviada/entregue/recebida; retry/replay depende de SPEC-004 e não duplica operação |
| Foco/teclado | executar jornadas sem mouse, abrir diálogo, negar/dispensar e retornar | foco inicial, ordem, rótulo e retorno ao ponto de origem permanecem utilizáveis; erro tem ação acessível | não prender foco nem esconder estado atrás de hover/cor; casos ficam OPEN até design/capability da interface definido |
| Leitor de tela/texto/contraste/motion | percorrer estados normal, offline, stale, negado, erro e SOS prioritário | nomes e atualizações essenciais são anunciados; informação não depende de cor/animação; conteúdo ampliado permanece disponível | alternativa não pode omitir motivo/recuperação; anúncio não repetitivo exige decisão de política de frequência |
| Layout/plataforma reduzida | abrir área não suportada, janela sem espaço ou recurso sem capability | mensagem identifica recurso/área e alternativa, preserva estado não relacionado | não afirmar que serviço está operante; não descartar dado local ou atividade por mera indisponibilidade visual |

Cada linha é caso futuro sem ID: `NOT_ASSIGNED`; `observed=NOT_EXECUTED`. Para torná-la reproduzível faltam versão/build por plataforma, permissões, fixtures sem dados reais, ação concreta, fonte de evidência e esperado/observado. PASS/FAIL/BLOCKED/INCONCLUSIVE não podem ser atribuídos nesta fase.

### 12.4 Estado, permissão e recuperação — responsabilidade

Esta SPEC descreve o que a pessoa observa. Dono do domínio determina autoridade e estado de SOS/Guardian/PTT; Alex especifica persistência/estado local e a fronteira offline; Kleppmann/Graydon definem sync/evento; Donny cobre fronteiras iOS e Chet cobre Android, conforme atribuições atuais. Em macOS, Lea é integradora documental de jornada/UX/acessibilidade desktop e Alex cobre a interface documental do estado local/FFI; nenhum dos dois recebe responsabilidade por APIs/capabilities nativas, suporte por versão, plantão ou validação. Margaret decide a jornada e semântica de produto. A UI não inventa transições, retenção nem ordem de retry. Para cada área, exibir separadamente conectividade, disponibilidade local (`absent`, memória não durável, persistido, inacessível), autorização (`valid`, denied, expired, unknown), sync (`pending`, attempted, destination-confirmed, unknown), frescor e capacidade por função; nomes de wire-state dependem da conciliação com SPEC-014/004. Escrita local/FFI não é commit, sync nem entrega; falha de persistência não pode ser apresentada como durável. Mutação offline só é permitida se domínio e política/grant a autorizam. Retry/replay segue SPEC-004, retenção/recuperação SPEC-005 e autorização SPEC-002; FFI deve resolver ownership/lifetime, erro, executor/thread, async/cancelamento/shutdown antes de fixar bindings. Revogação deve impedir acessos subsequentes quando comprovada pela autoridade pertinente; remoção de cópias, propagação offline, conflitos e reconciliação seguem contratos de estado/sync ainda dependentes.

### 12.5 OPENs do complemento

- **Matriz de capacidades/versões:** por plataforma, produto/oferta, recurso e estado foreground/background; donos de UX documental: Lea para jornadas/acessibilidade desktop; Alex para estado local/FFI; Donny para iOS, Chet para Android. Dono técnico de API/capability/suporte macOS ainda não designado. Fechamento: fontes oficiais e homologação documental sem presumir API.
- **Jornada e rotas das áreas RF-034:** estrutura de navegação e áreas exibidas por perfil; dona: Margaret; fechamento: escopo e regras de conteúdo aprovados e rastreados.
- **Estado local/offline/FFI:** cache autorizado, idade/frescor, fila, conflitos, retry, falha de escrita e reconciliação; dono: Alex/SPEC-014 e Kleppmann/SPEC-004; fechamento: contrato de origem e transições explícito, incluindo ownership/lifetime, erro, executor/thread, async/cancelamento/shutdown na fronteira FFI.
- **Permissões por feature e sistema:** quando pedir, negado/revogado, Settings/manual action, capacidade indisponível; donos: plataformas; fechamento: matriz por SO/release apoiada por documentação oficial.
- **Acessibilidade por desktop/web:** padrões observáveis de teclado/foco/leitor/zoom/contraste/motion e ações de recuperação; dono: UX com revisores de plataforma; fechamento: casos vinculados às superfícies aprovadas.
- **Web:** escopo por jornada permanece aberto; produto precisa decidir comportamento e cobertura antes de qualquer alegação de N/A ou aceite web.
- **Persistência local e retry:** gravação via fronteira local/FFI não prova envio ou entrega; replay depende de SPEC-004, retenção/recuperação de SPEC-005 e autorização de SPEC-002. Seleção/binding, lifecycle e compatibilidade por sistema operacional não foram validados.
- **Prioridade/energia RF-043:** P0–P3 é direção documental em `aplicativo-operacao-normal.md:6`, não se transforma nesta SPEC em agenda normativa para desktop; perfis de energia, metas e impacto continuam OPEN com produto/plataformas.

Nenhum desses OPENs autoriza escolha de framework, serviço ou API. Critério de fechamento deste complemento: Margaret/Alex e responsáveis de plataforma revisam as fronteiras; tabela de estados referencia SPECs de domínio/offline; matrizes de plataforma e casos são completados sem valores ou suporte inventados; responsável técnico de API/capability macOS é explicitamente designado; James revisa o novo hash. Não representa implementação, suporte validado ou aprovação de RF-034.

### 12.6 Insumo de estado local recebido de Alex Crichton

Alex confirmou, em mensagens 984a0856-49d4-4dc7-b6a0-7c1029f6cee4 e 37122cf4-63c0-4d64-b9b7-29db6cbbf428, as distinções entre disponibilidade local (ausente/memória não durável/persistido/inacessível), autorização (válida/negada/expirada/desconhecida), sync (pendente/tentado/confirmado por destino/unknown), frescor e capacidade. A mutação offline depende do domínio/política/grant; falha de escrita não prova durabilidade; replay, retenção/recuperação e autorização dependem das SPECs 004/005/002. Integrei essas distinções aos cenários e à fronteira da seção 12.4. FFI, binding, lifecycle e compatibilidade por OS seguem OPEN; P0–P3/energia não foram promovidos a nova política desktop. Nenhum caso executado.

### 12.7 Mapeamento de UX para inventário de cliente da SPEC-014

SPEC-014 §8 define somente possibilidade de semântica local comum sob condição de que o core Rust seja consumido; marca APIs/bindings/lifecycle desktop como não definidos ou não validados. O inventário atualizado em SPEC-014 §18 (DRAFT-0.1, SHA-256 `e16b4a252e4713374d05575dd725280b1eb451558b15c3dd55ed91ffa79c8670`) discrimina macOS arm64/x86_64, Windows, Linux, iOS, Android e web: contrato comum condicional, adaptações nativas e donos/lacunas por alvo. Alex confirma que esse split cobre semântica local/FFI comum, mas não ownership técnico de APIs, armazenamento/lifecycle nativos macOS nem validação por plataforma. Este mapeamento indica estados que a UX deve distinguir se o contrato correspondente vier a existir; não prova que há UI, armazenamento, API, evento ou suporte operacional em qualquer alvo.

| Inventário da SPEC-014 | macOS arm64 / x86_64 | Windows desktop | Linux desktop | Responsável e lacuna |
|---|---|---|---|---|
| Conectividade, estados simultâneos e `unknown` | UX mostra serviço e conectividade separadamente | mesma semântica, sem presumir implementação | mesma semântica, sem inferir equivalência com servidor Linux | Lea: jornada; Alex: estado local; event contract/reconciliation em SPEC-004 OPEN |
| Disponibilidade (ausente, memória não durável, persistido, inacessível) | nunca chamar memória ou fila não durável de salvo | mesmo limite comportamental; mecanismo nativo OPEN | mesmo limite comportamental; mecanismo nativo OPEN | Alex: semântica compartilhada condicional; classe de dado/retention OPEN com SPEC-005 |
| Autorização válida, negada, expirada, desconhecida | indicar função limitada e respeitar decisão; sem Settings/API assumida | mesmo princípio, caminhos de SO OPEN | mesmo princípio, caminhos de SO OPEN | autorização/escopo/propagação dependem SPEC-002 e contrato por plataforma OPEN |
| Sync pendente, tentado, destino-confirmado, unknown | comunicar fases distintas; local write não é envio | idem | idem | SPEC-004 define evento/replay; confirmação e nomes finais OPEN |
| Lifecycle, bloqueio, shutdown/restart | indicar perda/inacessibilidade somente com evidência | cenários e mecanismo OPEN | cenários e mecanismo OPEN | lifecycle desktop nativo não inventariado; responsável técnico adicional/designação e fontes OPEN |
| FFI e binding do núcleo | erro/cancelamento não vira sucesso de UI | sem binding definido; não presumir Rust consumido | sem binding definido; não confundir alvo cliente com server | Alex integra contrato comum; adapters nativos/ownership/async/thread/cancelamento/shutdown OPEN; revisão técnica independente necessária |
| Recuperação / migração / chave indisponível | apresentar dado não disponível e ação segura, se definida | comportamento por classe OPEN | comportamento por classe OPEN | SPEC-005 dados; SPEC-002 autorização; nenhuma disposição ou recovery policy selecionada aqui |

Cobertura de interface documentada por Lea abrange somente os estados e jornadas observáveis; Alex integra contratos técnicos comuns de cliente/local/FFI nos três desktops conforme atribuição de Alan. SPEC-014 §18 confirma que a integração não define bindings específicos, APIs, armazenamento, lifecycle, chaves ou compatibilidade por versão. Não há responsável técnico nativo nem revisor especializado designado para macOS/Windows/Linux; essa lacuna impede concluir suporte, mas não elimina os alvos do produto. Donny continua apenas iOS; Chet, Android. Plataformas e arquiteturas nomeadas no produto não significam suporte validado. Web fica fora desta tabela porque seu fluxo de estado local e jornada de página ainda não foram definidos; não é excluída do produto.

**Casos planejados ligados ao inventário:** (a) leitura local offline autorizada vs inacessível; (b) falha de escrita antes/depois de persistência confirmada; (c) mutação offline permitida vs negada por política; (d) revogação local conhecida vs propagação desconhecida; (e) tentativa de sync sem confirmação do destino; (f) reinício/shutdown durante mutação, replay ou cancelamento. Para cada desktop/arquitetura, registrar build/release e fixture, capacidade/autoridade de origem, permissão, estado inicial, estímulo, UX visível, estado final esperado, evidência e classe `PASS/FAIL/BLOCKED/INCONCLUSIVE` antes da execução. IDs permanecem `NOT_ASSIGNED`; estado observado `NOT_EXECUTED`. Até inventário de API e contratos serem detalhados, cenários de lifecycle/FFI ficam `BLOCKED`/OPEN, não PASS.

### 12.8 Revisão independente do ownership

James Bach revisou a SPEC-013 SHA-256 `919bddf63c8e57e2de9357b4787f0d4068748069f5d1c0ac85b6737a1b5402e4` (registro em `SPECS-EM-ANDAMENTO/contribuicao-james-bach.md`). Confirmou a divisão documental Lea (UX desktop), Alex (estado local/FFI), Donny (iOS) e Chet (Android); macOS técnico permanece OPEN sem nova divergência. A revisão não cobre a matriz nova §12.7/§12.8, não aprova a SPEC, não confirma APIs e não atesta execução. James continua revisor independente do novo hash após este complemento.

### 12.9 Re-revisão e correção de referência

James Bach revisou a versão SHA-256 `4d2b7f065e3ce2bc6dcccd411524bb2a12a0ca22fe862416f22d92c817a8965e` e confirmou que a divisão Lea/Alex estava clara; registrou que a referência à SPEC-014 deveria apontar a uma versão recuperável. Naquela versão, §12.7 ainda citava o hash anterior do §17. Corrigi a referência para o arquivo canônico recuperável SPEC-014 §18, DRAFT-0.1, SHA-256 `4dc6c07bef2239a8a020ae9003e9b6745057b202e58f480831d8f2e9f750fc05`, conferido localmente. Seu §18 contém o inventário por plataforma referido nesta SPEC. Permanecem OPEN APIs/suporte/adapters, casos executáveis, capabilities e revisores técnicos especializados por SO. A re-revisão de James sobre o hash citado não aprova SPEC-013 nem comprova testes; o novo hash da presente edição requer confirmação independente.

### 12.10 Revalidação da referência SPEC-014

Na cópia local consultada em 2026-11-16, SPEC-014 §18 tinha SHA-256 `17ebf271790cb857b7378a0a2c1c6ed2d69976b5889a18673c84c8c7a5a5f8a6`; atualizei §12.7 para citar essa versão. Esta anotação registra a checagem anterior, não afirma que o hash continua atual. A rastreabilidade da referência depende de nova revisão independente e da identificação de uma versão recuperável; a correção não aprova a SPEC nem fecha APIs, suporte, casos executáveis, capabilities ou revisores técnicos por SO. Nada foi executado.

### 12.11 Divergência de cópias da SPEC-014

Na re-revisão, James confirmou que SPEC-014 §18 na cópia local corresponde ao SHA-256 `e16b4a252e4713374d05575dd725280b1eb451558b15c3dd55ed91ffa79c8670` citado em §12.7. Corrigiu também a origem do hash `df02aea8`: ele veio de uma consulta ao caminho errado e não constitui evidência de divergência da cópia canônica. Não foram encontradas cópias arquivadas dos hashes históricos `17eb…` ou `df02…`; por isso, a recuperação/reconciliação histórica permanece OPEN, embora a referência atual seja rastreável na cópia local. Suporte e implementação continuam não verificados.

## Mudanças desta versão — ADR-058 e ADR-060

- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- **casos por requisito e casos P desbloqueados (C3-1):** subseção `Casos por requisito (P/N/B/F)` ao final, uma linha por requisito coberto; e `S013-056-P1`, `S013-059-P1` e `S013-062-P1` reescritos para o comportamento com parâmetro ausente (`param_not_configured`), sem inventar janela, frescor ou prazo.

**ADR-058 — ACK:** a UI mostra somente a classe recebida e sua evidência/destino. É proibido promover classe por passagem do tempo, sucesso de camada inferior ou ausência de erro. `unknown` aparece explicitamente como desconhecido; não é convertido em pendente confirmado, entrega ou atendimento.

**ADR-060 — confidencialidade:** a UI expõe `confidentiality` por canal e por perna. Toda perna que termine a proteção, incluindo bridge, ASR ou gravação, é marcada como terminador (`terminated`). Não usar rótulo E2EE sem prova de que a proteção se aplica àquele canal/perna; TLS, DTLS ou SRTP por si só não autorizam esse rótulo. Tela bloqueada não exibe posição precisa nem segredo.

As revisões citadas neste documento são de versões anteriores. A revisão independente desta versão/hash permanece pendente; nenhum parecer anterior é atribuído ao DRAFT-0.2.


## Casos por decisão (ADR-056…063)

Casos documentais planejados de comportamento observável de UI. ID formal: `NOT_ASSIGNED`. `Estado` em todos: `NOT_EXECUTED`. Parâmetro OPEN resulta em exibir desconhecido/negar, nunca em número inventado. Nenhum caso define enum de wire; a UI só renderiza o que o contrato entrega.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S013-056-P1 | ADR-056 | Cliente e serviço cujo par N/N-1 não tem janela aprovada (`window=not_decided`) | Ação que chama a API | Resposta exibe `compatibility=param_not_configured`; a tela mostra perfil/versão recebidos e informa compatibilidade desconhecida; não declara sucesso, suporte nem fim de suporte; nenhum valor de janela é presumido | P | NOT_EXECUTED |
| S013-056-N1 | ADR-056 | Resposta de incompatibilidade antes do efeito | Nova tentativa de ação incompatível | Tela exibe incompatibilidade com `required_version`/`required_profile`; nenhum retry automático reenvia a mutação; botão de nova tentativa exige ação da pessoa | N | NOT_EXECUTED |
| S013-056-B1 | ADR-056 | Janela de calendário OPEN | Tela de suporte/versão | Mostra `window=not_decided`; nenhuma data de fim de suporte exibida | B | NOT_EXECUTED |
| S013-056-F1 | ADR-056 | Timeout sem resposta conclusiva do serviço | Ação em tela | Estado exibido `unknown`; nenhum selo de sucesso, envio ou entrega; texto não afirma conclusão | F | NOT_EXECUTED |
| S013-057-P1 | ADR-057 | Cursor de timeline/presença com `partition` e `authority_epoch` válidos | Rolagem/paginação | Eventos na ordem canônica; `cursor_version` exibido ou disponível no detalhe; nenhuma releitura duplicada | P | NOT_EXECUTED |
| S013-057-N1 | ADR-057 | Cursor sem partição ou de epoch incomparável | Nova paginação | Erro de cursor exibido; lista não mescla eventos de origens distintas; nenhum item "traduzido" aparece como atual | N | NOT_EXECUTED |
| S013-057-F1 | ADR-057 | Mudança de `authority_epoch` entre páginas | Replay da paginação | Lista anterior marcada `stale`; nenhuma duplicata inserida; nenhum evento apresentado como novo | F | NOT_EXECUTED |
| S013-057-B1 | ADR-057 | Dois eventos com mesma `seq` em partições distintas | Exibição da timeline | Ambos visíveis e identificáveis; nenhum colapso por chave sem partição | B | NOT_EXECUTED |
| S013-058-P1 | ADR-058 | Resposta com `ack_class=persisted_local` | Envio de mensagem/registro | Indicador mostra "persistido local" com a evidência recebida; não mostra "enviado", "entregue" ou "recebido" | P | NOT_EXECUTED |
| S013-058-N1 | ADR-058 | Mesma classe `persisted_local` mantida por tempo | Espera e reabertura da tela | Classe inalterada; promoção para `delivered_to_device` bloqueada; nenhum ícone de entrega aparece sem recibo do destino | N | NOT_EXECUTED |
| S013-058-B1 | ADR-058 | Evento `acknowledged_human` sem `principal_ref` | Exibição do detalhe | Mostra "não reconhecido por pessoa"; nenhum nome de pessoa exibido | B | NOT_EXECUTED |
| S013-058-F1 | ADR-058 | Envio com confirmação perdida | Reabertura da tela depois de timeout | Texto "desconhecido" explícito; nunca "pendente confirmado"; ação de reenvio exige decisão da pessoa | F | NOT_EXECUTED |
| S013-059-P1 | ADR-059 | Offline; grant vigente para check-in e mensagem a canal já autorizado, sem limite de frescor aprovado | Ação permitida em tela | `grant=valid` e `authorization=unknown` exibidos em dimensões separadas, com `freshness_limit=param_not_configured`; a ação não entra na fila e nenhum pendente local é exibido como válido; a ausência de parâmetro não revoga o grant | P | NOT_EXECUTED |
| S013-059-N1 | ADR-059 | Offline; estado de autoridade `stale`/`unknown` | Tentativa de divulgar localização precisa, mutação federada ou alterar grant | Ação negada com motivo exibido; nenhuma promessa de envio futuro; nada enfileirado | N | NOT_EXECUTED |
| S013-059-B1 | ADR-059 | Offline em rede local sem grant de floor | Tentativa de iniciar PTT | PTT não inicia; tela mostra negado por falta de grant de floor | B | NOT_EXECUTED |
| S013-059-F1 | ADR-059 | Relógio incerto ou limite de frescor OPEN | Operação offline condicional | Tela mostra negado/desconhecido com `freshness_limit=not_decided`; nenhum prazo exibido como se fosse aprovado | F | NOT_EXECUTED |
| S013-060-P1 | ADR-060 | Canal com perna terminadora (bridge, ASR ou gravação) | Abertura do canal e do detalhe da mensagem | `confidentiality=terminated` visível no cabeçalho e no detalhe; nenhum cadeado de E2EE | P | NOT_EXECUTED |
| S013-060-N1 | ADR-060 | Artefato terminado ou sem metadado | Tentativa de exibir rótulo "E2EE" ou "anônimo" | Rótulo bloqueado; UI exibe terminado; nenhuma promessa de anonimato | N | NOT_EXECUTED |
| S013-060-F1 | ADR-060 | Metadado `confidentiality` ausente na cópia recebida | Renderização da mensagem | UI assume `terminated`; ausência não é suprida por cache ou versão anterior | F | NOT_EXECUTED |
| S013-060-B1 | ADR-060 | Bridge sem degradação de conteúdo | Detalhe da perna | Mostra `terminated` e "sem perda de conteúdo aplicada"; ainda sem rótulo de E2EE | B | NOT_EXECUTED |
| S013-061-P1 | ADR-061 | Projeção de estado materializada com evidência da fase | Recebimento de evento de fase | UI avança o estado apenas após o evento da fase respectiva; estado anterior permanece legível | P | NOT_EXECUTED |
| S013-061-N1 | ADR-061 | Sessão/incidente já em estado terminal | Replay de evento que reconstruiria projeção | Nenhuma notificação ou superfície é reaberta; terminal permanece terminal na tela | N | NOT_EXECUTED |
| S013-061-F1 | ADR-061 | Autoridade `stale`/`unknown` | Ação de mutação forte apresentada na tela | Controle desabilitado com motivo e estado `unknown` exibido; nenhuma ação enviada; leitura continua disponível | F | NOT_EXECUTED |
| S013-061-B1 | ADR-061 | Inbox separado da projeção; ACK máximo `persisted_local` | Exibição de mensagem | Estado exibido "aguardando materialização"; nenhum "materializado" ou "entregue" por inferência | B | NOT_EXECUTED |
| S013-062-P1 | ADR-062 | Política por classe existente, com prazo não decidido | Abertura de histórico | Tela mostra `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo é exibido como aprovado nem como "retido por N"; a referência da política permanece identificada | P | NOT_EXECUTED |
| S013-062-N1 | ADR-062 | Classe sem política aprovada | Tentativa de gravar e abrir histórico | UI declara indisponibilidade explícita ("histórico não disponível"); nenhum item aparece como salvo; degradação visível | N | NOT_EXECUTED |
| S013-062-F1 | ADR-062 | Descarte em andamento por cópia; revogação local conhecida | Consulta ao item revogado | Estados por cópia exibidos; nenhuma mensagem de "removido de todos os lugares"; remoção remota não anunciada | F | NOT_EXECUTED |
| S013-062-B1 | ADR-062 | Hold de auditoria ativo | Consulta comum ao item retido | Tela mostra dado retido para auditoria e acesso comum não reativado | B | NOT_EXECUTED |
| S013-063-P1 | ADR-063 | Release assinada com `generation` igual ou superior ao piso | Instalação limpa/atualização | Instalação aceita; tela de versão mostra versão/digest e nenhum aviso de integridade | P | NOT_EXECUTED |
| S013-063-N1 | ADR-063 | Release com `generation` inferior ao piso (versão vulnerável) | Tentativa de atualização/downgrade | Atualização rejeitada; versão atual mantida; motivo do piso exibido; nenhum fallback silencioso | N | NOT_EXECUTED |
| S013-063-F1 | ADR-063 | Assinatura inválida ou chave de assinatura revogada | Aplicação do artefato | Rejeição com `signature_invalid`/`key_revoked` exibido; artefato não executado; nenhuma opção de ignorar | F | NOT_EXECUTED |
| S013-063-B1 | ADR-063 | Rollback autorizado com piso rebaixado por decisão registrada e limite de tempo | Instalação do artefato anterior | Aceito apenas dentro do limite; tela mostra estado de exceção e registro de auditoria; fora do limite volta a rejeitar | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** cobertura limitada a superfícies com contrato de projeção definido; web e desktops sem requisito RF-042 não têm caso de Live Presence e não recebem caso por analogia. Frescor de Live Presence, cadência e stale date seguem OPEN (SPEC-016 §13) e por isso não há caso positivo com valor temporal aprovado.

### Casos por requisito (P/N/B/F)

Uma linha por requisito coberto. Células citam casos já existentes (§8 `LP-*`, §12.3 e `Casos por decisão`); onde não havia caso do tipo, o ID é novo e descrito na coluna Nota. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. Parâmetro OPEN aparece como `param_not_configured`/`not_decided`, nunca como valor. Os casos transversais ADR-056…063 permanecem na tabela anterior.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-042 | LP-SOS-normal; LP-Guardian-normal; LP-PTT-normal; LP-WiFi-normal | LP-terminal; LP-duplicate-order | LP-capability; LP-concurrency | LP-offline-recovery; LP-permissão | Cobertura iOS/Android; matriz de versão/capability por SO segue OPEN (§9). Web e desktops sem RF-042 não recebem caso por analogia. |
| RNF-016 | LP-PTT-normal; LP-SOS-normal | LP-terminal; S013-058-N1 | LP-lock-screen; LP-capability | LP-stale; S013-058-F1 | Rótulo nunca excede a evidência; classe de ACK é exibida como recebida (ADR-058), sem promoção por tempo. |
| RF-034 (§12.3) | Navegação normal; Modos simultâneos | Mensagem sem entrega; Capacidade ausente | Layout/plataforma reduzida | Inicialização offline; Permissão negada/revogada | Complemento desktop; capability/autorização por SO seguem OPEN (§12.5) e não há paridade presumida entre macOS/Windows/Linux. |
| RF-033 (dependência) | n/a — esta SPEC não reivindica aceite próprio de prioridade sob link restrito (§1) | S013-059-N1 | S013-059-B1 | S013-059-F1 | Apenas o efeito de UI do grant é verificado aqui; prioridade e orçamento pertencem à SPEC-023. |
