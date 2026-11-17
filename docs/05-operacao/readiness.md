# Contrato documental de readiness por serviço

**Estado:** proposta operacional para integrar às fontes normativas após revisão dos owners. ADR-027 aprova o princípio de readiness por serviço, não este contrato concreto. Este texto não declara serviço implementado, endpoint disponível ou resultado medido; limiares e dependências efetivas dependem dos contratos de cada serviço.

## Termos

- **Liveness:** processo continua executando; não prova capacidade de atender.
- **Startup:** inicialização ainda não terminou; durante esse estado o serviço não recebe tráfego novo.
- **Readiness:** predicados daquele serviço/rota permitem receber operações novas no escopo declarado.
- **Degraded:** serviço pode atender um subconjunto explicitamente nomeado, sem violar invariantes; apenas rotas compatíveis podem admitir novas operações.
- **Unready:** um predicado requerido falhou; negar novas operações da rota afetada.
- **Unknown:** evidência está ausente, stale, inválida ou contraditória; tratar fail-closed para novas operações protegidas.
- **Isolated:** controle global indisponível, mas comportamento local continua conforme autorização e dados locais; não anunciar disponibilidade global.

Estados de domínio distintos (por exemplo, ACK de SOS, conexão de mídia e acesso a backup) não devem ser reduzidos a um booleano global. Transição exige observar o predicado e sua idade; limiar/histerese numérica continua pendente de baseline/aprovação.

## Envelope mínimo do resultado

| Campo | Tipo documental | Regra |
|---|---|---|
| `service` / `route_class` | identificadores controlados | nomes versionados; sem tenant ou usuário como label pública |
| `state` | enumeração acima | `ready` é escopado, nunca global por inferência |
| `observed_at` | instante com base temporal declarada | idade comparada a freshness aprovado; relógio incerto mantém `unknown` |
| `evidence` | lista de predicados e resultado | cada predicado nomeia fonte, versão, estado e classe de falha; sem segredo/payload |
| `dependencies` | lista de dependências requeridas/opcionais | falha opcional pode gerar `degraded`; requerida gera `unready` conforme contrato de rota |
| `admission` | `accept_new`, `existing_only`, `drain`, `reject` | ação por rota; não presume cancelar sessão existente |
| `reason_class` | categoria sanitizada | estável e limitada; sem texto livre que possa carregar PII/segredo |
| `capacity` | estado/classe de capacidade | reservado/disponível/limitado/desconhecido segundo unidade definida pelo serviço |

Campos não disponíveis não são preenchidos por suposição. Compatibilidade/esquema de wire precisam ser definidos pelos owners; esta tabela não é API aprovada.

## Predicados por tipo de serviço

| Tipo | Evidência mínima a declarar pelo owner | Resultado de falha |
|---|---|---|
| API e serviços regionais | processo/configuração válida, versão/protocolo, rota autorizada, dependências requeridas, capacidade e fonte recente | retirar apenas a rota afetada de novas admissões; comunicar indisponibilidade/degradação |
| Persistência/eventos | acesso autorizado, capacidade de persistência compatível com a operação, checkpoint/retention/replay conforme contrato de dados | distinguir banco acessível de durabilidade/materialização; sem evidência requerida não declarar `ready` |
| Replicação/federação | autoridade, escopo de dados autorizado, estado de cursor/lag e região/peer elegível | não converter partição em sucesso; impedir escrita/failover sem autoridade determinada |
| Mídia/PTT | sinalização e recursos de mídia disponíveis para rota; grants/leases válidos e capacidade | preservar semântica das sessões ativas conforme política; bridge isolada não muda voz nativa |
| Bridge externa | adapter autorizado, lease/fencing válido, destino externo e modo RX/TX declarado | bloquear TX quando autorização/fencing desconhecido; manter status de bridge separado do núcleo |
| SOS | rota/capacidade por camada, ACK técnico separado do ACK humano e alternativa configurada quando aplicável | declarar etapa não confirmada; nunca anunciar atendimento humano pela prontidão do serviço |
| Cliente local/mobile | permissão/capacidade/rota de cada função, estado local/offline e restrição de energia/background | mostrar estado por serviço; não converter uma função indisponível em indisponibilidade global |

Esta enumeração indica quais contratos precisam ser preenchidos; os owners de serviço definem os predicados concretos e dependências. Não prova que a checagem existe.

### Responsabilidade documental por predicado

