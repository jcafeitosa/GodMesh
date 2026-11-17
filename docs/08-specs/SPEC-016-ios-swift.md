# SPEC-016 — cliente iOS e integração Swift

**Versão:** DRAFT-0.6  
**Estado:** DRAFT / OPEN. Não aprovada, implementada ou validada.  
**Autor documental:** Donny Wals.  
**Revisão independente designada:** Alex Crichton, fronteira Rust–Swift e cliente offline.  
**Data:** 2026-11-17.

## 1. Objetivo e escopo

Esta SPEC define o comportamento documental do cliente GodMesh para iOS e de sua integração Swift com o núcleo local. Abrange capacidades, permissões, ciclo de vida, execução em segundo plano, dados locais protegidos, conectividade degradada, Guardian/SOS, energia e Live Presence.

O cliente informa o que o sistema operacional e o dispositivo permitem no momento, solicita apenas permissões necessárias a uma ação iniciada pelo usuário e apresenta estado local, estado remoto e incerteza separadamente. A disponibilidade de uma API ou permissão não garante execução contínua, transporte de rede ou entrega.

Fora do escopo: decisão de protocolo e sync durável, política global de autorização/retensão, contratos de SOS e ACK humano, interface Kotlin, implementação, seleção de banco ou binding, limiares energéticos e metas de desempenho. Esses temas permanecem dependências explícitas.

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
- **iOS:** controla autorizações, acesso a sensores, estado de execução e elegibilidade de superfícies do sistema. O app não contorna essas decisões.
- **Aplicativo Swift:** consulta capacidades e estados atuais; exibe efeitos e limitações; adapta UX às APIs elegíveis.
- **Núcleo Rust local:** mantém domínio e estado compartilhado conforme contratos próprios. A FFI Rust–Swift é dependência de `SPEC-014`; esta SPEC não define tipos nem autoridade concorrentes.
- **Serviços remotos:** emitem evidências autenticadas conforme GMP/contratos. O cliente não converte estado local em confirmação remota nem ACK técnico em atendimento humano.

Autoridade sobre permissão do dispositivo pertence ao sistema e à pessoa. Autoridade de tenant, consentimento remoto, retenção e revogação global depende dos contratos de identidade, segurança, domínio e sincronização. A leitura de dado local não prova que a autorização remota segue vigente.

## 4. Contrato de capacidade e permissões

Para cada capacidade que a jornada solicitar, o adaptador iOS deve representar separadamente suporte conhecido, autorização, disponibilidade operacional e limitação de background. A enumeração comum de estados (`ready`, `unavailable`, `permission_required`, `hardware_unsupported`, `background_limited`) é candidata da arquitetura DICE; enum final e mapping para domínio devem ser acordados com `SPEC-014` e `SPEC-003`.

1. Ao iniciar ou retomar o app, consultar o estado de autorização e capacidade sem pedir permissões preventivamente.
2. Solicitar uma permissão somente quando a pessoa inicia uma ação que dela depende e após explicar a função afetada.
3. Se a pessoa negar, revogar ou restringir a permissão, marcar a capacidade dependente como indisponível/limitada e manter funções independentes utilizáveis.
4. Após retorno do app ao foreground, reavaliar estados que podem ter mudado em Ajustes. Não inferir concessão pela mera abertura de Ajustes.
5. Toda medição de localização leva origem/combinação, instante e qualidade/incerteza segundo o contrato Guardian. Medição ausente, atrasada ou sem autorização não vira coordenada atual.
6. Descoberta de peer ou visibilidade de rede não autoriza conexão, associação ou compartilhamento.

Apple documenta que localização em background depende de autorização e configuração e que apps podem ser suspensos; atualizações podem ser enfileiradas até o app voltar a executar. A autorização e os serviços devem ser escolhidos por necessidade e contextualizados. Ver fontes Apple 1–3. Consequências concretas dependem de API e versão alvo ainda OPEN.

## 5. Ciclo de vida e estados

O modelo abaixo é uma proposta da SPEC para o adaptador, não um enum normativo global. Cada capacidade mantém seu próprio estado; atividades podem coexistir.

