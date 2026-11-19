# M4-A1 — persistência transacional da réplica de metadados

A coordenação aprova a [SPEC M4-A1 v1.1](../../08-specs/federacao/m4-a1-persistencia-transacional.md) no SHA-256 `e4ff32583688d7d5667061424cdea2585cc51d9b123a188151efc133c9b458ed`, sob a autorização por fatia da ADR-075. A revisão independente aprovou essa versão. A dependência de [aceite operacional da M3-B2](2026-11-18-m3-b2-integracao-operacional.md#aceite-em-três-instâncias) foi cumprida em três processos.

A fatia usa bancos descartáveis, eventos de metadados sintéticos e políticas de teste específicas por cópia. A migração será aditiva. Transporte, ACK no fio e aceite em três servidores pertencem à [proposta M4-A2 v0.2](../../08-specs/federacao/m4-a2-replica-quic.md), revisada e condicionada ao aceite da A1. Para dados reais, seguem pendentes a classe, a finalidade, a residência, os prazos e o descarte, conforme ADR-062 e ADR-065.

1. **Reserva e migração.** Persistir reserva por namespace, referências de versão, ACLs e evidências de cópia. Usar o mesmo lock do ingest e Inbox para recusar escrita legada em qualquer epoch. Verificar histórico preexistente, replay e as duas ordens da corrida entre reserva e escrita.
2. **Origem e política.** Verificar grant, escopo, chave, sequência, epoch e retenção por cópia antes do efeito. Gravar a posição, o envelope assinado e a disponibilidade para exportação no mesmo commit. Exercitar rollback e política ou autorização divergente.
3. **Seleção e cópia.** Exportar somente para destino autorizado. Medir os limites do lote e preservar prefixos independentes. Na aplicação, validar a prova original, a sobreposição e o sufixo antes de estabelecer o novo checkpoint. Verificar replay após perda do resultado, lacuna e envelope que excede o teto.
4. **Descarte.** Usar a política histórica e o hold corrente. Testar a igualdade da expiração, descarte fora da ordem de sequência, versão substituída ou retirada e revogação de ACL. Conferir que os marcadores impedem salto e reaproveitamento de sequência.
5. **Integração.** Rodar casos positivos, negativos, de borda e de recuperação em três bancos locais. Conferir migração limpa, reexecução, regressão do workspace, Clippy e revisão independente do candidato.

O registro abre o gate de implementação da A1. Testes da implementação seguem pendentes.
