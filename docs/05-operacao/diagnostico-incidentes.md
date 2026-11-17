# Diagnóstico de incidentes e recuperação operacional

**Estado:** roteiro documental para orientar investigação; não é runbook executado nem comprova disponibilidade, causa ou recuperação.

O método de registro, hipótese e evidência deste roteiro tem primário documental `C13-METHOD-003` (Brendan Gregg), com interfaces para o primário do sinal/família referenciado no [catálogo C13](observabilidade.md#catálogo-de-sinais-candidatos). Essa atribuição não é owner de incidente, comandante operacional, on-call nem executor de mitigação; esses papéis continuam não atribuídos até decisão organizacional.

## Registro inicial

Para cada incidente, preservar (com acesso apropriado e redaction): horário e qualidade do relógio; serviço/rota e região afetados; alcance observado; estado esperado e observado; versão/configuração; mudança recente conhecida; sinais/fonte e frescor; dependências; autorização; impacto para operações novas/existentes; hipótese falsificável; próxima observação capaz de distinguir hipóteses; ação, owner e evidência de fechamento. Não registrar conteúdo de áudio, mensagem, credencial ou localização pessoal precisa para “facilitar” correlação.

Se telemetria estiver ausente ou atrasada, registrar cobertura como desconhecida. Falta de sinal não confirma causa nem saúde. Distinguir `symptom_observed`, `cause_hypothesis`, `cause_confirmed` e `recovery_observed`; usar confirmação somente quando evidência distinguir alternativas.

## Sequência de triagem

1. Delimitar serviço, rota, região, janela e classes de usuário/operação atingidas sem coletar identificadores além do necessário.
2. Verificar saúde/cobertura da própria telemetria; identificar sinais stale, perdas e escopo inacessível.
3. Percorrer dependências e correlação permitida: cliente/fonte → transporte → nó/serviço → fila/dados → adaptador/stream quando aplicável → consumidor. Respeitar autorização regional e residência.
4. Comparar readiness, capacidade, fila/backpressure, erros/latência, mudanças de configuração, estado de replicação e revogação. Não confundir processo vivo com readiness ou ACK técnico com operação concluída.
5. Registrar hipóteses concorrentes, evidência que as favorece/refuta e decisão de contenção. Ações de tráfego seguem o contrato de readiness/balanceamento; qualquer comando operacional requer autorização fora deste documento.
6. Confirmar recuperação com predicados do serviço e resultado observado, não apenas desaparecimento de alerta. Registrar efeito residual, backlog/reconciliação e condição de reabrir.

## Exemplo: conectividade móvel e Guardian

**Hipótese A — transporte indisponível:** esperado é falha de rota/transporte e backlog local crescendo, mantendo estado por serviço degradado/offline. **Hipótese B — serviço não elegível:** transporte funciona, mas permissão, consentimento, capacidade de plataforma ou política de background impede a função; a telemetria deve registrar classe de motivo sanitizada sem localização/sensor bruto. **Hipótese C — pipeline sem cobertura:** ausência de métricas não permite distinguir A de B; resultado é inconclusivo até evidência autorizada adicional.

Para um caso futuro reproduzível, registrar modelo/SO/build, permissões e consentimentos de teste, conectividade inicial, estado foreground/background e energia, sequência controlada de perda/retorno de rede, estado esperado de fila e Guardian, sinais observados e limpeza. Critério esperado: uma falha de transporte não é relatada como revogação de autorização; permissão negada não é relatada como queda de rede; ausência de sinal resulta desconhecido. Alvos temporais e budgets ficam pendentes de baseline. Este exemplo é procedimento proposto, não executado.

### Crosswalk diagnóstico mínimo (proposta)

| Hipótese | Sinais existentes a consultar | Resultado esperado para a triagem documental |
|---|---|---|
| Transporte/rota indisponível | `C13-SIG-003` (transporte) e `C13-SIG-016` (estado offline/fila local), com cobertura/frescor da amostra | Somente amostra válida que indique rota indisponível sustenta a hipótese; a matriz de testes móveis já determina que queda de 4G com Wi-Fi online não seja rotulada como ausência total de conexão. Separar fila/retry de confirmação de entrega. |
| Permissão, consentimento ou capacidade inelegível | `C13-SIG-014`/`C13-SIG-015` (cliente/permissões/background) e `C13-SIG-012` (Guardian/consentimento), conforme plataforma | Motivo sanitizado de negação/inelegibilidade com transporte disponível sustenta hipótese local; não afirmar entrega nem inferir revogação a partir de timeout de rede. |
| Telemetria ausente, atrasada ou contraditória | `C13-SIG-001` e `C13-ALT-001` | Cobertura/frescor insuficiente ou coletor sem heartbeat resulta `unknown`; não atribuir causa a transporte ou permissão, nem afirmar saúde. |

Procedimento futuro: fixar build/SO/configuração, estado de rede e permissão de teste; registrar estado inicial e cobertura; variar uma condição por execução; comparar estado local, rota, tentativa/ACK e consumidor com timestamps e dados sintéticos redigidos; preservar a primeira divergência e confirmar causa somente se sinal discriminante e reprodução concordarem. O vínculo aos sinais existentes não afirma que as fontes estejam implementadas. Resultado observado, limpeza e pass/fail pertencem ao contrato de evidências e permanecem `NOT_EXECUTED` até execução autorizada. Este complemento é o crosswalk por operação de `C13-METHOD-003`; não altera ownership documental dos sinais nem designa on-call.

## Saturação, pico e failover por perfil

Para separar falta de capacidade de falha de dependência, a triagem deve declarar perfil de produto/implantação, regime nominal/pico/degradado, população/mistura de operações e falha exercitada. Comparar, por rota e janela comum, demanda/concorrência, capacidade reservada, CPU/memória/IO/rede, pool wait, profundidade e idade de fila, rejeições/backpressure, p95/p99, cobertura da telemetria e custo/egress quando disponíveis. Sem fonte/cobertura suficiente, a causa fica desconhecida.

| Cenário diagnóstico proposto | Sinais que distinguem hipóteses | Resultado esperado a documentar, sem valor aprovado presumido |
|---|---|---|
| Personal — fila móvel cresce após perda/retorno de rede | estado local/offline, idade/contagem por estado, tentativas de transporte, persistência e aplicação; p95/p99 de sync com marcos distintos | distinguir rede indisponível de armazenamento/projetor lento; timeout sem recibo segue desconhecido, e a carga pós-reconexão deve ser descrita antes de propor limite |
| Teams — rajada de operações em tenant/serviço | concorrência/rota, pool wait, filas, latência e erros por classe segura; comparação com dependências e cobertura | identificar saturação de um serviço sem revelar identidades; admitir ou recusar novas operações conforme reserva/prioridade aprovada, preservando isolamento entre tenants |
| Node — perda de nó/região ou link de federação | readiness/autorização/residência da região alternativa, capacidade reservável, backlog/replay/egress e sessões existentes | diferenciar ausência de capacidade de failover proibido/sem autoridade; bloquear overflow em `unknown`, declarar escopo isolado e registrar reconciliação pendente |

Para cada caso, a decisão em falta é a carga nominal/pico (população, taxa, concorrência e duração), a operação elegível para percentis e sua janela/amostra, a reserva/custo máximo aceitável, os alertas e o responsável operacional. Proposta de contenção: avisar ao aproximar-se da reserva aprovada; retirar novas admissões não essenciais ao atingir o limite aprovado; manter/encerrar operações existentes só pelo contrato de domínio; registrar retry/backoff explícito. Os limites, prioridades por serviço e plantão são decisões pendentes, não thresholds já ativos.

## Região isolada, sync e mídia

Em isolamento global, registrar região/autoridade/escopo em que a conclusão vale, atraso e dados que não puderam ser reconciliados. Não promover cópia local a disponibilidade global nem replicar evidência pessoal fora da política. Em sincronização, separar persistido, enviado, recebido, aplicado/materializado e confirmado; timeout sem confirmação permanece desconhecido. Em mídia, distinguir sinalização/grant, fluxo nativo, bridge e consumidor; ausência de bridge não prova falha de voz nativa. Em SOS, registrar etapas e ACK técnico/humano separadamente, sem inferir atendimento.

## Fechamento e evidência

Incidente só pode marcar causa confirmada quando a evidência diferenciadora foi observada. Encerramento requer estado atual por serviço, impacto residual, filas/replay/reconciliação relevantes, ação de mitigação, proveniência das evidências redigidas, owner e pendências. Resolução aparente sem causa confirmada registra `cause_unknown`, não causa presumida. Campos e status de evidência seguem [contrato de evidências](contrato-evidencias.md).

## Dependências abertas

Inventário e primário documental de métricas/estados/alertas em [observabilidade](observabilidade.md#catálogo-de-sinais-candidatos); predicados e efeito de tráfego em [readiness](readiness.md); políticas de identidade, consentimento, redaction e retenção; contratos de conectividade, sync, mídia e SOS; owners operacionais/runbooks por serviço ainda não atribuídos. GMD-A18-001 permanece parcial até responsáveis de domínio preencherem fontes/sinais específicos e um caso móvel ser aprovado e futuramente reproduzido com evidência.