| Estado | Significado observável |
|---|---|
| `unknown` | app ainda não obteve evidência suficiente |
| `unsupported` | dispositivo/SO não expõe a capacidade necessária |
| `permission_required` | função bloqueada até ação de autorização |
| `ready` | capacidade e pré-condições atuais foram observadas |
| `background_limited` | execução ou atualização depende de retorno/limite do SO |
| `stale` | última evidência passou do prazo/condição definida pelo contrato; prazo ainda OPEN |
| `unavailable` | condição observada impede a ação no momento |
| `ending` / `ended` | encerramento solicitado / observado pelo adaptador conforme evidência disponível |
| `failed` | falha identificada com causa e camada; sem causa distinguível permanece `unknown` |

Eventos: iniciar, ir ao background, suspensão/interrupção, retomada, mudança de permissão, bloqueio/desbloqueio do dispositivo, alteração de rota, expiração/revogação de atividade e encerramento. O app revalida a autorização e estado antes de iniciar coleta ou replay após retorno. Processo encerrado pelo SO não equivale a callback concluído, atividade encerrada ou dado remoto atualizado.

O iOS suspende a maioria dos apps pouco depois da transição ao background e pode enfileirar atualizações de localização. Core Location oferece caminhos específicos para operações elegíveis e exige comportamento de configuração compatível. Isso não constitui garantia de execução contínua para o GodMesh. Fontes Apple 1–3, consultadas em 2026-11-16.

## 6. Dados locais protegidos e integração FFI

O app distingue dado persistido localmente, mudança em memória, item pendente, envio tentado, ACK técnico e confirmação humana. Gravação local bem-sucedida não significa envio, commit remoto ou sincronização completa. Falha de escrita não pode ser apresentada como persistência durável.

O armazenamento deve aplicar classe de proteção e política de chave definidas por decisão aprovada, por classe de dado. O estado de bloqueio/desbloqueio pode tornar arquivos inacessíveis; Foundation documenta diferenças entre proteção completa e proteção até o primeiro desbloqueio. Nenhuma classe é selecionada nesta versão. Fonte Apple 4 descreve a API, mas não decide qual nível serve aos requisitos GodMesh.

Quando arquivo/chave estiver indisponível, a UI informa que o dado não está acessível no momento, sem revelar conteúdo em cache de apresentação. Não descartar fila ou declarar perda sem política/evidência. Após retomada, sincronização e replay obedecem `SPEC-014` e `SPEC-004`: revalidar autorização, retenção e cursores; cliente não cria mecanismo local de replay concorrente.

A interface FFI deverá futuramente declarar ownership, duração, erro, cancelamento solicitado versus concluído, executor/isolamento, shutdown e distinção entre estado durável e em memória. São contratos pendentes em SPEC-014, submetidos à revisão designada de Alex.

## 7. Rede, conectividade e degradação

O app informa conectividade observada por serviço, rota e frescor. Deve distinguir rede local, internet observada, rota candidata e envio/ACK. A classificação e fonte de transporte seguem `SPEC-003` e `SPEC-007` conforme interface futura; esta SPEC não estabelece enum/wire próprio.

- Rota celular, Wi-Fi de terminal satelital ou indicação de sistema não comprova acesso do GodMesh a serviço satelital nem entrega.
- Se uma troca de operadora, plano ou configuração externa não estiver disponível a API oficial, orientar a pessoa a agir manualmente; resultado permanece desconhecido até confirmação observável.
- Em rede restrita, reduzir primeiro tarefas discricionárias do próprio GodMesh, mantendo estado/encaminhamento prioritário possível e transparente. A ordem conceitual P0–P3 do APP-ARC-001 é proposta; prioridade concreta, empate, capacidade e orçamento são OPEN.
- Ausência de rota, timeout ou resposta parcial preserva pendência e estado incerto. Não marcar mensagem, SOS ou posição como entregue/atual.
- Na recuperação, validar autorização e expiração antes do replay comum e apresentar a transição apenas quando houver evidência da camada correspondente.

## 8. Guardian, SOS e privacidade

Guardian só coleta dentro de sessão voluntária, audiência, propósito e validade autorizados. Revogação local impede novas leituras autorizadas; enquanto offline, o app não pode afirmar que servidor ou cópias remotas receberam a revogação. Atraso de check-in é atraso, não SOS automático (ADR-042).

