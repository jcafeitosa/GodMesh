# Contrato de evidências e resultados de validação

**Estado:** formato documental proposto. Nenhum teste, carga ou cenário de aceitação é declarado executado por este arquivo.

## Registro por execução

Cada execução futura deve ter registro separado por caso e incluir:

| Campo | Obrigatório | Conteúdo |
|---|---|---|
| `case_id`, `requirement_id`, `spec_version` | sim | identificadores estáveis e vínculo rastreável; se ainda não existirem, registrar `not_assigned` |
| `documentary_primary`, `interfaces` | quando o caso valida sinal/SLI/alerta | usar o `C13-SIG-*`, `C13-SLI-*` ou `C13-METHOD-*` aplicável e registrar as interfaces da [matriz C13](observabilidade.md#catálogo-de-sinais-candidatos); papéis documentais propostos não são responsáveis operacionais |
| `owner`, `reviewer` | sim, após atribuição da execução | responsáveis por execução e revisão; não inferir nomes nem derivá-los do primário documental |
| `environment` | sim | sistema, região/escopo, topologia, hardware/perfil, rede simulada/real autorizada e isolamento |
| `software_config` | sim | build/revisão, versão de dependências, configuração relevante e feature flags; redigir segredos |
| `scenario_profile` | quando medir capacidade/desempenho | `Personal`, `Teams` ou `Node` (ou `not_applicable` justificado), implantação/topologia, regime `nominal`/`peak`/`degraded`, versão do perfil e decisão que aprovou suas entradas; proposta ainda não aprovada deve ser marcada `proposed` |
| `workload` | quando medir capacidade/desempenho | população e unidade, mistura de operações, taxa/distribuição, concorrência, duração, estado de pico, região/rota e falha exercitada; campos desconhecidos permanecem `not_decided`, não recebem estimativa apresentada como fato |
| `resource_cost_observation` | quando medir recursos/custo | unidade e fonte de CPU, memória, disco, rede/egress, fila/dados/mídia, período/amostra/perdas; custo traz moeda/unidade, fonte e data, separando preço cotado, estimativa e custo observado |
| `preconditions` | sim | estado inicial, identidades/autorização de teste, dados sintéticos e dependências |
| `steps` | sim | ações ordenadas, determinísticas e sem comandos secretos; registrar sequência e versão do procedimento |
| `injected_failure` | quando aplicável | falha controlada, alvo e momento; ausente não significa que resiliência foi exercitada |
| `expected` | sim | estados/eventos/sinais e faixa/limite aprovado, com fórmula e fonte |
| `observed` | sim após execução | resultado bruto redigido, timestamp e artefatos; antes de execução usar `not_executed` |
| `evidence_refs` | sim após execução | logs, métricas, trace, captura ou relatório com escopo, integridade e política de acesso |
| `cleanup` | sim | restauração do estado, descarte de dados sintéticos e verificação de recursos residuais |
| `status` | sim | `PASS`, `FAIL`, `INCONCLUSIVE`, `BLOCKED`, `NOT_EXECUTED`; cada estado tem justificativa |
| `deviation` | quando aplicável | diferença entre o procedimento e execução; reexecução não apaga tentativa anterior |

Esperado e observado são campos distintos. `PASS` exige condição de aceite aprovada e evidência íntegra; `FAIL` registra contradição observada ao esperado; `INCONCLUSIVE` não é pass; `BLOCKED` requer causa e mantém resultado desconhecido; `NOT_EXECUTED` não declara funcionalidade. Um checklist preenchido sem evidência não equivale a teste executado. Não reclassificar bloqueio ou ausência de resultado como sucesso.

Em resultados de latência, indicar operação/população elegível, relógio, componentes incluídos (fila, rede, serviço ou fim-a-fim), janela, método de quantil, tamanho de amostra, cobertura e p95/p99 observado. Um limite esperado só pode ser preenchido quando aprovado; se faltar meta, registrar a distribuição observada como evidência exploratória e o campo de alvo como `not_decided`, sem `PASS` de SLO/capacidade. Para custo, não converter preço público estimado em custo operacional medido.

## Sanitização e custódia

Usar dados sintéticos e contas de teste autorizadas. Redigir tokens, credenciais, PII, áudio/conteúdo, localização precisa e identificadores de visitante antes de exportar. Manter integridade (origem, versão, hash/assinatura quando definida), acesso autorizado e retenção conforme política aprovada. Evidência insuficiente ou sem proveniência não sustenta conclusão positiva. Trilha de auditoria permanece separada do pacote comum de logs.

## Caso mínimo de observabilidade

Cada caso de pipeline deve declarar sinal/fonte e versão, evento elegível, dimensões seguras, estado esperado, cobertura esperada, redaction, falha de fonte/exportador e condição de perda/atraso. Se executado no futuro, observar o dado pelo destino permitido e verificar também a ausência de valores proibidos. Para readiness, registrar mudança de estado e o efeito de admissão esperado. Para recovery, separar backup, réplica, persistência e restauração. Casos de carga devem cobrir o perfil aprovado, os regimes nominal/pico/degradado aplicáveis e resultado de admissão/backpressure, sem confundir a carga configurada com a carga efetivamente alcançada.

## Decisões e status

Owners de domínio e produto definem quais casos são gates, ambientes/versões válidos, limiares aprovados e prazo/controle de retenção. A responsabilidade documental primária por sinal referencia o catálogo C13; executor, reviewer, owner operacional e on-call são campos distintos e não atribuídos por esta tabela. Para fechar os perfis de capacidade, decidir população/topologia Personal/Teams/Node, mistura e duração de pico, falhas que formam o regime degradado, percentis/população/janelas de latência, reservas de recurso/custo e política de alertas. Este modelo não escolhe esses valores. Até haver procedimento completo, decisão e execução autorizada, os casos permanecem propostos ou não executados.

## Relação com a auditoria

Atende parcialmente a forma de registrar resultado/evidência do critério GMD-A14-003; não converte a matriz existente em procedimento reproduzível por cenário nem modifica `05-operacao/testes.md` ou planos de domínio. James Bach e os owners de cada plano precisam integrar o esquema, preencher os casos e preservar estados técnicos/humanos distintos. Nenhuma execução foi realizada.
