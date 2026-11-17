# SPEC-014 — Cliente offline e fronteira FFI

**Versão:** DRAFT-0.4  
**Estado:** DRAFT / OPEN — não aprovada; nenhuma implementação ou validação é afirmada.  
**Autor documental:** Alex Crichton  
**Revisor independente designado:** Donny Wals; revisão desta versão pendente.  
**Data:** 2026-11-17  
**Requisitos cobertos:** RF-043 (principal); subescopos de estado local relacionados a RF-034 e RNF-017, sem substituir os contratos de produto, autorização, Wi-Fi ou sincronização.  
**Contribuição-base:** `SPECS-EM-ANDAMENTO/contribuicao-alex-crichton.md`, SHA-256 `29c9d11ac89c132da5033776b34dfbf36e653d5033d281adc24cedc7dc298ed2`.  
**Pareceres incorporados:** Donny Wals, `contribuicao-donny-wals.md`, SHA-256 `d9f22f920222343d50125a193e9e6ed78c8bb4c8718e13f23fc56d85bd7b0b6a`; Chet Haase, `revisao-chet-alex-crichton.md`, SHA-256 `61eaa4532fd3a1d140ba5f8079b955550d599bef7b8b7efd3ffa55253b80241c`.

## 1. Propósito e limites

Esta SPEC define a proposta comportamental para o estado local/offline do cliente e a fronteira de chamadas locais entre núcleo Rust e cliente Swift. A intenção é preservar operações locais permitidas e comunicar corretamente incerteza, autorização, durabilidade e sincronização. A SPEC não especifica engine de armazenamento, esquema persistido completo, criptografia, chaves, política de retenção, algoritmo de sync, binding definitivo ou versões mínimas de sistema operacional.

A implementação planejada do núcleo é Rust; macOS universal (arm64/x86_64) e iOS usam SwiftUI. Outras plataformas de cliente permanecem no escopo integral do produto. Android/Kotlin tem contrato próprio em SPEC-015; não é critério de aceite nem está validado por esta SPEC. A API HTTP/OpenAPI é fronteira remota separada e não substitui FFI.

## 2. Fontes e precedência

As citações abaixo apontam para a árvore de documentação do projeto; títulos de seção são localizadores quando a paginação variar.

| Fonte | Trecho/localizador | O que sustenta e limite |
|---|---|---|
| `docs/README.md` | Regras de prevalência, itens 1, 6–8 e 20 | Arquitetura de referência e ADRs prevalecem; produto integral; especificação não prova funcionalidade; estado offline honesto e prioridade SOS sem promessa de entrega. |
| `docs/00-produto/requisitos.md` | RF-043, linha 77; RF-034 na tabela de requisitos; RNF-017, linha 85 | RF-043 está marcado APROVADO como direção de produto; RNF-017 exige teste sob partição regional. Não comprova aceite de parâmetros nem execução. RF-034 permanece requisito relacionado, não é apropriado integralmente por esta SPEC. |
| `docs/01-arquitetura/aplicativo-operacao-normal.md` | seções de estados, inicialização, offline/energia e critérios de aceite | Exige modos coexistentes, início offline e comunicação sem alegar execução sem evidência. Continua sendo arquitetura proposta. |
| `docs/01-arquitetura/replicacao.md` | autoridade, partições, replicação e recuperação | Dados privados são replicados por escopo/política; cópia isolada não é autoridade global. Sync lógico permanece sob SPEC-004. |
| `docs/07-contratos/api-inicial.md` | API/GMP inicial | Insumo para operações remotas; não define persistência de cliente nem ABI local. |
| `docs/01-arquitetura/arquitetura-referencia.md` | Plataformas e planos | Core Rust e separação de plataformas; status arquitetural não é validação de compatibilidade. |
| `docs/06-planejamento/decisoes.md` | ADR-047, ADR-053 e ADR-055 | Direções e plataformas conforme estado registrado; seleção concreta de binding, deployment target e compatibilidade permanecem abertas. |
| `docs/06-planejamento/matriz-rastreabilidade.md` | cobertura individual RF-043 e RNF-017; GMD-A18-001 | Propostas de aceite e autoria são propostas. RF-043 associa energia/prioridade; RNF-017 tem Kleppmann como integrador e subescopos explicitados. GMD-A18-001 segue parcial. |
| `SPECS-EM-ANDAMENTO/contribuicao-alex-crichton.md` | SPEC-LC-01, SPEC-FFI-01 e integrações | Rascunho de autoria e propostas, não fonte normativa nem aprovação. Hash no cabeçalho desta SPEC. |
| Parecer Donny | “Revisão independente — contribuição de Alex Crichton” | Quatro lacunas iOS: proteção por estado do aparelho; lifecycle/cancelamento; compatibilidade versionada; corrida revogação/fila. Resolvidas textualmente como OPEN, não tecnicamente. |
| Parecer Chet | R-CH-001 | Aceite Rust–Swift não pode exigir revisão Android sem artefato. Critério corrigido: Donny revisa iOS/Swift; Android Kotlin é SPEC-015. Revisão documental não aprova binding. |

### 2.1 Estado da evidência

Os documentos de projeto foram lidos como especificação/decisão documental. Fontes de binding citadas na nota técnica anterior foram consultadas em 2026-11-16 e são candidatos, não prova de compatibilidade do GodMesh. Não foram verificados código do produto, APIs existentes no repositório, builds, aparelhos, persistência real, serviços, execução ou testes. Toda afirmação técnica de plataforma que dependa de API ou versão precisa ser reconfirmada por Donny/Chet nas SPECs específicas e com fontes oficiais versionadas.

## 3. Requisitos, produtos e cobertura

| Requisito | Relação com esta SPEC | Estado e limite |
|---|---|---|
| RF-043 | Principal: manter prioridades operacionais/energia adaptativa e comunicação essencial sem controle de apps de terceiros; esta SPEC cobre estado local e limites de execução do cliente relevantes a isso. | Direção aprovada no requisito; aceite comportamental abaixo é proposta. Metas de energia, latência e disponibilidade não foram aprovadas aqui. |
| RF-034 | Subescopo: início, estados e capacidades do app sem conexão; experiência/jornada completa pertence à SPEC-012 e interfaces à SPEC-013. | Não reivindico cobertura integral de RF-034. |
| RNF-017 | Subescopo cliente: conexão local versus sync regional, autorização/retensão antes de servir/replay após partição; eventos, cursores e recuperação lógica pertencem à SPEC-004, dados/isolamento à SPEC-005 e Wi-Fi/AP à SPEC-009. | Não reivindico cobertura integral do teste sob partição, nem disponibilidade de AP, revogação ou sync completos. |
| RF-042/RNF-016 e outros | Só como consumidores de estados confirmados/pendentes quando projeção local é relevante; seus contratos de Live Presence/SOS permanecem em SPEC-006/012/016. | Sem criar semântica paralela para SOS, ACK humano ou presença. |

Aplicabilidade de produto: Personal, Teams, Node e Platform/SDK quando houver cliente local e operações persistentes autorizadas. Uma biblioteca sem armazenamento ou cliente remoto não herda automaticamente os requisitos de UI/energia. Aplicabilidade por modalidade hospedada/autogerida depende do fluxo de dados e do ator, não da marca do produto.

## 4. Atores, autoridade e fronteiras