SOS segue `SPEC-006` e contratos próprios. O cliente pode conservar intenção local e tentativa quando permitido, mas não contorna autorização do SO, não promete transmissão e não afirma que atendimento humano começou. Posição exata fica restrita à audiência autorizada conforme ADR-041 e política de localização; não vai à projeção comum de tela bloqueada.

Mínimos de privacidade da interface:

- não mostrar coordenadas precisas, credenciais, SSID/BSSID privados, identidade civil ou histórico de acesso em tela bloqueada;
- qualquer resumo Live Presence usa conteúdo redigido mínimo e deep link autenticado;
- proteger estado stale, desconhecido, local, técnico e humano sem colapsar estados;
- minimizar registros de capacidade e transição; não armazenar identificadores de rede desnecessários;
- preferências de privacidade podem reduzir exposição, mas não convertem o app em agente sempre ativo.

Política detalhada de classes de dado, retenção, destruição de chave, backup e revogação continua sob `SPEC-020`, `SPEC-002`, `SPEC-005` e `SPEC-014`.

## 9. Live Presence iOS

Live Activity é opcional por atividade em andamento e elegibilidade do SO. Não criar atividade persistente para mera presença “online”. Antes de iniciar, o cliente consulta elegibilidade/autorizações disponíveis e preserva fallback no app. Uma atividade desativada, inelegível ou descartada não altera o estado de domínio.

Dados da projeção: identificador opaco, tipo, estado, prioridade, resumo redigido, momento da atualização, validade quando definida e referência ao evento de origem. Não inclui segredo, coordenada precisa ou token de acesso. A atividade usa `staleDate` somente quando houver regra temporal aprovada; depois da stale date a plataforma considera seu conteúdo desatualizado. A API de ActivityKit fornece estado de stale e mecanismo para consultar se a pessoa permite iniciar atividades. Fontes Apple 5–7.

| Entrada/evento | Exibição esperada |
|---|---|
| Evento autenticado de SOS ativo | Prioridade visual; texto informa estado técnico exato, sem alegar socorro/claim humano não comprovado |
| Guardian válido | Resumo, check-in/frescor conforme evidência; sem posição precisa |
| PTT ativo | Estado da comunicação conforme evento; sem alegar áudio entregue quando há somente transmissão local |
| Sem evento recente / stale date vencida | Marcar estado desatualizado; não repetir como atual |
| Permissão/suporte inelegível ou atividade encerrada pelo SO | Fallback no app/notificação permitida; preservar estados das demais atividades |
| Evento duplicado ou atrasado | Deduplicação/ordem conforme contrato GMP e sync; não ressuscitar encerramento sem regra aprovada |

ActivityKit documenta ordem/relevância de Live Activities e estados próprios, mas o contrato GodMesh de precedência concorrente e eventos atrasados continua OPEN. `priority` interno GodMesh não é promessa de controle da ordem visual do sistema.

## 10. Critérios de aceite documental

Estes critérios definem resultado que o futuro teste deverá observar. Eles não são resultados de execução.

