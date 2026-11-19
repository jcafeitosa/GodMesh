# M4 — preparação da replicação de eventos

**Estado.** Proposta de recorte para planejamento. A implementação depende do aceite operacional da M3 e de SPEC aprovada após revisão independente, conforme as ADRs 072 e 075. O aceite local da M1C-B já demonstrou ingresso, identidade mTLS, vigência e revogação entre três processos.

## Fontes e lacunas no código

| Fonte | Contrato ou comportamento observado |
|---|---|
| [Arquitetura de replicação](../../01-arquitetura/replicacao.md) e [rede-alvo, §§2 e 7](../../09-equipe/rede-alvo-cto.md) | O receptor pede eventos a partir de checkpoint. O envio é escopado por tenant, classe e residência. |
| [ADRs 057, 058, 061, 062, 065, 066](../../06-planejamento/decisoes.md) | Posição canônica, ACK por fronteira, Outbox transacional e autorização de retenção por cópia. |
| [SPEC-004](../SPEC-004-eventos-sincronizacao.md) | Especificação geral ainda em elaboração. Replay histórico, handoff de autoridade, snapshot, tombstones e semântica de destino têm contratos a fechar. |
| `crates/godmesh-server/src/events.rs` | O ingest verifica o `SignedEvent`, autoriza o nó e o escopo e persiste a posição. A linha de `event_position` não contém a assinatura nem `issued_at`; sozinha, não permite reconstruir o envelope para replay autenticado. |
| `crates/godmesh-server/src/inbox.rs` | O Inbox HTTP recebe posições sob grant e confirma o prefixo durável. Conserva o contrato legado. Um receptor federado precisa revalidar as provas e a autoridade de transporte e escrita. |
| `crates/godmesh-sync/src/pg_outbox.rs` e `migrations/0003_outbox_publisher.sql` | Servem ao teste local de publicação e confirmação. A Outbox atual guarda posição e natureza do efeito. O sink de teste não atesta uma cópia remota. |

## Primeiro recorte proposto

Replicar o `SignedEvent` original de uma classe de metadados explicitamente elegível, em streams autorizadas de um tenant de laboratório e dentro da jurisdição declarada. Usar uma origem e dois receptores diretos. Após o commit local, a origem disponibiliza o envelope assinado. Cada receptor busca os próximos eventos pelo próprio prefixo e confirma apenas a cópia durável que gravou. A SPEC deve nomear a classe, os campos e os limites de lote, duração e retry. O envelope atual carrega posição, identidade, instante e metadados de tenant e retenção. Conteúdo de domínio exige um perfil próprio.

O documento de política deve identificar a classe, sua finalidade, as cópias de origem e destino, a residência, o prazo e o descarte. O catálogo hoje distingue cópia primária, secundária e backup. A política só passa a valer para a réplica após vínculo do destino autorizado, da classe e da jurisdição em ambas as pontas. A prova de ingresso e o escopo de ingest, isoladamente, não concedem exportação. Nenhum conteúdo de mensagens, SOS, localização, áudio ou transcrição entra no ensaio inicial.

## Contrato a fechar na SPEC

1. **Publicação.** Persistir prova, posição e disponibilidade de replay na mesma transação de origem. Marcar histórico sem envelope original e exigir início verificável por namespace. Uma posição migrada sem envelope original não ganha assinatura nem compõe o prefixo autenticado. A admissão imediata usa a janela atual do ingest. Replay após atraso precisa de uma política de prova histórica que preserve `issued_at` e revalide autorização corrente de exportação e recepção.
2. **Protocolo.** Definir mensagens versionadas de pull, lote e recibo, trilha do destino autenticado e limites exatos. Usar a identidade mTLS, a autoridade operacional M1C e o piso efetivo M3 antes de transferir ou persistir dados. Proteger cada efeito local até o commit; espera por rede fora da transação. O endereço do nó serve à discagem.
3. **Residência e revogação.** Especificar os vínculos documentados de exportação e importação por origem, destino, tenant, stream, partição, classe e cópia. Exigir autorização corrente em ambos os lados, mesmo no replay. Definir como uma decisão concorrente de revogação se ordena com a exportação e o commit. Recebimento da revogação, parada de novas cópias e descarte de cópias existentes têm evidências distintas.
4. **Idempotência e epoch.** Compartilhar a proteção por namespace com ingest e Inbox. Avançar checkpoint apenas até o prefixo de envelopes duráveis e verificados. Repetição da mesma prova conserva o prefixo. Uma posição com prova ou conteúdo divergente exige resultado explícito. Se já houver posição sem prova, a SPEC fixa a regra de conflito ou a prova explícita exigida para vincular o envelope ao histórico. O primeiro perfil usa atribuição estática, com um escritor e epoch explícito por namespace. Troca de autoridade ou replay entre epochs requer contrato de handoff.
5. **Recuperação.** Tratar resposta perdida por retomada do mesmo destino e namespace, sem repetir efeito externo. Separar lacuna recuperável de intervalo fora da retenção. Definir a condição de parada até que exista snapshot aprovado e paginar sem pular sequência. Registrar resultados de recusa e atraso sem payloads nem segredos.
6. **Aceite.** Após o gate M3, executar A→B e A→C com três processos, chaves e bancos distintos. Produzir evento assinado em A, perder uma resposta depois do commit de B, repetir e provar uma única posição no destino. Particionar C e verificar retomada. Recusar destino, tenant, classe e epoch divergentes, demonstrar revogação por destino e bloqueio de exportação e persistência em sessão já aberta quando a configuração vencer ou o piso superior se tornar exigível. Inspecionar os registros duráveis de cada banco. ACK máximo do receptor é `persisted_local`.

Este plano conserva a integração de snapshots, tombstones, handoff, réplica entre jurisdições e efeitos de domínio para contratos próprios da M4. A M5 tem aceites distintos para nós de borda e relays.
