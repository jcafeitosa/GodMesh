# M4-B — mapa de snapshot assinado de corte fixo

**Estado:** proposta documental para revisão. Implementação após aceite operacional A2, fechamento do contrato de wire e aprovação por fatia da ADR-075, na ordem da ADR-072. Este mapa não registra aceite A2.

## Recorte executável

Demonstrar snapshot para **bootstrap e restore de estado sintético ainda elegível**. Uma origem A, dois receptores B/C, uma reserva A1, epoch estático, mesma jurisdição e um pacote limitado por destino. O snapshot complementa a cópia incremental A2. A suspensão durável `0015` fica fora da transição positiva deste piloto.

- **Bootstrap.** Destino provisionado pela A1, com checkpoint 0 e sem suspensão, importa provas originais consecutivas de 1 até o corte assinado.
- **Restore.** Partir de cópia consistente de laboratório com reserva, provas locais demonstráveis, checkpoint P e estado de bloqueio preservados. Aplicar a faixa de P+1 até o corte somente se o estado restaurado não estiver suspenso. A revisão do procedimento deve excluir rollback de proteção. Um backup anterior à suspensão não serve como evidência de que ela deixou de existir.
- **Corte fixo.** A escolhe o corte no prefixo durável sob os locks. O manifesto fixa a fronteira inicial e final. Publicações posteriores ficam para o Pull A2. Se a faixa comprometida estiver indisponível ou não couber nos limites, recusar o pacote inteiro. Após recusa, uma nova seleção exige nova decisão e novo manifesto. O receptor não ativa parte do pacote nem muda o corte recebido.

## Integridade, autoridade e efeitos

O manifesto do snapshot é assinado por A em domínio distinto, proposto como `godmesh.federation.snapshot.v1`, com versão de perfil, identidade de A e do destino, namespace completo, epoch, prefixo base, corte final, quantidade e digest ordenado dos envelopes. Reusar TLV canônico, SHA-256 e Ed25519. Calcular o digest sobre os bytes originais de cada envelope precedidos de comprimento explícito. Verificar a assinatura do manifesto contra a chave autenticada de A e cada `SignedEvent` pelo domínio e chave de origem existentes. Alteração de whitespace, bytes escapados, ordem, quantidade, destino ou corte invalida o pacote correspondente. O manifesto prova integridade e escopo do pacote. A durabilidade no receptor é verificada no commit.

Em A, sob `READ COMMITTED`, adquirir guard de cluster M1C e depois lock A1 do namespace. Amostrar um segundo após ambos. Verificar autoridade local e remota M1C, piso M3 efetivo, reserva, ACL de exportação, residência e retenção aplicável à origem e ao destino. Capturar e assinar o pacote de posições confirmadas e publicadas sob essa decisão. Transferir fora da transação, na sessão QUIC oficial autenticada, com limites explícitos de quantidade, bytes por prova, pacote e prazo da tentativa.

No receptor, verificar sessão e manifesto, então revalidar todos os gates, inclusive ACL de importação e retenção por cópia, sob os mesmos locks e um segundo amostrado após adquiri-los. Reler reserva, checkpoint e `0015`. Exigir checkpoint igual ao prefixo base, faixa contígua até o corte e ausência de suspensão. Persistir provas originais, posições, checkpoint e auditoria em um commit. Manter prazo e gatilho conforme política aplicável, sem renovar TTL por transferência ou replay. Revogação, vencimento, inconsistência ou falha antes do commit deixam o estado anterior íntegro.

`persisted_local` só atesta a faixa de provas confirmada naquele destino. Em replay exato com provas locais ainda demonstráveis e autorização corrente, retornar confirmação idempotente, sem regravar nem avançar de novo. Em commit incerto, reler o destino. A origem não ganha garantia durável de cópia. O piloto não adiciona rota de política, nova dependência, autoridade de escrita, troca de epoch ou origem alternativa.

## Perda legítima e suspensão 0015

`HistoryUnavailable` vem do expurgo ou da expiração primária reconhecidos por `usable()` na A1, ou da recusa da janela secundária na seleção. Uma lacuna sem prova em slot vazio é `SequenceGap`. Troca de política, relógio adulterado e ausência de prova não são casos positivos de recuperação de `0015`.

| Situação | Efeito |
|---|---|
| Prova expurgada ou vencida, inclusive cópia física sob hold ou backup sem elegibilidade | Preservar a posição perdida, o prefixo e a suspensão. Marcador ou hash pode explicar a indisponibilidade. Não substitui o evento nem recebe ACK. |
| Snapshot íntegro apresentado a destino suspenso | Recusar aplicação neste perfil. Retirada de `0015` exige contrato separado e um cenário autorizado e reproduzível que demonstre a disponibilidade legítima da prova original perdida, sem contornar retenção. Até lá, manter o bloqueio. |
| Necessidade de retomar apesar de perda definitiva | Decisão futura de produto: autorizar ou recusar abertura explícita de novo namespace, com política, autoridade e descontinuidade visível. Se aprovada, o novo namespace começa com checkpoint próprio. O bloqueio e a posição perdida permanecem no namespace anterior. |

O usuário precisa aprovar essa descontinuidade antes de um procedimento de novo namespace. O recorte sintético de bootstrap/restore não exige essa decisão nem autorização para dados reais.

## Subtarefas e evidências planejadas

1. **Formato e fixture.** A/B/C descartáveis, M1C oficial, M3 efetiva, reserva e ACLs sintéticas. Fixar vetor independente do manifesto e dos bytes originais, incluindo whitespace e escapes.
2. **Bootstrap de corte fixo.** Capturar faixa retida em A, importar em B com prefixo 0 e continuar por A2 acima do corte. Publicar uma posição em A após a captura e provar que ela ficou fora do snapshot.
3. **Restore e atomicidade.** Restaurar C de estado sintético consistente com prefixo positivo, preservando os controles locais. Aplicar a faixa elegível até o corte. Exercitar rollback, perda da resposta, replay e partição. Recusar manifesto adulterado, destino divergente, expiração ou revogação entre captura e aplicação.
4. **Perda legítima.** Produzir `0015` pelo fluxo A2 com expiração primária, descarte e recusa da janela secundária em rodadas separadas. Verificar recusa de snapshot, ausência de ACK, prefixo e posição bloqueada preservados após reinício. Confirmar que `SequenceGap` continua distinto.
5. **Gate e revisão.** Registrar separadamente o aceite A2 quando concluído. Fechar wire, prazos e limites da fixture antes da aprovação M4-B. Manter a retirada de suspensão e a abertura de novo namespace fora deste aceite.

**Rastreabilidade.** ADR-057/058 → posição, prefixo e ACK. ADR-061 → commit transacional. ADR-062/065/066 → retenção, descarte e residência. ADR-072/075/076 → ordem, aprovação e identidade. A1 → prova original e semântica de indisponibilidade. A2 → cópia incremental e `0015`. Nenhum ensaio M4-B foi executado.
