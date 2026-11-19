# M1C — prova portátil de ingresso e revogação assinada

**Estado:** rascunho do contrato operacional. A [ADR-076](../../06-planejamento/decisoes.md#adr-076--prova-portátil-e-revogação-de-ingresso) registra a direção adotada. A M1C depende da integração M1B/I e estende a decisão HTTP local para que outro servidor possa verificar e consumir a prova de ingresso. O aceite usa três instâncias com bancos e chaves próprios. Distribuição de configuração e confiança em produção permanece na M3. Replicação de eventos e autorização do Inbox permanecem na M4.

## 1. Escolha do formato

Em `0550be4`, a aprovação v1 assina um `join_request_id` aleatório, resolvido no banco de origem. O `node_id` e a `public_key` assinados pertencem ao aprovador. A assinatura não vincula a chave nem o manifesto do candidato. As quatro colunas de prova também não guardam todos os campos assinados, como `scopes` e `not_before`. Um verificador independente precisa de uma declaração assinada que vincule a decisão ao candidato e que possa verificar integralmente.

| Opção | Efeito |
|---|---|
| **Aprovação v2 vinculada ao candidato** | O aprovador assina a identidade e o manifesto do candidato junto com a decisão. O mesmo artefato serve ao commit local e à verificação remota. A emissão portátil exige que o cliente aprovador adote o formato v2. |
| **Recibo portátil assinado na decisão** | Preserva o contrato v1. O nó aprovador assina uma segunda declaração, que vincula o candidato ao resultado da decisão. Exige coordenar as duas provas. Se o servidor que executa o commit tiver identidade distinta do aprovador, sua assinatura sozinha não prova a autoridade deste e exigiria uma nova regra de confiança. |

**Recomendação:** adicionar a aprovação v2 como caminho explícito para emissão do `join_certificate` portátil. Preservar v1 para decisões locais, com seu contrato e histórico. Persistir a prova v2 completa na mesma transação que confirma a decisão. Aprovações v1 existentes continuam locais. A emissão portátil para esses candidatos exige nova autorização explícita pelo caminho aprovado, com procedimento **OPEN**. Essa opção preserva clientes v1 e usa uma única assinatura de autoridade por novo ingresso portátil.

## 2. Contrato da prova e fluxo

- **Prova assinada.** Especificar versão, domínio exclusivo, ação de aprovação, codificação canônica e limites de tamanho. Cobrir identificador do pedido, cluster, identidade e chave do candidato, digest do manifesto assinado, identidade do aprovador, escopos, janela de validade e epoch. Vincular a prova à credencial de autoridade do aprovador ou à raiz genesis aplicável. Definir como o verificador obtém e valida o manifesto correspondente. A chave do aprovador deriva da cadeia validada.
- **Emissão e entrega.** A decisão exige grant humano `federation:admin` e assinatura do aprovador autorizado. Aplicar a serialização e a revalidação transacional da M1B/I v1.2. Expor ao candidato a prova completa e a cadeia somente após commit, por caminho autenticado e autorizado. Permitir recuperação idempotente após timeout. O protocolo deve definir como o candidato comprova posse da chave na recuperação e no uso da prova. Rotas, códigos de erro e limites de retry ficam **OPEN**.
- **Validação e consumo.** Partir de raízes genesis configuradas fora de banda para o cluster. Verificar cada elo, as assinaturas, a correspondência entre identidade e chave, a validade, a autoridade para delegar, os escopos e o estado de revogação disponível conforme a política aprovada. Exigir cadeia completa, acíclica e limitada. A origem da entrega pode variar. A promoção local só ocorre depois da validação e do commit atômico. Reentregar a mesma prova é idempotente. O candidato permanece isolado enquanto a validação estiver incompleta.

## 3. Revogação assinada

Definir um artefato versionado com domínio distinto, ação de revogação, cluster, alvo inequívoco, identidade do revogador e epoch. Exigir autorização explícita para emitir e validar a autoridade do signatário pela cadeia. Persistir o registro de revogação e o piso monotônico junto com a transição local. Provas antigas e reentregas não reativam acesso. A importação repetida é idempotente e a revogação precisa sobreviver a reinícios.

A M1C deve entregar e consumir a revogação por um caminho oficial verificável nas três instâncias. O protocolo precisa distinguir **assinatura válida** de **informação de revogação atualizada**. Uma cadeia assinada, por si só, não demonstra que o distribuidor apresentou a revogação mais recente. Convergência operacional, distribuição de política e disponibilidade sob partição dependem da M3.

## 4. Decisões OPEN antes da implementação

| Tema | Decisão necessária |
|---|---|
| **Revogação** | A ADR-076 define quem pode revogar, o alvo e o efeito na cadeia. Falta fixar a emissão, a propagação, a ordem durável de decisões, o desempate e como os servidores comprovam frescor. Também falta definir a revalidação no transporte, no ingest e em estados offline ou de cadeia incompleta. |
| **Validade** | Duração, tolerância de relógio, renovação e efeito da expiração de um elo sobre descendentes. Recomenda-se exigir cadeia válida no consumo e limitar a validade do filho à do pai. |
| **Escopos** | Vocabulário e regra de delegação. Recomenda-se declarar a capacidade de aprovar e limitar os escopos delegados aos do aprovador. `federation_peer_scope` continua sendo autorização explícita e independente para o ingest. |
| **Quórum** | A decisão local da M1B/I e o fluxo portátil da M1C seguem o modo explícito de cluster sem votantes. Clusters com votantes dependem dos contratos de quórum da M3. O runtime precisa aplicar esta fronteira em emissão, importação e revogação. |

## 5. Aceite da fatia

1. **Ingresso em três instâncias.** Com bancos e chaves independentes e apenas A configurado como genesis, A aprova B. B recebe e consome a prova pelo protocolo, depois aprova C com autoridade delegada. C verifica A → B → C antes de sair de `isolated`. Verificar a cadeia a partir da raiz em instância distinta da emissora. Os registros de confiança surgem pelo fluxo oficial, sem semear pares ou allowlists por SQL.
2. **Recusa e recuperação.** Verificar assinatura inválida, candidato ou cluster divergente, elo ausente, validade encerrada, escopo excedido e falta de autorização. Conferir ausência de transição durável. Repetir entrega após perda da resposta e verificar o mesmo resultado persistido.
3. **Revogação.** Emitir revogação autorizada e consumi-la pelo protocolo nas instâncias pertinentes. Confirmar recusa de uso posterior da prova atingida, persistência após reinício e ausência de reativação por reentrega. Cobrir descendentes e informação desatualizada conforme as decisões acima.

**Rastreabilidade.** RF-015 → ingresso explicitamente autorizado e portátil. RNF-002 → vínculo entre chave e identidade, validade e revogação, com cobertura parcial. ADR-071 → prova, cadeia, genesis e revogação. ADR-072 → governança e ordem das dependências. O aceite dos enlaces pelo transporte, do ingest com escopo explícito e da replicação permanece condicionado aos contratos das respectivas fatias.