- **Usuário do cliente:** inicia ações, concede/revoga consentimentos conforme política aplicável e recebe estados legíveis.
- **Cliente Swift:** apresenta disponibilidade local, conectividade observada, frescor e pendência; não concede autoridade regional nem confirma ACK remoto.
- **Núcleo Rust:** fonte de semântica local compartilhada definida por contrato; não substitui autoridade remota para decisões que exigem coordenação.
- **Serviço/autoridade regional:** autentica e decide operações remotas conforme contratos de identidade, autorização e GMP.
- **Componente de sync:** responsável por outbox/inbox, cursores, replay, fencing, ordenação e idempotência lógica; esta SPEC consome seu contrato e não redefine esses mecanismos.
- **Sistema operacional/plataforma:** pode limitar sensores, armazenamento protegido, background e ciclo de vida. A SPEC não promete execução contínua nem contorno dessas restrições.

FFI é uma chamada dentro do processo do cliente; não introduz processo auxiliar. IPC em iOS não é proposto como alternativa equivalente sem fonte oficial específica que comprove viabilidade para o cenário.

## 5. Modelo de estado local

A implementação deve representar separadamente, no mínimo, estas dimensões conceituais; nomes e codificação são OPEN até conciliação com SPEC-004, SPEC-005 e SPEC-015/016:

| Dimensão | Valores semânticos mínimos | Regra |
|---|---|---|
| Conectividade | desconhecida, indisponível, rota observada, serviço alcançável quando comprovado | Rota observada não prova serviço alcançável nem entrega. |
| Disponibilidade local | ausente, somente memória/não durável, persistido localmente, inacessível por proteção/erro | Não declarar persistido sem confirmação da gravação. |
| Autorização | desconhecida, válida segundo evidência vigente, negada/revogada/expirada | Cópia local não prova autorização atual; quando a validade não pode ser determinada, manter desconhecida e restringir dado sensível. |
| Sincronização | não submetido, pendente local, enviado/tentado, confirmado pelo destino/camada identificada, rejeitado, resultado desconhecido | Cada estado requer evidência da camada indicada; timeout não é rejeição nem sucesso. |
| Frescor | atual segundo fonte/validade definida, desatualizado, desconhecido | Não inventar idade/validade quando fonte temporal ou política não foi decidida. |
| Capacidade da plataforma | disponível, indisponível, permissão necessária/negada, limitada pelo lifecycle, desconhecida | Avaliar por capacidade; uma permissão ausente não desabilita funções independentes. |

Transições entre estados devem preservar eventos/intenções sem promover desconhecido a sucesso. A máquina de estados concreta e serialização interoperável ficam OPEN; estas dimensões não autorizam escolher enum ou schema persistido por analogia.

## 6. Contrato comportamental de persistência offline

### 6.1 Inicialização e leitura

**Pré-condições:** cliente tem identidade/contexto local configurado; política da classe permite leitura local; dados e proteção de chave estão disponíveis. Se tais condições não puderem ser comprovadas, a aplicação pode abrir em estado reduzido, mas não deve expor dado protegido.

**Estímulo:** inicialização sem rede ou leitura de estado local.

**Resultado esperado:** iniciar sem depender de resposta remota quando os dados exibidos forem locais e autorizados por política; rotular origem/frescor conforme evidência; expor conectividade e sync separadamente. A leitura falha de forma explícita quando dado expirou, autorização local conhecida foi revogada ou chave/armazenamento está indisponível.

**Invariante:** “disponível localmente” não significa “atual”, “autorizado agora”, “visível regionalmente” ou “entregue”.

### 6.2 Mutação local

**Pré-condições:** operação de domínio foi declarada offline-capaz; finalidade e autorização local permitem enfileirar; chave/armazenamento necessários estão acessíveis. A lista de operações permitidas ainda depende dos domínios.

**Estímulo:** usuário solicita operação durante desconexão.

**Resultado esperado:** se contrato permitir, gravar intenção durável e idempotente segundo chave/semântica definida pelo domínio, retornando ao cliente estado “pendente local” apenas após confirmação da persistência. Se armazenamento falhar, a interface deve indicar não durável/erro e não prometer recuperação após encerramento.

**Negativo:** operação que requer autoridade online é recusada ou apresentada como não submetida; ausência de rede não converte tentativa em aceitação remota. Tentativa em memória não é persistência.

**Invariante:** persistência local não é commit remoto, autorização regional, envio, entrega ou atendimento humano.

### 6.3 Reconexão e reconciliação

**Pré-condições:** conexão/serviço e identidade são reavaliados; autorização e retenção são verificadas por autoridade/contrato aplicável; componente de sync fornece regras de replay e confirmação.

**Estímulo:** retorno de conectividade após partição.

**Resultado esperado:** revalidar autorização e validade dos dados antes de servir/enviar conteúdo sensível; reconciliar somente conforme sequência, cursor, idempotência e fence definidos na SPEC-004. Apresentar resultado por camada e destino com evidência recebida.

**Negativo:** se autorização, frescor, cursor, destino ou resultado forem desconhecidos, manter estado desconhecido/pendente e não declarar revogação global, entrega ou sync concluído. Resposta tardia/duplicada não ressuscita operação sem regra de versão e ordenação do contrato comum.

**Recuperação:** retomada após erro volta a avaliar estado, autorização e cursor, sem saltar lacunas. Remoção de fila por revogação e tratamento de operação já em trânsito dependem de decisão comum de Kim/Martin/Graydon; não são determinados aqui.

### 6.4 Falhas locais e recuperação

Para falta de espaço, corrupção, falha de leitura/escrita, chave indisponível, reinício, reinstalação, migração e relógio sem confiança, o cliente não declara dado recuperado nem elimina intenção silenciosamente. A política deve distinguir cache descartável de fila/estado necessário; classe, comportamento e recuperação ficam OPEN. A interface mantém visível o que está perdido, inacessível, não durável ou aguardando intervenção, conforme evidência disponível.

## 7. Contrato FFI Rust–Swift

### 7.1 Requisitos de fronteira

- Cada operação publicada define tipos de entrada/saída e a semântica de ausência, desconhecido e erro; nenhuma coerção silenciosa de estados.
- Para cada buffer/handle/objeto, declarar proprietário, lifetime, regra de clone/liberação, validade após cancelamento e teardown.
- Erros distinguem rejeição de domínio/autorização, indisponibilidade, corrupção, cancelamento, timeout/resultado desconhecido e falha interna; detalhe privado não é exposto em mensagem/log público.
- Async define executor, callback/retorno, ordenação, concorrência e se cancelamento foi solicitado, observado ou concluído. Cancelar espera no wrapper não prova cancelamento da operação de domínio.
- Definir threads permitidas, reentrância, segurança de chamadas simultâneas e passagem para executor/actor de UI; callbacks não presumem main thread.
- Panics não atravessam a ABI como erro recuperável. Shutdown encerra recursos sem converter operação incompleta em sucesso.
- Versão do binding, biblioteca e representação persistida são contratos distintos; migração de formato requer decisão/versionamento compatível com SPEC-005.

### 7.2 Estado e alternativa tecnológica

UniFFI (versão/capacidades registradas na nota de compatibilidade de 2026-11-16) e C ABI manual são candidatos, não decisão. Permanecem incompatibilidades/desconhecidos sobre async/cancelamento efetivo, Swift 6, matriz iOS, slices macOS arm64/x86_64, toolchain, linking e deployment target. A nota de compatibilidade Rust–Swift é material de pesquisa sujeito a revisão por versão, não garantia normativa. Decidir o binding somente após comparar as necessidades do contrato e versões oficiais aplicáveis aos alvos efetivamente escolhidos.

Android/Kotlin está fora do aceite desta SPEC. Chet revisa contrato de Android em SPEC-015; uma interface comum do core só pode ser revisada por Chet em recorte explicitamente identificado, sem que isso valide binding Kotlin. Donny revisa a fronteira Swift/iOS desta SPEC.