1. **Permissão negada/revogada:** com duas funções, uma dependente e outra independente, negar/revogar a permissão da primeira marca apenas essa capacidade como limitada/indisponível; a segunda permanece utilizável; novo acesso não ocorre sem reautorização.
2. **Retorno de background:** suspender e retomar durante atividade de localização não causa estado atual sem nova evidência; o app mostra valor anterior com idade/estado correspondente ou desconhecido, reavalia a autorização e só então processa nova leitura.
3. **Bloqueio e dados protegidos (parcial/OPEN):** falta definir classes de dados e política de acesso; portanto, não há ainda resultado esperado completo para leitura/escrita/replay em cada estado de bloqueio. Depois da decisão aprovada, testar cada célula da matriz; confirmar que o comportamento segue a classe e que a UI não revela payload inacessível nem afirma perda. Este critério não pode ser considerado atendido enquanto a decisão sobre classes de dados e proteção no §13 estiver OPEN.
4. **Offline e persistência (parcial/OPEN):** abrir sem rede mostra estado offline e dados locais autorizados; envio pendente não vira entregue; erro de armazenamento não vira confirmação de persistência. Ownership, lifetime, cancelamento, threads, binding e tipos finais devem seguir a semântica comum em `SPEC-014` §18.1 e permanecem sem fechamento até decisão explícita do binding e contrato FFI; este critério não prova que a fronteira está completa.
5. **Revogação offline:** revogar Guardian desconectado interrompe novas leituras locais e marca entrega remota como não confirmada; reconexão só altera a situação remota após evidência definida pelos contratos de sync/autorização.
6. **Rota:** indicação de rede ou satélite sem prova de transporte do serviço não marca rota GodMesh utilizável; falha de rota não é inferida de falta de telemetria.
7. **Orientação manual:** quando ação externa não for permitida por API, app mostra instrução adequada e status sugerido/desconhecido; não afirma abertura de Ajustes ou mudança de operadora sem observação/ação confirmada.
8. **Priorização:** sob limitação de bateria/rede, reduzir apenas tarefas próprias não essenciais conforme política aprovada e manter estado de serviços por capacidade; não encerrar serviço do SO nem inferir que SOS será entregue.
9. **Live Presence stale:** ao atingir a stale date definida, conteúdo deixa de ser tratado como atual; fallback permanece e eventos recebidos preservam fase/destino comprovados.
10. **Coexistência:** PTT + Guardian + SOS mantêm estado individual; SOS pode receber destaque sem apagar atividade anterior; claims e resolução humana exigem eventos de autoridade próprios.
11. **Dados bloqueados:** tela bloqueada não revela os campos proibidos na seção 8, inclusive em estado stale/offline.
12. **Cross-platform:** execução desses critérios em outras plataformas não é atribuída a esta SPEC. Windows, Linux desktop, Android e web permanecem no escopo integral do produto e requerem contratos/aceites por seus responsáveis; servidor Linux/macOS não se aplica ao cliente iOS porque não é hospedado por ele. Não aplicabilidade não exclui interação via APIs/serviços que o cliente consome.

Casos de teste formais: `NOT_ASSIGNED`. Execução: `NOT_EXECUTED`. Nenhum limiar numérico foi definido sem política ou medição.

## 11. Casos planejados sem IDs

1. Autorizar Guardian em foreground, iniciar atividade e ir ao background; registrar autorização, estado da capacidade, origem/instante da localização, evento enviado e apresentação.
2. Negar e depois revogar autorização enquanto app roda; confirmar transição da capacidade, ausência de nova leitura e continuidade de funções independentes.
3. Suspender/interromper app com atualização pendente, retomar com e sem rede, verificar stale, fila, idempotência e distinção entre persistido, tentado e ACK.
4. Bloquear aparelho antes do primeiro desbloqueio, após desbloqueio e após bloquear novamente; executar leitura/escrita/replay por classe somente depois que política de proteção for decidida.
5. Perder rota controlada mantendo permissão, restaurar rota e verificar que o app não confundiu falha de transporte com revogação, sem serviço observador inexistente.
6. Mostrar Live Activity elegível e desativada; atualizar, expirar, receber evento atrasado e coexistir com SOS; confirmar stale e fallback.
7. Aplicar limite de energia e rede de fixture autorizada; verificar apenas classes de tarefa que a política final aprovar, sem medir ou alegar economia nesta SPEC.
8. Forçar falha de persistência e cancelamento FFI; observar diferença entre erro, cancelamento solicitado, cancelamento efetivo e operação durável, quando contrato de Alex estiver integrado.

Cada caso requer build, versão/plataforma, fixture, permissões, fonte de evidência e ambiente definidos por responsáveis antes da execução. Nenhum foi executado; IDs só serão atribuídos pelo índice de Ada após definição formal.

## 12. Compatibilidade e aplicabilidade

| Alvo | Aplicabilidade |
|---|---|
| iOS | Escopo principal; versão mínima, aparelhos e APIs suportadas OPEN. Atividades e sensores condicionados a SO, dispositivo e permissão. |
| macOS arm64/x86_64 | Produto suportado em outra superfície, mas fora desta SPEC iOS. Requer especificação própria do cliente macOS; FFI comum pode ser dependência de Alex. |
| Android | Fora do escopo iOS; coberto pela SPEC-015 atribuída a Chet. |
| Windows/Linux desktop e web | Escopo do produto; fora desta SPEC e exige aceites próprios de Lea/outros donos. Não inferir equivalência com iOS. |
| Servidor Linux | Não hospeda cliente iOS. Continua sujeito a SPECs de serviço e integração no escopo geral. |

