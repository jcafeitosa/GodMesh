# Autoridade de escrita por tenant/partição — proposta de contrato

> **Estado: proposta documental para integração e aprovação; não é ADR aprovada nem implementação validada.** O baseline textual normativo em [Replicação § Autoridade de escrita e isolamento](replicacao.md#autoridade-de-escrita-e-isolamento-normativo) exige autoridade por tenant/partição e registra `home_region` + `authority_epoch`; isso não aprova automaticamente os mecanismos detalhados deste contrato. Este documento propõe uma composição operacional com um único escritor vigente por epoch e trata de eleição/transferência sob quorum. Seleção do escritor, consenso, membership/placement, prova e mecanismo concreto de fencing no destino e recuperação continuam propostas até decisão explícita/ADR. Não escolhe países, quantidade de votantes, serviço de consenso, RPO/RTO, SLO, orçamento, nem política de propriedade/residência.

## 1. Escopo e relação com outros contratos

Aplica-se a mutações fortes de posse, ACL, governança e revogação global sobre registros mutáveis sensíveis. Não torna GPS/telemetria eventual, presença/health efêmera, mensagens, SOS ou sessões PTT em escrita forte por padrão; suas semânticas pertencem aos contratos de evento, domínio e operação correspondentes.

- [Replicação](replicacao.md) continua definindo classe de consistência, escopo de dados, snapshots/replay e reavaliação de autorização.
- [Balanceamento](balanceamento.md) continua escolhendo rotas elegíveis; não pode substituir a autoridade lógica pelo nó mais próximo ou saudável.
- [Nós/regiões](nos-regioes.md) descreve topologia e colocações.
- O [contrato proposto de eventos e sincronização](../07-contratos/eventos-sincronizacao.md) descreve eventos, ordem e cursores. A integração com eventos de transição de autoridade e seus cursores ainda deve ser conciliada; a existência do contrato não aprova o wire, o namespace ou os mecanismos de transferência. Este documento trata da autoridade para mutações sensíveis e transferência.

Os links de arquivos ainda não presentes na árvore devem ser integrados por Alan ao concluir a primeira onda de contratos. Não se define aqui outro envelope geral de evento, autenticação de sujeito, política de retenção ou protocolo wire.

## 2. Proposta de composição e alternativas

**Recomendação proposta:** cada partição mutável tem uma `home_region` como posição lógica de autoridade, enquanto um único `authority_node_id` é o escritor atualmente habilitado para aquela partição. A home region pode conter vários full nodes; somente o líder vigente aplica mutações fortes. O estado da atribuição e cada transferência são comprometidos pelo quorum de governança/consenso aplicável. A quantidade de votantes permanece a opção arquitetural documentada de 3 ou 5, sem seleção neste contrato. Partições diferentes podem ter autoridades independentes, se a política de propriedade aprovar esse particionamento.

Esta composição preserva operação regional ativa para escopos diferentes e evita interpretar active-active como multi-writer para o mesmo registro sensível. Não presume que a home region sempre tenha disponibilidade: se o quorum não puder estabelecer ou confirmar autoridade, a mutação forte falha fechada.

| Alternativa | Benefício | Custo/risco | Estado |
|---|---|---|---|
| Home region fixa por partição + líder único na partição | Propriedade clara e localidade previsível; failover não muda residência automaticamente | Dependência da região/quorum e política de transferência; pode aumentar RTT para clientes remotos | Recomendada como proposta, não aprovada |
| Líder por partição sem home region normativa | Flexibilidade de movimentação | Exige definir residência separadamente e impede inferir propriedade pela liderança | Alternativa a deliberar |
| Serviço dedicado global de autoridade | Ponto único lógico para decisões | Custo de disponibilidade, latência, operação e escopo global; possível dependência concentrada | Alternativa a deliberar |

A decisão de usuário/arquitetura ainda é necessária sobre propriedade por tenant/partição, residência, regiões elegíveis e quórum. A proposta não escolhe esses valores.

## 3. Registro de autoridade (campos propostos)

Representação ilustrativa, não esquema de banco nem formato wire aprovado:

| Campo | Tipo | Nulo? | Semântica |
|---|---|---:|---|
| `tenant_id` | identificador opaco | Não | Tenant ao qual a atribuição se limita |
| `partition_id` | identificador opaco | Não | Partição lógica do recurso mutável |
| `home_region` | identificador opaco de região | Não | Colocação lógica aprovada; não equivale ao destino de todo tráfego |
| `authority_node_id` | identificador opaco de nó | Não em `active`; pode ser nulo nos demais estados | Único escritor da partição durante o epoch ativo |
| `authority_epoch` | inteiro sem sinal, monotônico por partição | Não | Cerca atribuições antigas; nunca regride nem se reutiliza |
| `state` | enumeração | Não | `preparing`, `active`, `transferring`, `unavailable` ou `fenced` |
| `committed_at` | instante UTC | Não após commit | Momento de commit da atribuição no quorum; é metadado, não mecanismo de fencing |
| `change_id` | identificador idempotente | Não | Deduplica repetição de pedido de transferência |
| `quorum_proof_ref` | referência opaca a prova/registro assinado | Não em `active` | Referência verificável ao commit da autoridade; formato pertence ao contrato de identidade/GMP |
| `previous_authority_node_id` | identificador opaco de nó | Sim | Diagnóstico/auditoria da transferência; não concede escrita |
| `reason_code` | enumeração/texto limitado | Sim | Razão de transferência ou indisponibilidade; sem dados sensíveis livres |

O contrato wire não é escolhido. A materialização local deve rejeitar campos ausentes, tenant/partição incompatíveis, epoch regressivo, nó não autorizado ou prova de commit inválida. `committed_at` nunca basta para vencer uma partição nem prova que o relógio local está correto.

## 4. Permissão de escrita e invariantes

Uma mutação forte solicita uma credencial de escrita de autoridade conceitualmente equivalente a:

```json
{
  "tenant_id": "tenant-exemplo",
  "partition_id": "acl/org-42",
  "authority_node_id": "node-7",
  "authority_epoch": 18,
  "operation_id": "op-uuid",
  "quorum_proof_ref": "proof-ref"
}
```

Valores são ilustrativos, não IDs reais nem configuração aprovada. Requisições também carregam identidade e prova de autorização do ator segundo o contrato de identidade/autorização; esse material não é substituído pelo token de autoridade.

Invariantes propostas:

1. **Um escritor vigente:** por tenant/partição, há no máximo um escritor que o recurso de destino aceita para o epoch corrente.
2. **Epoch monotônico:** toda transferência que altera o escritor incrementa o epoch; overflow deve bloquear transferência e exigir decisão de migração, nunca voltar a zero.
3. **Fencing no destino:** o armazenamento/adapter que efetiva a mutação compara o epoch e a autoridade com o estado committed; validar apenas no balanceador ou na origem é insuficiente.
4. **Quorum autoritativo:** estado novo de autoridade só fica utilizável depois de commit pelo quorum aplicável. Quorum não disponível significa sem eleição/transferência confirmada.
5. **Sem fallback implícito:** roteamento, health check, RTT baixo ou timeout não promovem automaticamente outro nó.
6. **Idempotência:** repetir `change_id` retorna o resultado já committed; reutilizá-lo com conteúdo diferente resulta em `change_id_conflict`.
7. **Escopo:** tenant e partição são parte da chave da autoridade; prova de um escopo não autoriza outro.
8. **Semântica do registro:** um escritor com epoch obsoleto é recusado mesmo se sua assinatura e credenciais ainda forem válidas.
9. **Revogação:** antes de cada operação forte verifica-se a autorização atual exigida; cache de autorização não pode reabilitar um ator ou autoridade já revogado.
10. **Residência:** transferência de líder não altera `home_region` nem política de replicação/residência sem uma mudança separada e autorizada.

## 5. Estados e transições

| Estado | Significado | Operações fortes |
|---|---|---|
| `preparing` | Partição/autoridade ainda não foi committed ou está sendo recuperada | Recusar |
| `active` | Atribuição de nó e epoch foi committed e prova é verificável | Aceitar apenas do nó/epoch atual, com autorização válida |
| `transferring` | Transferência iniciada, mas novo epoch ainda não foi committed | Recusar mutações que não possam ser linearizadas pelo escritor atual; recomendação fail-closed durante a janela de transferência |
| `unavailable` | Nenhum escritor/quorum atual comprovável | Recusar; permitir apenas eventos locais não fortes e autorizados |
| `fenced` | Autoridade anterior explicitamente invalidada; aguardando nova atribuição | Recusar |

Fluxo proposto de transferência planejada: `active(e,n1)` → `transferring` → parar/admitir conclusão das mutações já committed pelo escritor atual → commit quorum de `active(e+1,n2)` e da prova → aplicar fencing no recurso de destino → anunciar a nova atribuição. A nova autoridade só aceita escrita depois do commit e do fencing no destino. A transferência pode permanecer indisponível entre esses passos; não há promessa de failover sem interrupção.

Falha abrupta: se quorum confirma que a autoridade antiga foi invalidada, commit de nova atribuição usa epoch maior e o destino cerca o escritor antigo antes de aceitar escritas. Sem quorum ou sem fencing verificável no destino, permanece `unavailable`/`fenced`; não se declara sucessão por timeout.

## 6. Erros e comportamento de falha

| Código conceitual | Condição | Resultado |
|---|---|---|
| `authority_unavailable` | Sem atribuição `active` comprovável ou quorum necessário indisponível | Recusar mutação forte; conservar evento local permitido com estado pendente, se aplicável |
| `authority_epoch_stale` | Epoch da requisição menor que o committed | Recusar e devolver epoch corrente sem revelar dados fora do escopo autorizado |
| `authority_node_mismatch` | Nó remetente não corresponde ao escritor committed | Recusar e registrar evidência mínima de auditoria |
| `authority_proof_invalid` | Prova ausente, inválida, não committed ou escopo incompatível | Recusar |
| `authority_transfer_in_progress` | Partição em transição | Recusar ou enfileirar somente operação explicitamente enfileirável e idempotente; não alegar commit |
| `authorization_revoked` | Ator ou capacidade revogada | Recusar; invalidar cache conforme contrato de identidade |
| `change_id_conflict` | Mesmo `change_id` com conteúdo divergente | Recusar e sinalizar inconsistência |

Falha de comunicação com o quorum não deve ser convertida em autorização local. Falha do recurso que não consegue consultar/validar epoch deve bloquear a mutação sensível. Leituras podem usar réplica conforme consistência declarada e mostrar sua proveniência/defasagem; não se devem inferir leituras lineares a partir desse contrato.

## 7. Exemplos de conformidade documental

### Positivo — mutação na autoridade vigente

Dada a partição `(tenant-exemplo, acl/org-42)` em estado `active`, epoch 18, nó `node-7`, com prova committed válida e ator autorizado, uma alteração é aplicada uma vez. Retry com o mesmo `operation_id` retorna o mesmo resultado; não cria segunda alteração.

### Negativo — nó antigo após transferência

Após commit de epoch 19 para `node-9`, uma requisição assinada de `node-7` com epoch 18 é recusada como `authority_epoch_stale`, mesmo que chegue a outra região saudável.

### Negativo — partição minoritária

Região isolada sem quorum tenta alterar ACL global. A mutação falha fechada. Evento de observação local não sensível pode ser persistido como pendente segundo política de sync, mas não altera ACL nem se apresenta como committed.

### Negativo — eleição por timeout

O balanceador observa timeout do líder e encaminha a escrita para outro full node sem prova de novo epoch. Receptor recusa; saúde/latência não são autoridade.

### Negativo — cross-tenant replay

Uma prova válida para `(tenant-A, partition-X)` acompanha pedido para `(tenant-B, partition-X)`. O receptor recusa por incompatibilidade de escopo.

## 8. Casos de conformidade a validar quando houver implementação autorizada

São critérios documentais propostos, não testes executados:

- Em uma única atribuição ativa, a mutação válida do nó/epoch atual é aplicada uma vez; replay idempotente não duplica.
- Após transferência committed, todos os caminhos de escrita do epoch anterior são recusados pelo destino; teste inclui retorno atrasado do nó antigo.
- Sem quorum, não se observa commit de nova autoridade nem mutação forte na região isolada; operações locais permitidas permanecem claramente pendentes.
- Queda do líder antes, durante e depois de commit resulta em estado determinístico recuperável; nenhuma sequência promove dois escritores aceitos.
- Prova inválida, expirabilidade não especificada, escopo trocado, epoch regressivo e revogação são recusados.
- Retorno da região restaura replay autorizado sem converter eventos locais em mutações fortes e sem sobrepor dados de autoridade mais recente.

## 9. Decisões ainda abertas e prontidão

O usuário/arquitetura precisa escolher: chave e regra de particionamento/propriedade; política de atribuição de `home_region` e migração de residência; conjunto de votantes, posicionamento em domínios de falha e mecanismo de consenso; se todas as escritas fortes requerem quorum no commit ou se há outro modelo de commit linearizável; formatos e expiração de provas; orçamento de latência e disponibilidade por operação; RPO/RTO/SLO e política de recuperação; semântica de eventos locais em conflito. Nenhum valor é afirmado aqui como medido ou aprovado.

**Bloqueio de prontidão:** não anunciar autoridade/failover de produção até ADR decidir essas opções, contratos de sync/autorização definirem o intercâmbio de provas e replay, e evidências futuras cobrirem partições assimétricas, perda/retorno do líder, fencing no destino e revogação. Nada neste documento valida implementação.