## 8. Plataformas e aplicabilidade

| Plataforma | Aplicabilidade desta SPEC | Limites/pendências |
|---|---|---|
| Servidor Linux em contêiner | O núcleo Rust pode participar de semântica compartilhada; persistência e sync do servidor têm SPEC-004/005/018/019/021. | Esta SPEC não define storage servidor, readiness ou restore. |
| macOS arm64 e x86_64 | Cliente Swift pode consumir a fronteira FFI e estado local. | Nenhum slice, build, deployment target ou integração está validado; critérios futuros devem incluir os dois alvos se ambos seguirem aprovados. |
| iOS | Cliente SwiftUI, sujeito a permissões, proteção de dados e lifecycle do sistema. | Donny é revisor. Bloqueio, primeiro desbloqueio, background, término/reinício, chave e backup precisam de matriz por classe/operação; sem prometer execução contínua. |
| Windows | Produto prevê cliente desktop; esta SPEC cobre apenas semântica compartilhada do estado local se o core Rust for consumido. | Binding/armazenamento nativo e lifecycle requerem contrato de plataforma específico; aplicabilidade comportamental ainda deve ser mapeada por Lea/Alan. |
| Linux desktop | Igual à semântica compartilhada do cliente Rust, se aplicável. | UI, proteção de chave, lifecycle e binding não definidos nesta SPEC; não implica exclusão do produto. |
| Android | Semântica compartilhada de requisitos offline poderá ser referenciada. | Binding Kotlin, lifecycle, permissões e critérios Android pertencem a SPEC-015; não cobertos/validados aqui. |
| Web | Estado de operações observável pela interface web é consumidor de contratos remotos. | Persistência local de cliente web não é assumida; comportamento e limitações precisam de SPEC-013/contrato web. Não afirmar execução background persistente. |

## 9. Segurança, privacidade e consentimento

Aplicar tenant, finalidade, consentimento, escopo e retenção definidos pelos contratos competentes. Minimizar payload em logs e telemetria; não inserir conteúdo, credenciais, coordenada precisa ou prova reutilizável nos registros de estado. Revogação offline só permite afirmar a mudança local conhecida; exclusão remota exige confirmação por cópia/autoridade conforme política. Backup, dados protegidos, expiração, destruição de chaves, retenção offline e relógio confiável são OPEN para Kim, Bruce, Michael, Martin e decisão de produto/usuário. Não se fixa valor quantitativo nem classe de criptografia nesta SPEC.

## 10. Critérios de aceite documental

A SPEC só pode sair de DRAFT quando os critérios abaixo forem satisfeitos e revisados; isso não significa autorização de implementação:

1. Matriz aprovada por domínio das operações que podem ler/gravar/replay offline, por classe de dado, finalidade, autoridade e plataforma.
2. Cada transição de estado tem evidência de origem, destino, tempo/frescor quando disponível, e comportamento ao desconhecido; sem transição inferida por silêncio.
3. Contrato SPEC-004 define cursor, idempotência, revalidação, revogação/fila, fence, replay e resultado parcial usados pelo cliente; SPEC-005 define durabilidade, isolamento, retenção, migração e recuperação.
4. Matriz FFI especifica ownership/lifetime, erros, async, cancelamento, threads, callback e shutdown para cada operação identificada, com decisão de binding explicitamente aprovada e compatibilidade documentada para alvos escolhidos.
5. Matriz de iOS por classe × estado de proteção × operação cobre bloqueio/desbloqueio, primeiro desbloqueio quando aplicável, suspensão/término/reinício, backup e disponibilidade de chave, com fonte Apple oficial/versionada e revisão de Donny.
6. Critério desta SPEC exige revisão Donny para Swift/iOS. Revisão Android/Kotlin não é critério de aceite; Chet revisa SPEC-015 ou interface comum identificada, sem validar o binding Android por esta peça.
7. RF-043 está ligado a comportamento de prioridade/energia sem metas inventadas e reconciliado com SPEC-012/013/018. RNF-017 permanece parcialmente dependente de SPEC-004/005/009 e de teste regional futuro.
8. Matriz de aplicabilidade deixa explícito para cada plataforma/produto se o comportamento se aplica e por quê; limitações não são interpretadas como exclusão global.
9. Toda decisão OPEN tem responsável, alternativas, consequência e evidência/condição de fechamento registrados no §12.

## 11. Casos planejados (não executados)

Não existem IDs concretos de caso atribuídos nesta contribuição consolidada. Casos formais: `NOT_ASSIGNED`. Execução observada: `NOT_EXECUTED`. Os cenários seguintes são intenções de cobertura e precisam ser convertidos em casos com identificador, fixture, ambiente, versão, evidência e revisão antes de qualquer execução autorizada.

| Cenário planejado | Estado inicial e estímulo | Resultado/invariante | Negativo/recuperação | Aplicabilidade |
|---|---|---|---|---|
| Inicialização desconectada | Dados locais disponíveis; serviço sem rota; iniciar cliente | Inicia sem declarar conectividade/sync; distingue dado local e frescor | Chave/dado indisponível resulta em acesso negado/estado explícito; reconectar não confirma sync automaticamente | clientes Personal/Teams; Node conforme fluxo de usuário |
| Operação offline durável | Operação explicitamente declarada offline-capaz; persistência local disponível; submeter | Pendente local após confirmação de persistência, nunca entrega remota | Falha de write mantém não durável/erro; retry usa idempotência definida | apenas operações que domínio aprovar |
| Operação não offline-capaz | Sem autoridade online; solicitar operação que exige decisão remota | Recusa ou mostra não submetida | Não converte tentativa em aceite; restaura após resposta de autoridade | qualquer cliente que exponha a operação |
| Retorno após partição/revogação | Operação pendente, autorização mudou durante desconexão; restabelecer rede | Revalidar antes do replay; resultado segue SPEC-004 e autoridade | Resposta desconhecida não apaga fila nem afirma remoção/entrega; preservar lacunas do cursor | requer integração regional; cenário não substitui teste de AP em SPEC-009 |
| Falha/corrupção/espaço/chave | Estado inicial explicitado; induzir uma falha por caso futuro | Não afirmar gravação/recuperação; classificar classe e efeito observável | Recuperar conforme política aprovada; não descartar silenciosamente intenção necessária | plataformas com armazenamento persistente |
| Lifecycle iOS | Operação persistida e em memória; variar bloqueio/suspensão/retomada/término por cenário | Estado/callback nunca promete conclusão sem evidência; retomada revalida | Proteção indisponível deixa operação pendente/inacessível; testar somente após contrato Apple/política definidos | iOS; versão e mecanismo OPEN |
| Fronteira FFI | Uma chamada por cenário; exercitar sucesso/erro/cancelamento/shutdown/concor­rência | Tipos, ownership, erro e executor seguem contrato; liberação sem uso após lifetime | Corrida entre cancelamento e completion não produz dupla conclusão ou use-after-free; caso exige binding decidido | macOS universal e iOS; não executado |
| Paridade de semântica em outras plataformas | Repetir contrato de estado em Windows/Linux/Android/web com adaptador próprio | Semântica compartilhada preserva distinções; capacidades nativas declaram diferenças | Plataforma sem capacidade não declara sucesso; adaptações específicas ficam em SPEC própria | aplicabilidade concreta e ferramentas de teste OPEN |

## 12. Decisões OPEN, responsáveis e fechamento