As afirmações sobre APIs Apple foram conferidas em documentação oficial disponível em 2026-11-16. Não escolhemos deployment target ou versão mínima. As fontes são documentos online sem versão semântica publicada nas páginas consultadas; os links e data de consulta devem ser conferidos novamente antes da aprovação da SPEC e seleção de stack.

## 13. Decisões OPEN

| Questão | Responsável proposto | Alternativas e consequência | Fechamento necessário |
|---|---|---|---|
| Classes de dados e proteção no bloqueio/primeiro desbloqueio | Kim, Bruce, Michael; produto decide finalidade | Maior proteção pode impedir fila/replay após bloqueio; acesso mais amplo amplia dados disponíveis em background. Não escolher nível nesta SPEC. | Matriz dado × estado × read/write/replay aprovada e compatível com retenção/backup; revisar fontes Apple. |
| Lifecycle Swift–Rust, threading, cancelamento e shutdown | Alex, revisão iOS Donny | Callback, async sequence, polling e cancelamento têm semânticas distintas; seleção depende do binding e compatibilidade oficial. | Contrato comum define semântica abstrata em SPEC-014 §18.1; binding, ABI, tipos finais, representação, modelo de threads e funções exportadas continuam OPEN. Fechamento requer decisão aprovada e revisão independente. |
| Sync e revogação offline | Martin Kleppmann, Kim; dependências Michael/Graydon | Revalidar, rejeitar ou retirar item da fila conforme autoridade; sem regra pode vazar ou descartar operação. | Política e transições reconciliadas em SPEC-004/SPEC-002 e mapeadas nesta SPEC. |
| Frescor, expiração e cadência de Guardian/Live Presence | Margaret, Nancy, Lea e responsáveis de domínio | Prazo/cadência altera energia e risco de conteúdo obsoleto; não fixar número sem evidência. | Política aprovada por atividade e destinatário; contrato inclui origem, tempo e estado stale. |
| Eventos concorrentes, replay e precedência SOS/PTT/Guardian | Nancy, Martin Kleppmann, Graydon; produto | Ordenação local/servidor e prioridade de UI podem divergir. | Contrato define fonte/ordem e casos de evento atrasado, duplicado, revogado e resolvido; validar sem colapsar ACK técnico/humano. |
| Transporte e estado de rota | Vint, Graydon, Brendan | Rota observada pode não transportar serviço; status desconhecido é mais honesto que inferência. | Fontes/versões e método de observação registrados por campo; sem depender de telemetria perturbada. |
| Prioridade e energia adaptativa | Margaret, Lea, Brendan, Alex e Donny | Perfil estático vs adaptativo; custo sem medição é desconhecido. | Política por serviço e método futuro reproduzível aprovados; limiares continuam OPEN até dados autorizados. |
| Alvos oficiais Apple e matriz de compatibilidade | Donny, Alex e Alan | Adotar só APIs compatíveis com plataformas/alvos decididos; sem downgrade silencioso. | Confirmar versões e documentação oficial para cada API antes de aprovar. |

## 14. Dependências e divergências

- **SPEC-014, Alex:** FFI, offline e persistência. R-CH-001 de Chet corrigiu o aceite da SPEC-014 para exigir revisão Rust–Swift de Donny; Android/Kotlin só em artefato próprio ou interface compartilhada explicitamente delimitada. A versão atual consultada, SHA-256 `e16b4a252e4713374d05575dd725280b1eb451558b15c3dd55ed91ffa79c8670`, §18.1, estabelece ownership/lifetime abstratos e distinção de cancelamento como contrato comum, mas mantém binding, ABI, tipos, serialização e threads OPEN. A revisão iOS de Donny na contribuição de Alex mantém os quatro itens como disposições textualmente OPEN, não resoluções técnicas; contribuição revisada SHA-256 `d9f22f920222343d50125a193e9e6ed78c8bb4c8718e13f23fc56d85bd7b0b6a`.
- **SPEC-006, Nancy:** autorização, estados e ACK de SOS; não declarar atendimento humano por estado de UI.
- **SPEC-004, Martin Kleppmann:** outbox/inbox, cursores, replay, idempotência e revogação em reconexão.
- **SPEC-002 e SPEC-020, Kim/Bruce:** identidade, autorização, privacidade, chaves e retenção.
- **SPEC-003, Graydon; SPEC-007, Radia/Vint:** eventos e transporte; estado de rota é evidência separada de entrega.
- **SPEC-012, Margaret; SPEC-013, Lea:** jornadas, texto, acessibilidade, fallback e prioridade visual.
- **SPEC-018, Brendan:** sinais, evidência e metodologia; revisão INT-ADA-001 não prova que observadores existem.

