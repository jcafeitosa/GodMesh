# M4-A1 — persistência transacional da réplica de metadados

A coordenação aprova a [SPEC M4-A1 v1.1](../../08-specs/federacao/m4-a1-persistencia-transacional.md) no SHA-256 `e4ff32583688d7d5667061424cdea2585cc51d9b123a188151efc133c9b458ed`, sob a autorização por fatia da ADR-075. A revisão independente aprovou essa versão. A dependência de [aceite operacional da M3-B2](2026-11-18-m3-b2-integracao-operacional.md#aceite-em-três-instâncias) foi cumprida em três processos.

A fatia usa bancos descartáveis, eventos de metadados sintéticos e políticas de teste específicas por cópia. A migração será aditiva. Transporte, ACK no fio e aceite em três servidores pertencem à [proposta M4-A2 v0.2](../../08-specs/federacao/m4-a2-replica-quic.md), revisada e condicionada ao aceite da A1. Para dados reais, seguem pendentes a classe, a finalidade, a residência, os prazos e o descarte, conforme ADR-062 e ADR-065.

1. **Reserva e migração.** Persistir reserva por namespace, referências de versão, ACLs e evidências de cópia. Usar o mesmo lock do ingest e Inbox para recusar escrita legada em qualquer epoch. Verificar histórico preexistente, replay e as duas ordens da corrida entre reserva e escrita.
2. **Origem e política.** Verificar grant, escopo, chave, sequência, epoch e retenção por cópia antes do efeito. Gravar a posição, o envelope assinado e a disponibilidade para exportação no mesmo commit. Exercitar rollback e política ou autorização divergente.
3. **Seleção e cópia.** Exportar somente para destino autorizado. Medir os limites do lote e preservar prefixos independentes. Na aplicação, validar a prova original, a sobreposição e o sufixo antes de estabelecer o novo checkpoint. Verificar replay após perda do resultado, lacuna e envelope que excede o teto.
4. **Descarte.** Usar a política histórica e o hold corrente. Testar a igualdade da expiração, descarte fora da ordem de sequência, versão substituída ou retirada e revogação de ACL. Conferir que os marcadores impedem salto e reaproveitamento de sequência.
5. **Integração.** Rodar casos positivos, negativos, de borda e de recuperação em três bancos locais. Conferir migração limpa, reexecução, regressão do workspace, Clippy e revisão independente do candidato.

## Aceite da persistência

A implementação revisada está em `8693e52200148a900040df44404383b8676dd162`. A migração `0014` adiciona as tabelas de reserva, política versionada, ACL, prova, checkpoint, hold e disposição. A atualização de um banco que já tinha a `0013` preservou as linhas de posição e checkpoint legados e o histórico das 13 migrações. A reexecução confirmou que a `0014` foi aplicada uma única vez. As migrações anteriores e o lockfile mantiveram os mesmos bytes.

Nos testes em três bancos descartáveis, A gravou posição e envelope assinado original na mesma transação. B e C mantiveram checkpoints independentes. Replay preservou o prazo do primeiro commit. A autorização para B pôde ser revogada enquanto C continuou elegível. Grant com escopo válido e predicado negado deixou posição, prova e checkpoint vazios. Descarte obedeceu à política histórica e ao hold corrente, mesmo após revogação da ACL.

O ingest e o Inbox retornaram `409 event_namespace_reserved` para os dois epochs ensaiados, com zero posições ou checkpoints novos. Com barreiras do PostgreSQL, a reserva que confirmou primeiro bloqueou o escritor. Na ordem inversa, a escrita legada confirmou e a reserva retornou `LegacyHistory`.

Os testes de retenção primária e secundária reproduziram a perda da prova vigente antes da correção. No candidato aceito, a primeira página conserva a sequência 1 com `has_more=true`. Ao pedir a sequência 2 indisponível, o receptor receberá `HistoryUnavailable`; a sequência 3 permanece fora da página. O resultado é do serviço interno; o transporte dessa resposta pertence à A2.

| Verificação do commit | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` em PostgreSQL 16.15 descartável | 568 aprovados, zero falhas e um ignorado por desenho. |
| Teste ignorado de política ausente, em banco próprio | Um aprovado. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 60 invariantes em banco criado do zero. |
| Regressão M3-B2 com três processos em IPv6 | Publicação, `304`, bloqueio de QUIC/ingest, recuperação, partição e conflito passaram com a `0014`. |

A revisão independente aprovou o delta final após conferir as duas ordens entre escrita e reserva e a paginação com expiração fora de ordem. A A1 conclui o serviço transacional local com dados sintéticos. O bootstrap e a cópia entre processos com ACK no fio pertencem à A2.