| ID | Decisão / alternativas | Responsável proposto | Consequência e condição de fechamento |
|---|---|---|---|
| O-014-01 | Classes offline e mutações: por operação/classe; leitura apenas; ou nenhuma mutação offline | Produto e donos de domínio; Alan integra | Define superfície de RF-043/RNF-017; fechar com matriz de operação, escopo e autorização aprovada, rastreável a RF. |
| O-014-02 | Retenção, backup, expiração e revogação: política por classe/tenant/finalidade ou ausência de cópia local | Kim, Bruce, Michael e decisão de produto | Define exposição e remoção; fechar com política e evidência de disposição por cópia, sem prazo inventado. |
| O-014-03 | Falta de espaço/corrupção: suspender escrita, descartar somente cache elegível ou recuperação assistida | Michael, Martin e dono de produto | Define durabilidade e perda aceitável; fechar por classe separando cache de intenção necessária e mensagens de estado. |
| O-014-04 | Fonte de tempo/frescor e relógio não confiável | Martin, Graydon e donos de domínio | Define validade, ordenação e replay; fechar com origem, semântica, degradação e teste futuro aprovado. |
| O-014-05 | Corrida revogação vs intenção em trânsito/fila | Kim e Martin; Graydon para API | Define revalidar, rejeitar ou remover; fechar com sequência e evidência de autoridade por operação, sem afirmar exclusão global. |
| O-014-06 | Binding: UniFFI, C ABI manual ou outra alternativa suportada | Alex, Donny, Graydon; decisão integrada por Alan/usuário | Define contrato FFI e risco de ownership/async. Fechar após fontes oficiais versionadas, compatibilidade dos alvos escolhidos e decisão registrada; candidatos não são aprovação. |
| O-014-07 | Lifecycle iOS: bloqueio/desbloqueio, background, término, primeiro desbloqueio, chave/backup | Donny e Kim/Michael | Define disponibilidade de cada operação; fechar com matriz por classe/estado/fonte Apple oficial e política, sem prometer background universal. |
| O-014-08 | Contratos Windows, Linux desktop, Android e web | Lea, Chet e responsáveis de plataforma | Define adaptadores e limites sem exclusão global; fechar por SPEC identificada e revisão independente. Android é SPEC-015; esta SPEC não faz seu aceite. |
| O-014-09 | Metas RF-043 para prioridade/energia e cenários que demonstram adaptação | Produto e Brendan, com Lea/Donny/Chet | Define métricas e limites; fechar com parâmetros aprovados, metodologia e critério futuro. Nenhuma meta ou medição é criada aqui. |
| O-014-10 | Cobertura RNF-017 de disponibilidade de AP/partição regional | Martin, Jouni, Graydon e Michael | Define fonte autorizada do estado do AP, autoridade regional e cenários; fechar em contratos SPEC-004/005/009 e caso revisado. |

## 13. Dependências e responsáveis

- **SPEC-004, Martin Kleppmann:** sync lógico, eventos, outbox/inbox, cursor, fence, idempotência, replay e reconciliação.
- **SPEC-005, Michael Stonebraker:** durabilidade, dados, isolamento, retenção, migração e recuperação local/remota.
- **SPEC-002, Kim Cameron; SPEC-020, Bruce Schneier:** identidade, autorização, revogação, consentimento, privacidade e política de cópias.
- **SPEC-003, Graydon Hoare:** contrato de operação remota/GMP e distinção de resultado/erro da autoridade.
- **SPEC-015, Chet Haase:** contrato Android/Kotlin; dependência de compatibilidade, não validação desta FFI Swift.
- **SPEC-016, Donny Wals:** lifecycle e fronteira Swift/iOS; revisor independente desta SPEC.
- **SPEC-012/013, Margaret Hamilton/Lea Verou:** produto, prioridades, experiência e aplicabilidade de clientes.
- **SPEC-009, Jouni Malinen:** disponibilidade de AP/Wi-Fi sob partição quando o caso envolver essa função.
- **SPEC-018, Brendan Gregg:** evidência/observabilidade futura e limites de energia, sem presumir sinais instrumentados.
- **Alan Turing/Ada Lovelace:** conciliação de decisões e rastreabilidade no catálogo/gates; não alterados nesta tarefa.

## 14. Estado, revisão e próximos critérios

Esta peça é DRAFT-0.1/OPEN e cobre parcialmente RF-043 e subescopos de RF-034/RNF-017. Não fecha RF-034, RNF-017, Guardian, AP, sync, segurança, energia nem compatibilidade geral. Não há SPEC aprovada, teste atribuído/executado, implementação ou validação demonstrada.

**Revisão independente designada:** Donny Wals para Rust–Swift/iOS. Chet Haase não é revisor de aceite desta SPEC; sua revisão de Android/Kotlin pertence à SPEC-015, podendo examinar interface comum apenas com recorte identificado e sem declarar binding Android validado. A revisão deve apontar a versão e hash desta peça. Após revisão, integrar somente deltas com evidência e preservar OPENs não decididos.

## 17. Limite de responsabilidade para cliente desktop

Para a lacuna de RF-034 desktop, a divisão é: **Lea Verou** integra jornadas observáveis, áreas da aplicação, navegação, acessibilidade e comportamento de UX para macOS/Windows/Linux desktop; **Alex Crichton** especifica o contrato compartilhado de estado local/offline e fronteira FFI do núcleo, incluindo distinções entre estado durável/em memória, autorização, pendência, tentativa, confirmação e erro. Confirmo capacidade para integrar documentalmente esse contrato comum e comparar fontes oficiais pertinentes. Não confirmo especialização nativa em APIs de macOS, Windows ou Linux, nem capacidade para atestar suporte de APIs, segurança de armazenamento ou lifecycle de cada SO.

A SPEC-014 oferece semântica local compartilhável para macOS arm64/x86_64, Windows e Linux quando aplicável; não especifica navegação, teclado/foco/leitor de tela, APIs de sistema, armazenamento nativo, permissão, lifecycle nem compatibilidade. Os candidatos a autores e revisores da pesquisa documental nativa constam da seção 18; precisam confirmar capacidade e limites. A atribuição de Alex é integração técnica documental, não especialização nativa ou responsabilidade operacional. Web permanece aplicável por jornada quando decidida; esta SPEC não assume persistência ou execução em background no browser nem exclui o produto.

**Fonte de divisão:** `MOBILIZACAO.md` atribui a Alex o contrato comum offline/persistência/FFI, Lea UX, e propõe Fowler/Grady para pesquisa de lifecycle/permissões desktop, Kim/Bruce para chaves e Erich para revisão do contrato comum; a capacidade desses recortes deve ser confirmada. Donny revisa somente Rust–Swift/iOS; Chet mantém Android. James revisa a UX da SPEC-013. `SPEC-013` §9 mantém a integração de RF-033/RF-034 dependente de atribuição e indexação. Esta divisão não é decisão de suporte ou API. Compatibilidade, APIs, versões, casos e validação permanecem OPEN/NOT_ASSIGNED/NOT_EXECUTED conforme aplicável.

## 18. Capacidade, responsáveis propostos e inventário por SO/arquitetura

**Capacidade e limite confirmados por Alex:** consigo especificar o contrato semântico comum de estado local/offline, integrar a fronteira Rust FFI e produzir crosswalk documental por SO/arquitetura usando fontes oficiais. Não atesto especialização nativa nem suporte de API para macOS, Windows ou Linux. Para recortes nativos, os autores/revisores abaixo são propostas explícitas de `MOBILIZACAO.md` § “Proposta de pesquisa desktop”; as atribuições ainda precisam de confirmação de capacidade. Se alguém não confirmar, Alan deve indicar outro especialista e revisor independente. Nada nesta seção atribui implementação, plantão ou validação.