O envelope comum de readiness, freshness e efeito de admissão usa o primário documental `C13-SIG-020` (Brendan Gregg). Cada predicado continua com o primário e as interfaces da família do sinal na [matriz C13 de observabilidade](observabilidade.md#catálogo-de-sinais-candidatos), alinhada ao [registro único de responsabilidades por família](../06-planejamento/matriz-rastreabilidade.md#integração-c13--observabilidade-e-evidência-proposta-documental). Este vínculo é documental: não nomeia owner de operação, plantão, endpoint implementado ou fonte já disponível.

### Capacidade e readiness por cenário (proposta)

Readiness não significa capacidade ilimitada nem valida qualquer perfil de carga. Para Personal, declarar serviços/rotas incluídos, população e condições de cliente (inclusive offline/background quando aplicável); para Teams, declarar tenant/serviço/região e isolamento sob concorrência; para Node, declarar topologia/região, dependências, dados autorizados e comportamento em partição/failover. Em cada perfil, classificar modo nominal, pico e degradado e declarar a carga de referência, capacidade reservada, fontes e janela das evidências.

| Regime | Condição documental proposta para `ready`/`degraded` | Pergunta/decisão pendente |
|---|---|---|
| Nominal | Predicados obrigatórios satisfeitos e capacidade reservável para a mistura de operações declarada; estado vale apenas para a rota e escopo medidos | Quais serviços, operações e perfis de implantação compõem a oferta inicial? |
| Pico | Mesmas invariantes de autorização/durabilidade, mais os limites de latência, fila e recursos que o owner aprovar para o perfil; ao ultrapassar limite, reduzir admissão conforme prioridade explícita | Qual população, taxa/concorrência, duração do pico, p95/p99 e margem de reserva devem ser aceitos? |
| Degradado/failover | Verificar dependências remanescentes, autoridade/residência, replay e capacidade pós-falha; `degraded` só enumera rotas seguras que continuam disponíveis | Qual falha deve ser tolerada por serviço/região e como tratar sessões existentes, backlog e recuperação? |

Sem cenário aprovado e evidência fresca, um serviço pode declarar somente o escopo que seus predicados realmente cobrem; não pode anunciar capacidade para picos ou failover não avaliados. Ausência de baseline não torna o estado automaticamente `unready` para toda operação, mas impede afirmar um envelope quantitativo ou release gate baseado nesse envelope. O owner de serviço propõe predicados/fontes; SRE propõe método/freshness; produto/usuário decide envelope, impacto, limites e ações.

## Transições e efeito de tráfego

1. `starting` permanece sem admissão de novas operações até as evidências requeridas ficarem válidas.
2. `ready` só é publicado para a rota coberta e com evidência atual, autorização e capacidade reservável.
3. Falha de predicado requerido move a rota para `unready`; falha de predicado opcional só pode produzir `degraded` se o contrato declarar quais operações continuam seguras.
4. Evidência perdida, stale, schema incompatível ou contradição resulta `unknown`; não reutilizar último `ready` indefinidamente.
5. Ao retirar readiness, novas sessões não são encaminhadas. Sessões existentes seguem política de domínio: continuar, rejeitar novos comandos, drenar ou terminar com aviso; a decisão por serviço/rota permanece aberta quando não houver contrato normativo.
6. Recuperação exige observações suficientes para satisfazer estabilidade/histerese aprovada, e restauração de dependência, autoridade, capacidade e integridade. Não reintroduzir carga antes de evidência válida.

Eventos de transição devem guardar estado anterior/novo, rota/classe, razão sanitizada, fonte, timestamp/qualidade, ação de admissão e resultado. A trilha administrativa fica separada da telemetria agregada.

## Decisões ainda requeridas

Os responsáveis documentais por sinal estão no [catálogo C13](observabilidade.md#catálogo-de-sinais-candidatos); o registro por família na [matriz de rastreabilidade](../06-planejamento/matriz-rastreabilidade.md#integração-c13--observabilidade-e-evidência-proposta-documental) é a fonte canônica. Falta preencher a matriz `serviço/rota → dependências obrigatórias/opcionais → fonte/predicado → freshness → regra de degradação → efeito em operações novas e existentes → recuperação/histerese → alerta/runbook`, vinculando-a aos perfis Personal/Teams/Node e aos regimes nominal/pico/degradado aplicáveis. O usuário/produto aprova quais dependências podem bloquear cada serviço e os efeitos para operações críticas. Decisões abertas incluem população/topologia, carga/pico, operações/populações elegíveis para p95/p99, capacidade reservada, dependências toleráveis no failover, janelas/freshness/histerese, orçamento e owner/on-call operacional dos alertas. Limiares, períodos e cargas dependem de baseline; o texto não atribui esse owner nem declara envelope quantitativo decidido.

## Exemplos de resultado esperado (não executados)

- Banco aceita conexão, mas replay/retention necessário para o serviço não foi verificado: readiness desse serviço não pode ser inferida de `SELECT 1` ou processo vivo.
- Bridge FRN cai enquanto serviço de voz nativa segue pronto: a bridge fica indisponível e o domínio nativo não é reclassificado globalmente.
- Coletor de telemetria deixa de responder: a fonte fica sem cobertura/`unknown`; isso não prova que o serviço de produto caiu nem que permaneceu saudável.
- Controlador global inacessível: nó regional declara escopo isolado e não recebe autorização global por inferência; operações locais só continuam se autoridade e dados permitirem.

## Fontes e limite

Alinhado a `01-arquitetura/arquitetura-referencia.md`, ADR-027 em `06-planejamento/decisoes.md`, `01-arquitetura/balanceamento.md`, `05-operacao/implantacao.md` e `05-operacao/observabilidade.md`. O achado GMD-A14-002 permanece parcialmente atendido até dependências e ações serem preenchidas/aprovadas e evidências futuras verificarem o comportamento. Sem inspeção de implementação ou execução.