### Parecer independente de Alex Crichton

**Versão revisada:** DRAFT-0.1, SHA-256 `0d57d3c54e44c2ad4c30f55d80927960354f5075243d3fb054b681b6a504b8e1`. Não houve conflito direto com SPEC-014 no fluxo local/FFI e lifecycle. Alex registrou dois limites que permanecem OPEN:

1. A revisão da fronteira iOS/Swift não fecha ownership e cancelamento da FFI enquanto `SPEC-014` não definir/decidir esse contrato e o binding. O critério 4 desta SPEC só está cumprido no aspecto de existência do parecer; a dependência técnica segue aberta.
2. O cenário de bloqueio/dados protegidos do critério 3 é incompleto até decidir classes de proteção. Continua explicitamente parcial/OPEN; os efeitos distintos de `FileProtectionType` não selecionam política GodMesh.

O mesmo parecer considera as fontes Apple citadas compatíveis com suspensão/enfileiramento de Core Location e diferenças de FileProtection, sem validar implementação, alvo ou versão. Casos formais seguem `NOT_ASSIGNED` e observado `NOT_EXECUTED`. Esta integração não representa aprovação.

**Conferência de integração:** Alex reexaminou DRAFT-0.2 no hash `972ba80e05ba0be379687c3f289c56634088e64b007bfd9dbd0cd0a1df0c4019` e confirmou que as duas ressalvas acima foram representadas fielmente, sem nova divergência Rust–Swift/offline. O parecer confirma a fidelidade do registro das ressalvas, não fecha FFI ou política de proteção.

Não encontrei conflito direto entre as fontes normativas listadas e as ressalvas integradas. Direções de plataforma e produto são aprovadas em ADRs, porém detalhes de API, ciclo de vida, compatibilidade, prioridades de eventos, retenção e limites seguem OPEN. O registro de uma ressalva não a fecha.

## 15. Critério de fechamento da SPEC

Esta SPEC só pode avançar para revisão/aprovação quando: (a) IDs e subescopos forem confirmados no índice; (b) Alex revisar FFI/persistência e o contrato de ownership/cancelamento/binding de SPEC-014 for definido, ou sua permanência OPEN for justificada como não bloqueadora; os quatro OPENs iOS devem ter decisão/contrato ou permanência igualmente justificada; (c) alvos e APIs oficiais Apple forem versionados; (d) políticas de Guardian, SOS, energia, privacidade e revogação forem conciliadas; (e) casos por requisito e plataforma tiverem pré-condição, estímulo, resultado, evidência prevista, dependência e ID central atribuído, incluindo conclusão da matriz de proteção para considerar completo o caso 3. Mesmo atendidos, esses itens não significam implementação, build, teste ou validação.

## 16. Fontes

### Documentação do projeto

- `docs/README.md` §§ regras de prevalência e plataformas. Arquitetura de referência e ADRs prevalecem.
- `docs/01-arquitetura/arquitetura-referencia.md` §§ arquitetura de cliente e princípios de capacidade (versão consultada no snapshot 3.8.0, 2026-11-15).
- `docs/06-planejamento/decisoes.md` ADR-041, ADR-042, ADR-045, ADR-047, ADR-052 e ADR-055. Direções aprovadas, detalhes de suporte/implementação pendentes.
- `docs/06-planejamento/pendencias.md` §§ compatibilidade, lifecycle, armazenamento e critérios de validação.
- `docs/06-planejamento/matriz-rastreabilidade.md` §§ aceites por RF, cobertura, C13 e `GMD-A18-001`.
- `docs/00-produto/requisitos.md` RF-029, RF-031–034, RF-042–043 e RNF-016.
- `docs/01-arquitetura/cliente-mobile-sensores-conectividade.md` §§ matriz, avaliação de capacidades, Guardian e contingência.
- `docs/01-arquitetura/aplicativo-operacao-normal.md` §§ 3–8.
- `docs/01-arquitetura/live-presence.md` §§ 1–5; `docs/07-contratos/live-presence.md` §§ evento e política de projeção.
- Contribuições e pareceres consultados estão listados na seção 14, com SHA-256 observado.