| Alvo | Alex consegue especificar/integrar | Autor e revisor nativos propostos | Artefato e critério de fechamento |
|---|---|---|---|
| macOS arm64 | Semântica local comum e integração Rust–Swift; organizar referências oficiais, sem afirmar disponibilidade ou compatibilidade. | Lifecycle/permissões/compatibilidade: Fowler autor, Grady revisor. Chaves/armazenamento protegido/autorização: Kim autora, Bruce revisor. Todos precisam confirmar capacidade neste recorte. | Anexo macOS à SPEC-014 com fonte oficial por API/target/data, lifecycle, permissões, chave/armazenamento, negativos e recuperação. Parecer competente por hash e evidência própria de arm64. |
| macOS x86_64 | Mesmo contrato comum; distinguir requisitos do slice x86_64 de arm64. Não declaro compatibilidade binária. | Mesmos autores/revisores propostos para macOS arm64; aceitação e capacidade ainda não confirmadas. | Mesmo anexo macOS, com evidência explícita para x86_64, toolchain/targets e diferenças entre slices. Se não houver parecer competente, Alan designa especialista. |
| Windows desktop | Estados locais comuns, autorização, pendência, tentativa, confirmação, erro e fronteira FFI; sem escolher framework ou mecanismo nativo. | Lifecycle/permissões/compatibilidade: Grady autor, Fowler revisor. Chaves/armazenamento/autorização: Kim autora, Bruce revisor. Capacidade por API/SO precisa de confirmação. | Anexo Windows à SPEC-014 com fontes oficiais por API/versão/target, negativos, credenciais/chaves e recuperação. Revisão independente por hash; se os nomes propostos não confirmarem capacidade, Alan designa especialista. |
| Linux desktop | Estados locais comuns e interface FFI; não fixo distribuição, secret store ou lifecycle. | Lifecycle/permissões/compatibilidade: Grady autor, Fowler revisor. Chaves/armazenamento/autorização: Kim autora, Bruce revisor. Capacidade por ambiente precisa de confirmação. | Anexo Linux à SPEC-014 define ambientes/distribuições em escopo e fontes oficiais, sessão/secret store, falhas e recuperação. Sem escopo ou revisor competente, Alan designa dono/revisor. |
| iOS | Semântica comum/offline e fronteira FFI Rust–Swift. | Donny mantém o recorte iOS/Swift; nenhuma ampliação a macOS. Lea cobre UX. | Matriz por classe/operação e lifecycle Apple nos §§12–13; revisão de Donny limitada a iOS/Swift e identificada por hash. |
| Android | Semântica comum somente no recorte identificado. | Chet mantém Android/Kotlin; nenhuma revisão Android valida binding Swift. Lea cobre UX. | SPEC-015 e decisões de API/binding/OEM próprias; manter OPENs e revisão independente da SPEC-015. |
| Web | Alex não presume persistência local ou execução background de browser. | Lea integra jornada conforme aplicabilidade; especialista de armazenamento web ainda não proposto. | Aplicabilidade por jornada e eventual contrato web devem ser decididos antes de incluir persistência ou background; nenhum uso web é excluído do produto. |

### 18.1 Contrato comum e adaptações nativas

**Contrato comum de Alex:** semântica dos estados/resultados; regra de não converter estado local em remoto; fronteira de chamadas FFI; categorias de erro; distinção entre cancelamento solicitado, observado e concluído; ownership/lifetime abstratos; e dependências de cursor, idempotência e replay em SPEC-004. Binding, ABI, tipos finais, representação serializada, modelo de threads e funções exportadas continuam OPEN.

**Adaptações nativas:** fonte de permissão/capacidade, eventos de lifecycle, disponibilidade de armazenamento/chave, backup, criptografia de plataforma, notificação e interface de recuperação. Cada afirmação requer documentação oficial da API, versão/target e data de consulta. Sem isso, permanece OPEN. Target compilável ou semelhança entre SOs não comprova suporte real. Cada anexo cobre negativas de permissão/capacidade, suspensão/encerramento, indisponibilidade de chave, corrupção/falta de espaço e recuperação sem apagar intenção durável silenciosamente.

Para Windows, `contribuicao-kim-cameron.md` §20 (hash atual registrado em 18.3) distingue Credential Management (`CredWrite`/`CredRead`), DPAPI e KSP como alternativas com semânticas diferentes. Nesta atualização conferi as páginas Microsoft de `CredWriteW`, `CryptProtectData` e CNG KSP. Nenhuma API/provider foi escolhido; escopo, versão mínima GodMesh, compatibilidade e revisão nativa continuam OPEN.

### 18.2 Revisão e critérios de fechamento

1. Lea referencia RF-034, jornadas simultâneas/offline, permissões negadas, capacidade ausente, teclado/foco/leitor, fallback e casos por superfície desktop em SPEC-013; James revisa essa UX por hash.
2. Alex mantém o contrato comum de estado/FFI e integra anexos por plataforma. A linha da matriz não declara suporte de SO.
3. Alan resolve a lacuna de equipe por plataforma: Fowler/Grady confirmam (ou não) capacidade de autoria/revisão para lifecycle, permissões e compatibilidade; Kim/Bruce fazem o mesmo para chaves/armazenamento protegido. Se faltar competência, Alan designa especialista independente. Donny permanece iOS; Chet permanece Android.
4. Cada API proposta registra fonte oficial, versão/target, data, capability/permissão, falha e recuperação. Até lá, API/versão e compatibilidade ficam OPEN. Nenhum mínimo de SO/hardware é escolhido aqui.
5. Casos futuros cobrem offline, write não durável, permissão/capacidade negada, chave/armazenamento indisponível, suspensão/encerramento, cancelamento FFI e reconexão/replay. Casos formais NOT_ASSIGNED; observado NOT_EXECUTED.
6. Revisão do contrato comum/FFI: Erich é o revisor proposto na mobilização e precisa confirmar recorte/capacidade. Donny revisa somente Rust–Swift/iOS. Cada anexo desktop requer parecer independente competente por hash. Aprovação permanece pendente.

### 18.3 Pesquisa de fronteiras nativas integrada

A matriz abaixo integra os pareceres disponíveis e registra somente o alcance que suas fontes sustentam. As URLs foram conferidas nas páginas oficiais em 2026-11-16. Versões de documentação ou introdução da API não são versões mínimas do produto. Nenhuma API foi selecionada nem testada.

