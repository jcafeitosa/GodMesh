# M4-B2-A — captura e aplicação transacional do snapshot

A coordenação aprova a [SPEC M4-B2-A v1.1](../../08-specs/federacao/m4-b2-a-snapshot-transacional.md), SHA-256 `5589f5e9364d0243fc7b948ec47190555907f7a863e5460bcbbe424c22b6789f`, com parecer independente favorável e conforme a ADR-075. As dependências [M4-A2](2026-11-18-m4-a2-replica-quic.md#aceite-operacional-em-três-instâncias) e [M4-B1](2026-11-18-m4-b1-snapshot-formato.md#aceite-do-codec) foram aceitas.

1. **Política e migração.** Acrescentar política versionada de comprovante por destino e a referência durável, com unicidade por corte e retenção decidida na primeira aplicação. Verificar instalação, atualização e reexecução sem seed de produção.
2. **Captura.** Fixar o corte em A sob os locks, obter o intervalo completo com a causa da primeira recusa e conferir o tamanho exato antes da assinatura. Testar expiração, limite, abertura de faixa e preservação da paginação A2.
3. **Aplicação e replay.** Verificar o pacote e revalidar gates na transação. Gravar posição, prova, prefixo e referência em conjunto. Recusar sobreposição, conflito, prazo histórico vencido e suspensão `0015`. Confirmar replay exato pelos bytes e dados duráveis do destino.
4. **Aceite.** Usar três bancos e identidades sintéticas, com receptores de histórico conhecido. Exercitar fronteiras de prazo, cancelamento SQL e commit incerto, e as duas ordens de revogação. Rodar workspace, Clippy, invariantes SQL e revisão independente.

A entrada desta fatia é exclusiva do harness. A implementação e seus testes seguem pendentes. Transporte B2-B e reabertura após restore B2-C têm aceites separados.