### Documentação oficial Apple

Consultada em 2026-11-16; documentação online sem versão semântica exibida na página, portanto precisa de nova conferência para os alvos escolhidos.

1. [Requesting authorization to use location services](https://developer.apple.com/documentation/CoreLocation/requesting-authorization-to-use-location-services): solicitação contextual, níveis de autorização e comportamento em background/retomada.
2. [Handling location updates in the background](https://developer.apple.com/documentation/corelocation/handling-location-updates-in-the-background): suspensão de apps, enfileiramento e requisitos de processamento em background.
3. [Accessing the device’s location efficiently](https://developer.apple.com/documentation/xcode/accessing-the-device-s-location-efficiently): reduzir frequência de localização para limitar energia.
4. [FileProtectionType.complete](https://developer.apple.com/documentation/foundation/fileprotectiontype/complete) e [completeUntilFirstUserAuthentication](https://developer.apple.com/documentation/foundation/fileprotectiontype/completeuntilfirstuserauthentication): diferenças de acesso sob bloqueio/primeiro desbloqueio; a SPEC não seleciona um nível.
5. [ActivityAuthorizationInfo](https://developer.apple.com/documentation/activitykit/activityauthorizationinfo): consulta de autorização/ativação para Live Activities.
6. [Displaying live data with Live Activities](https://developer.apple.com/documentation/activitykit/displaying-live-data-with-live-activities): conteúdo e atualidade por `staleDate`.
7. [ActivityState.stale](https://developer.apple.com/documentation/activitykit/activitystate/stale): estado de conteúdo desatualizado.

## Mudanças desta versão — ADR-056, ADR-059 e ADR-060

**ADR-056 — compatibilidade:** suporte declarado por versão e perfil versionado de cliente, com janela N/N-1 dentro do perfil publicado. Perfis concretos e janela de calendário permanecem OPEN até decisão de produto. Servidor rejeitando versão/feature incompatível deve fazê-lo antes de qualquer efeito e sem downgrade silencioso; a UI informa incompatibilidade e requisito de versão/perfil somente com dados fornecidos pela resposta. Não repetir mutação incompatível; falta de resposta conclusiva é desconhecido.

**ADR-059 — offline:** únicas operações autorizadas: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) mensagem a canal já autorizado; (4) observação local de localização; (5) PTT apenas em rede local com grant de floor válido. Negar sempre localização precisa divulgada, mutação federada, criar/alterar grant, mudança de identidade, transmissão RF e transcrição remota. Exigir epoch comparável, frescor dentro do limite do perfil, ausência de revogação conhecida e anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. Frescor numérico desconhecido até medição.

**ADR-060 — confidencialidade:** expor `confidentiality` por canal e por perna; bridge, ASR e gravação que terminem a proteção são marcados `terminated`. Não rotular E2EE sem prova para canal/perna. Tela bloqueada não exibe posição precisa nem segredo.

- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- **casos por requisito e casos P desbloqueados (C3-1):** subseção `Casos por requisito (P/N/B/F)` ao final, uma linha por requisito do §2; `S016-056-P1`, `S016-059-P1` e `S016-062-P1` reescritos para o comportamento com parâmetro ausente (`param_not_configured`), sem inventar janela, frescor ou prazo; novo `S016-061-B1` para a borda da ADR-061.

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
| S016-062-P1 | ADR-062 | Política por classe existente para a coleta, com prazo ou gatilho não decidido | Coleta e gravação | Gravação aceita com `retention_policy_ref` e `retention=param_not_configured`; nenhum prazo ou gatilho exibido como aprovado; minimização de registros de capacidade e transição mantida | P | NOT_EXECUTED |
| S016-062-N1 | ADR-062 | Classe sem política aprovada | Tentativa de persistir histórico ou dado correlacionável | Falha fechada com erro explícito; nenhum dado persistido; UI declara indisponibilidade | N | NOT_EXECUTED |
| S016-062-F1 | ADR-062 | Política expirada; cópias incluem arquivo protegido, cache, backup e derivados de projeção | Gatilho de descarte | Descarte por cópia registrado; cópia sem confirmação pendente; nenhuma remoção global alegada | F | NOT_EXECUTED |
| S016-062-B1 | ADR-062 | Hold com autoridade, escopo e revisão | Consulta comum ao dado retido | Dado retido sob prazo próprio; acesso comum não reativado | B | NOT_EXECUTED |
| S016-063-P1 | ADR-063 | Build iOS assinado, com SBOM e `generation` igual ou superior ao piso | Instalação limpa/atualização | Verificação aprovada antes de executar; versão, digest e `generation` registrados e exibidos | P | NOT_EXECUTED |
| S016-063-N1 | ADR-063 | Artefato com `generation` inferior ao piso (versão vulnerável) | Tentativa de instalar versão anterior | Rejeitada; versão atual mantida; motivo `generation_below_floor` exibido; nenhuma execução | N | NOT_EXECUTED |
| S016-063-F1 | ADR-063 | Assinatura inválida ou chave revogada na lista verificada | Aplicação do artefato | Rejeição com erro tipado; nenhum fallback; estado `failed` com causa e camada | F | NOT_EXECUTED |
| S016-063-B1 | ADR-063 | Atualização N-1 dentro da janela, mas abaixo do piso | Instalação da atualização | Rejeitada; piso prevalece sobre a janela; motivo e auditoria registrados | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** classes de dado e proteção no bloqueio (matriz §13 OPEN) impedem caso positivo de leitura/escrita/replay por classe — os casos de bloqueio aqui verificam apenas ausência de revelação. Frescor e stale date de Guardian/Live Presence seguem OPEN, então nenhum caso usa prazo temporal aprovado. Alvos mínimos de iOS e deployment target são OPEN, então não há caso por versão concreta de SO.

### Casos por requisito (P/N/B/F)

Uma linha por requisito do §2. Células citam casos da tabela `Casos por decisão` ou ID novo descrito na Nota; prefixo `S016-R###` agrupa os quatro tipos do mesmo requisito. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. Alvos mínimos de iOS e deployment target seguem OPEN, portanto nenhum caso usa versão concreta de SO.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-029 | S016-R029-P | S016-R029-N | S016-R029-B | S016-060-B1 | P: leitura de sensor com origem, frescor e incerteza. N: indisponibilidade não inventa medida nem direção. B: sensor presente com permissão negada → `permission_required`, nunca `unsupported`. F: revogação suspende só a função dependente com motivo. |
| RF-031 | S016-R031-P | S016-R031-N | S016-R031-B | S016-056-F1 | P: estado de rede com fonte e instante. N: satélite detectado não implica rota GodMesh utilizável. B: IPv6 e IPv4 coexistem com resultados independentes. F: API indisponível deixa `unknown` e orienta ação manual. |
| RF-032 | S016-R032-P | S016-R032-N | S016-R032-B | S016-R032-F | P: orientação manual apresentada quando automação não existe. N: abrir ajustes não comprova ativação. B: usuário dispensa a orientação sem efeito. F: permissão negada/API ausente → instrução e estado não verificado. |
| RF-033 | S016-059-P1 | S016-059-N1 | S016-059-B1 | S016-R033-F | F: carga discricionária cede primeiro e comunicação de emergência do SO permanece acessível; orçamento e ordem fina OPEN. |
| RF-034 | S016-R034-P | S016-R034-N | S016-R034-B | S016-061-F1 | P: áreas e modos simultâneos utilizáveis offline com estado local. N: capacidade ausente limita só a função dependente. B: encerramento pelo SO durante uso mantém estado conhecido. |
| RF-042 | S016-R042-P | S016-060-N1 | S016-063-B1 | S016-061-F2 | P: Live Activity só para atividade em andamento elegível. N: tela bloqueada não expõe posição precisa nem segredo. B: sem Live Activities exigíveis → notificação/fallback no app. F: `stale`/`failed` exibidos sem converter falha de superfície em falha do domínio. |
| RF-043 | S016-R043-P | S016-058-N1 | S016-058-B1 | S016-061-F1 | P: prioridade preservada com estado real por classe de ACK. Metas de energia OPEN; nenhum caso mede SLO energético. |
| RNF-016 | S016-058-P1 | S016-058-N1 | S016-058-B1 | S016-058-F1 | Projeção não anuncia fase/estado sem evidência; classe exibida é a recebida (ADR-058). |