| Operação | SO, arquitetura, API e versão/data documentadas | Limite, dependência e estado |
|---|---|---|
| Empacotar binário universal | macOS arm64/x86_64. Apple, [Building a universal macOS binary](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary), consultada em 2026-11-16, indica Xcode 12.2+ para construir binários universais e fatias separadas. | Isso não prova as fatias de Rust, FFI ou dependências GodMesh. Distinguir arm64 nativo, x86_64 em Intel e x86_64 via Rosetta. Versão mínima do macOS, Xcode, SDK, distribuição, dependências e assinatura OPEN. |
| Lifecycle e retomada | macOS: [NSApplication](https://developer.apple.com/documentation/appkit/nsapplication) e [NSApplicationDelegate](https://developer.apple.com/documentation/appkit/nsapplicationdelegate), documentação corrente; Windows: [App lifecycle](https://learn.microsoft.com/en-us/windows/apps/develop/launch/app-lifecycle), documentação corrente; Linux: [QCoreApplication](https://doc.qt.io/qt-6.12/qcoreapplication.html), Qt 6.12.0, e [Gtk.Application](https://docs.gtk.org/gtk4/class.Application.html), API GTK 4. | Eventos/callbacks de encerramento não garantem execução contínua nem conclusão durável sob crash. O documento Qt alerta que Windows pode terminar o processo sem executar o restante após `exec()`. App versus helper/agent/daemon permanece OPEN; Alex/Kleppmann definem fila, ponto de persistência e reconciliação. Frameworks Windows/Linux não foram escolhidos. |
| Microfone/câmera | macOS: [media capture authorization](https://developer.apple.com/documentation/bundleresources/requesting-authorization-for-media-capture-on-macos), a partir de macOS 10.14, exige consentimento e chaves de uso para câmera/microfone. Windows: [desktop app privacy](https://learn.microsoft.com/en-us/windows/client-management/manage-desktop-app-privacy-access), aplicação por recurso/build permanece não confirmada na leitura direta relatada por Grady/Bruce. Linux: [XDG Camera portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Camera.html), versão efetiva e backend OPEN. | Uma permissão não concede outra: acesso à câmera não demonstra acesso ao microfone. Negação/indisponibilidade afeta somente a função dependente. No macOS, ausência da chave de uso pode encerrar o app; revisar processo efetor, entitlement, pacote e revogação. Versões mínimas e aplicabilidade Windows por build continuam OPEN. |
| Localização, BLE e rede local | macOS arm64/x86_64: [Core Location](https://developer.apple.com/documentation/corelocation/requesting-authorization-to-use-location-services), [Core Bluetooth](https://developer.apple.com/documentation/corebluetooth) e [TN3179 local network privacy](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy). TN3179 registra privacidade de rede local desde macOS 15, correções em 15.1 e diferenças app/helper/agent/daemon. | Fonte de localização/BLE deve ser cruzada por método e versão; catálogo de frameworks não prova disponibilidade ou background. Rede local é por operação, processo e identidade de código; as exceções documentadas para daemon/root/Terminal não autorizam contorno de consentimento. Grant, assinatura, operação, entitlement, distribuição e versão mínima permanecem OPEN com Fowler/Vint/Kim. |
| Arquivos e segredos | macOS: [App Sandbox](https://developer.apple.com/documentation/security/app-sandbox) e [Keychain Services](https://developer.apple.com/documentation/security/storing-keys-in-the-keychain), páginas correntes. Keychain é candidato para credenciais/segredos pequenos; App Sandbox é requisito para distribuição Mac App Store conforme a pesquisa de Fowler. | Sandbox e Keychain não definem política GodMesh, E2EE, retenção por cópia, backup ou recuperação. Canal de distribuição, grupos, acesso quando bloqueado e dados protegidos OPEN com Kim/Bruce. |
| Credenciais e proteção de segredo | Arquitetura alvo do cliente Windows ainda OPEN. As páginas Microsoft de runtime oferecem artefatos x86/x64/ARM64, mas isso não comprova a arquitetura GodMesh. [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew) associa credencial à sessão de logon do token e admite ausência de credential set. [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata) documenta Windows XP como mínimo da API e `CRYPTPROTECT_LOCAL_MACHINE` permite descriptografia por qualquer usuário local. [CNG KSP](https://learn.microsoft.com/en-us/windows/win32/seccertenroll/cng-key-storage-providers) distingue providers; software não significa hardware. | Credential Management, DPAPI e KSP são modelos/alternativas diferentes; nenhum foi escolhido e não são equivalentes a chave não exportável. Windows XP é mínimo declarado da API citada, não mínimo GodMesh. Escopo por usuário/máquina/logon, provider TPM, edição/build, fallback, import/export e recuperação OPEN. |
| Framework, arquitetura e backend gráfico | Linux: [Qt 6.10 Supported Platforms](https://doc.qt.io/qt-6.10/supported-platforms.html) lista distribuições/arquiteturas específicas, incluindo notas sobre arm desktop/glibc; [Qt Wayland](https://doc.qt.io/qt-6/wayland-and-qt.html) é página Qt 6.12; GTK 4 é alternativa pesquisada por Grady. | Configurações da matriz X11 não se estendem automaticamente a Wayland, nem a outras distros/releases. Qt Quick e GTK não foram selecionados; arquitetura, compositor, driver, pacote, licença, distribuição e suporte de cada operação OPEN. |
| Background/autostart | Linux sandbox: [XDG Desktop Portal Background v2](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html), versão da interface 2, separa resultado `background` de `autostart`. Windows: identidade de pacote é necessária para algumas funções de background/push, mas [Microsoft distingue notificações locais](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/modernize-packaged-apps), que podem funcionar sem identidade. macOS: processo de app/helper e regras de operação continuam específicas na TN3179. | Pedido não é concessão, nem concessão prova execução. Não exigir identidade de pacote Windows para toda notificação. Backend/portal, pacote, política, versões, processo efetor e reconciliação após revogação OPEN. |
| Segredos/chaves Linux | [Secret Service API 0.2 DRAFT](https://specifications.freedesktop.org/secret-service/latest-single/), publicada em 2026-10-19, descreve serviço em sessão do usuário, itens potencialmente bloqueados e atributos que podem não estar criptografados. [Kernel Key Retention Service](https://docs.kernel.org/security/keys/core.html), página `latest` consultada em 2026-11-16, cobre chaves para serviços/processos de kernel. | Secret Service draft não prova implementação universal, isolamento entre apps ou disponibilidade sem sessão. Kernel key retention não é, por si, vault de aplicação nem prova persistência entre reinícios ou hardware. Provider, distro/desktop, escopo, metadados, bloqueio/headless, perda e recuperação OPEN. |
| Versionamento e compatibilidade Windows | Nas fontes Microsoft consultadas em 2026-11-16, [downloads](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/downloads) lista Stable 2.6.4 (27/10/2026), a [release notes 2.0](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-notes/windows-app-sdk-2-0) lista 2.6.5 (02/11/2026), e [versioning overview](https://learn.microsoft.com/en-us/windows/apps/get-started/versioning-overview) apresenta Windows App SDK 2.2 e Windows SDK 10.0.26100; Grady também aponta [Windows SDK](https://learn.microsoft.com/en-us/windows/apps/windows-sdk/) 28000. | As páginas apresentam divergência de versão/catálogo; não resolvi por inferência. 2.6.5, 2.6.4, 2.2, SDK 26100 e 28000 não são mínimo ou escolha GodMesh. Confirmar release estável, edição/build/arquitetura e mínimo por API diretamente antes de proposta de stack. Sem seleção ou instalação nesta análise. |
| Contrato comum e FFI | Todos os alvos do §18. Alex cobre semântica comum estado/offline, fronteira Rust FFI e integração do crosswalk. | Binding/ABI/threading/ownership finais permanecem OPEN. Parecer técnico independente FFI proposto: Erich; capacidade/aceite precisam ser confirmados. Cada contrato nativo requer revisor independente competente. Donny apenas iOS; Chet apenas Android. |

**Proveniência e revisões:** Fowler, `parecer-macos-universal-fowler.md`, DRAFT-0.2, SHA-256 `e3b98817864412591cf50b220d433ca184206fb6a6a05bc7c2dce85daac50cdd`; o parecer reconhece que sua revisão por hash não prova especialização nativa macOS. Grady, `contribuicao-grady-booch.md` § Pesquisa desktop Windows/Linux, SHA-256 atual `5f1c079fec306eca4fb0e29f06512ebfec90af81dfcea6a9f7c57b1d4079c17d`; difere do hash inicialmente encaminhado, e pareceres de Bruce registram revisão de hash anterior. Kim, `contribuicao-kim-cameron.md` §20, SHA-256 atual `5c205e4c66dffe1d0189cad43ac9de01def72376120fb7d7768d0464cd6cc614`; contém crítica de Bruce na §27 a hash anterior e qualifica import/export como proposta conservadora, ainda não política. Bruce, `contribuicao-bruce-schneier.md` §§24–27, SHA-256 observado `67f3d8ff602b8454af605cd4a22374bd6c29fe125b2565b9baae93d4fba1ad9d`; declara que sua competência é confronto documental, não homologação nativa. Os hashes recebidos no encaminhamento de Alan não coincidem com os hashes atuais de Grady e Kim, nem com a versão DRAFT-0.2 atual de Fowler. As revisões cruzadas solicitadas pelos autores não são duplicadas aqui; toda conclusão permanece limitada ao hash que o respectivo revisor declara ter lido.

**Limites atuais:** integrei fontes oficiais e qualificadores, mas não fiz pesquisa exaustiva por API, build, versão, processo, modelo de hardware ou ambiente Linux. Não selecionei stack, API, provider ou mínimo de SO. Conflitos Microsoft e lacunas de release, versão, provider, política, capacidades e revisão nativa permanecem OPEN para os responsáveis indicados. Casos formais NOT_ASSIGNED; execução NOT_EXECUTED.

## Mudanças desta versão — ADR-058 e ADR-059

**ADR-059 — grant offline:** são permitidas apenas estas cinco operações: (1) abrir incidente SOS local e enfileirar; (2) check-in de sessão Guardian existente; (3) mensagem a canal já autorizado; (4) observação de localização local; (5) PTT em rede local com grant de floor válido. Negações explícitas: divulgar localização precisa, mutação federada, criar/alterar grant, mudar identidade, transmitir RF ou executar transcrição remota.

A permissão exige epoch comparável, frescor do perfil dentro do limite aplicável, ausência de revogação conhecida e estado anti-rollback verificável. `stale`, `unknown`, relógio incerto ou política indefinida negam. O valor numérico de frescor é desconhecido até medição; não há limiar definido nesta SPEC. A recuperação/replay reavalia esses predicados antes de qualquer efeito.

**ADR-058 — ACK:** cliente/FFI preserva a classe recebida, sem promoção por tempo, sucesso inferior ou ausência de erro. `unknown` permanece desconhecido na API local e na UI; não significa falha conclusiva nem sucesso.

- **casos por decisão (resposta ao veto C3-1):** seção final `Casos por decisão (ADR-056…063)` com casos locais por decisão aplicável, cada um com pré-condição, estímulo, esperado observável e tipo P/N/B/F; todos `NOT_EXECUTED`, ID formal `NOT_ASSIGNED`.
- **casos por requisito e casos P desbloqueados (C3-1):** subseção `Casos por requisito (P/N/B/F)` ao final, uma linha por requisito coberto; `S014-056-P1`, `S014-059-P1` e `S014-062-P1` reescritos para o comportamento com parâmetro ausente (`param_not_configured`), sem inventar janela, frescor ou prazo; novo `S014-061-B1` para a borda da ADR-061.

As revisões citadas são de versões anteriores; revisão independente desta versão permanece pendente. Estados de execução e testes permanecem `NOT_EXECUTED`.


## Casos por decisão (ADR-056…063)

Casos documentais planejados. ID formal: `NOT_ASSIGNED`. `Estado` em todos: `NOT_EXECUTED`. Nenhum parâmetro OPEN é convertido em valor. Nomes de estado citados são os desta SPEC (§5) e continuam sem codificação acordada.

| ID local | Decisão | Pré-condição | Estímulo | Esperado observável | Tipo | Estado |
|---|---|---|---|---|---|---|
| S014-056-P1 | ADR-056 | Cliente e núcleo/binding cujo par N/N-1 não tem janela de perfil aprovada; registro persistido do formato anterior | Abrir cliente e ler estado local | Leitura conclui com `compatibility=param_not_configured` exposto; campo desconhecido preservado; nenhuma migração destrutiva; nenhum valor de janela presumido e nenhuma versão declarada suportada | P | NOT_EXECUTED |
| S014-056-N1 | ADR-056 | Cliente sem feature crítica comum com o serviço | Operação remota a partir do cliente | Recusa antes do efeito; estado local expõe incompatibilidade com versão/perfil requeridos; nenhuma mutação federada gravada; nenhum downgrade | N | NOT_EXECUTED |
| S014-056-B1 | ADR-056 | Registro local antigo com campo numérico desconhecido | Regravação do registro | Campo desconhecido mantido; número de campo não reutilizado; campo removido tratado como `reserved` | B | NOT_EXECUTED |
| S014-056-F1 | ADR-056 | Formato persistido sem versão compatível declarada | Abertura do cliente após atualização | Dado não é destruído; estado local expõe inacessível/erro explícito de versão; nenhuma gravação sobre o dado antigo | F | NOT_EXECUTED |
| S014-057-P1 | ADR-057 | Fila local com itens gravados com `(origin_node_id, tenant_id, stream_id, partition, authority_epoch, seq)` | Replay após reconexão | Itens enviados na ordem canônica; posição preservada item a item; cursor do cliente vinculado a principal/escopo/filtros/perfil/retention | P | NOT_EXECUTED |
| S014-057-N1 | ADR-057 | Item de fila com cursor sem `partition` ou com epoch incomparável | Tentativa de replay | Item rejeitado e marcado com erro de cursor; não é traduzido, não é reenviado como atual; fila não avança nesse item | N | NOT_EXECUTED |
| S014-057-F1 | ADR-057 | `authority_epoch` mudou entre persistência e replay | Replay do item | Falha fechada; item marcado `desconhecido`; nenhum item seguinte ultrapassa a lacuna | F | NOT_EXECUTED |
| S014-057-B1 | ADR-057 | Dois itens locais com mesma `seq` em partições distintas | Consulta da fila | Chaves distintas; nenhum item deduplicado por chave sem partição; ordem canônica mantida | B | NOT_EXECUTED |
| S014-058-P1 | ADR-058 | Operação offline-capaz; gravação local confirmada | Enfileirar operação | Estado `pendente local` com classe `persisted_local`; nenhum campo de envio, entrega ou ACK humano preenchido | P | NOT_EXECUTED |
| S014-058-N1 | ADR-058 | Item apenas gravado localmente | Tentativa de exibir/retornar "enviado" ou "entregue" | Estado mantido em `não submetido`/`pendente local`; promoção de classe rejeitada pela fronteira local | N | NOT_EXECUTED |
| S014-058-F1 | ADR-058 | Envio realizado; timeout sem resposta | Consulta de resultado | Resultado `desconhecido`; timeout não é rejeição nem sucesso; item permanece com estado incerto | F | NOT_EXECUTED |
| S014-058-B1 | ADR-058 | Inbox local separado da projeção | Leitura do estado de sincronização | ACK máximo `persisted_local`; nenhum `materialized` reportado; separação visível nas dimensões | B | NOT_EXECUTED |
| S014-059-P1 | ADR-059 | Grant vigente para operação enumerada, epoch comparável, sem revogação conhecida e anti-rollback íntegro, mas sem limite de frescor aprovado | Mutação local permitida | `autorização=unknown` na dimensão frescor (`freshness_limit=param_not_configured`), separada de `grant=valid`; intenção não é gravada; nenhum limiar embutido no cliente e nenhuma divulgação a outro principal | P | NOT_EXECUTED |
| S014-059-N1 | ADR-059 | Offline | Operação não enumerada (divulgar localização precisa, mutação federada, criar/alterar grant, mudança de identidade, transcrição remota, transmissão RF) | Recusada com motivo; zero bytes persistidos para a operação; nenhuma fila criada | N | NOT_EXECUTED |
| S014-059-F1 | ADR-059 | Restore de snapshot antigo com grant na validade, ou grant com epoch anterior, ou revogação conhecida | Reavaliação do grant e nova mutação | Grant negado; contador monotônico não regride; operação recusada e estado `autorização=unknown` | F | NOT_EXECUTED |
| S014-059-B1 | ADR-059 | Limite de frescor OPEN | Mutação condicional | Recusa com `freshness_limit=not_decided` informado; nenhum limiar embutido no cliente | B | NOT_EXECUTED |
| S014-060-P1 | ADR-060 | Derivado de conteúdo cuja cadeia inclui perna terminadora | Leitura/entrega local do derivado | Estado local carrega `confidentiality=terminated`; derivados (índice, cache, fila) herdam o mesmo valor | P | NOT_EXECUTED |
| S014-060-N1 | ADR-060 | Artefato sem chave disponível ao núcleo | Tentativa de ler como E2EE ou de recuperar conteúdo sem chave | Leitura negada; nenhuma alegação de E2EE aceita; nenhum conteúdo reconstruído | N | NOT_EXECUTED |
| S014-060-F1 | ADR-060 | Metadado `confidentiality` ausente ou ilegível no registro local | Consumo do registro | Tratado como `terminated`; ausência não suprida por cache, versão anterior ou ausência de erro | F | NOT_EXECUTED |
| S014-060-B1 | ADR-060 | Arquivo protegido inacessível no estado de bloqueio | Abertura do app em background | Payload não revelado no cache de apresentação; nenhuma confirmação de leitura ou perda | B | NOT_EXECUTED |
| S014-061-P1 | ADR-061 | Transação local única com `outbox` | Mutação local permitida | Intenção e evento de outbox commitados juntos; `pendente local` só após confirmação da gravação; publicação após commit | P | NOT_EXECUTED |
| S014-061-N1 | ADR-061 | Operação já enfileirada e já efetivada | Replay do item | Nenhum efeito externo reemitido (bridge, rádio, notificação); apenas reconstrução de estado local | N | NOT_EXECUTED |
| S014-061-F1 | ADR-061 | Processo encerrado/crash entre commit local e envio | Reinício do cliente | Outbox reprocessada sem duplicar a operação (chave idempotente); operação incompleta não vira sucesso; nada alegado para o intervalo | F | NOT_EXECUTED |
| S014-061-F2 | ADR-061 | Estado de autoridade `stale`/`unknown` | Tentativa de mutação forte | Bloqueio com estado explícito; leitura local autorizada continua; nenhuma escrita apresentada como aceita | F | NOT_EXECUTED |
| S014-061-B1 | ADR-061 | Transação local aberta com `outbox` gravado e evento ainda não publicado | Consultar o estado da operação antes do publish | Estado lido como `pendente local` com prova de commit; `materialized` e efeito externo ausentes; nenhuma republicação do item pelo leitor nem promoção de classe | B | NOT_EXECUTED |
| S014-062-P1 | ADR-062 | Política por classe existente para a gravação local, com prazo não decidido | Gravação de estado e fila | Gravação aceita com `retention_policy_ref` registrado e `retention=param_not_configured`; nenhum prazo é gravado como aprovado nem convertido em retenção indefinida; descarte por cópia permanece `unknown` | P | NOT_EXECUTED |
| S014-062-N1 | ADR-062 | Classe sem política aprovada | Tentativa de gravação persistente | Falha fechada com erro explícito; nenhum dado gravado além do mínimo do efeito imediato; interface marca não durável | N | NOT_EXECUTED |
| S014-062-F1 | ADR-062 | Política expirada; cópias locais incluem cache, fila, índice e backup do dispositivo | Gatilho de descarte | Descarte registrado por cópia; fila necessária preservada conforme política; nenhuma afirmação de revogação global | F | NOT_EXECUTED |
| S014-062-B1 | ADR-062 | Hold local com autoridade e revisão | Consulta comum ao dado retido | Retenção estendida; acesso comum não reativado; estado por cópia visível | B | NOT_EXECUTED |
| S014-063-P1 | ADR-063 | Núcleo/biblioteca distribuída com SBOM, build reproduzível e assinatura verificada | Instalação limpa do artefato | Verificação de assinatura aprovada; versão, digest e `generation` registrados no cliente | P | NOT_EXECUTED |
| S014-063-N1 | ADR-063 | Artefato com `generation` inferior ao piso de versão assinado | Tentativa de instalar/atualizar | Artefato rejeitado; versão atual mantida; operação não inicia; motivo `generation_below_floor` exibido | N | NOT_EXECUTED |
| S014-063-F1 | ADR-063 | Assinatura inválida ou chave de assinatura revogada | Aplicação do artefato | Rejeição com erro tipado; nenhuma execução; nenhum fallback não assinado | F | NOT_EXECUTED |
| S014-063-B1 | ADR-063 | Atualização dentro da janela N/N-1 mas com `generation` abaixo do piso | Instalação da atualização | Rejeitada: o piso prevalece sobre a janela; registro de auditoria do motivo | B | NOT_EXECUTED |

**Sem caso por falta de decisão:** o binding (UniFFI versus C ABI) e os limites numéricos de frescor seguem OPEN, portanto não há caso de compatibilidade de binding por alvo nem caso positivo de grant dentro do prazo. O formato persistido não tem versão acordada, então S014-056-F1 verifica apenas o desconhecido, não uma migração aprovada.

### Casos por requisito (P/N/B/F)

Uma linha por requisito que esta SPEC cobre, com o limite declarado no §3. Células citam casos da tabela anterior e de §11; onde faltava caso do tipo, o ID é novo e descrito na Nota. Todos `NOT_EXECUTED`; ID formal `NOT_ASSIGNED`. Nenhum parâmetro OPEN vira valor.

| Requisito | Positivo (ID local) | Negativo | Borda | Falha/recuperação | Nota |
|---|---|---|---|---|---|
| RF-043 (principal) | S014-059-P1; S014-058-P1 | S014-059-N1; S014-058-N1 | S014-059-B1; S014-061-B1 | S014-061-F1; S014-061-F2 | Cobre estado local e limites de execução relevantes à prioridade/energia. Metas de energia e latência seguem OPEN (O-014-09); nenhum caso mede SLO energético. |
| RF-034 (subescopo) | S014-058-P1 | S014-058-N1 | S014-061-B1 | S014-061-F1 | Apenas inicialização/estado local; jornada e áreas de UI ficam em SPEC-013. |
| RNF-017 (subescopo cliente) | S014-057-P1 | S014-057-N1; S014-059-N1 | S014-057-B1 | S014-057-F1; S014-056-F1 | Conexão local vs sync regional e revisão antes de replay. Disponibilidade de AP e teste de partição regional permanecem em SPEC-004/005/009 (O-014-10). |
| ADR-060 (confidencialidade do estado local) | S014-060-P1 | S014-060-N1 | S014-060-B1 | S014-060-F1 | Aplicável a RF-043/RNF-017 quando há artefato protegido; não substitui SPEC-020. |
| ADR-063 (artefato do núcleo) | S014-063-P1 | S014-063-N1 | S014-063-B1 | S014-063-F1 | Distribuição do núcleo/biblioteca; `generation` e piso vêm de decisão assinada, não desta SPEC. |
