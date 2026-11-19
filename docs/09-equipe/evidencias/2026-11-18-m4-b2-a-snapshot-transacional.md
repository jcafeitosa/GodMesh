# M4-B2-A — captura e aplicação transacional do snapshot

A coordenação aprova a [SPEC M4-B2-A v1.1](../../08-specs/federacao/m4-b2-a-snapshot-transacional.md), SHA-256 `5589f5e9364d0243fc7b948ec47190555907f7a863e5460bcbbe424c22b6789f`, com parecer independente favorável e conforme a ADR-075. As dependências [M4-A2](2026-11-18-m4-a2-replica-quic.md#aceite-operacional-em-três-instâncias) e [M4-B1](2026-11-18-m4-b1-snapshot-formato.md#aceite-do-codec) foram aceitas.

1. **Política e migração.** Acrescentar política versionada de comprovante por destino e a referência durável, com unicidade por corte e retenção decidida na primeira aplicação. Verificar instalação, atualização e reexecução sem seed de produção.
2. **Captura.** Fixar o corte em A sob os locks, obter o intervalo completo com a causa da primeira recusa e conferir o tamanho exato antes da assinatura. Testar expiração, limite, abertura de faixa e preservação da paginação A2.
3. **Aplicação e replay.** Verificar o pacote e revalidar gates na transação. Gravar posição, prova, prefixo e referência em conjunto. Recusar sobreposição, conflito, prazo histórico vencido e suspensão `0015`. Confirmar replay exato pelos bytes e dados duráveis do destino.
4. **Aceite.** Usar três bancos e identidades sintéticas, com receptores de histórico conhecido. Exercitar fronteiras de prazo, cancelamento SQL e commit incerto, e as duas ordens de revogação. Rodar workspace, Clippy, invariantes SQL e revisão independente.

## Aceite transacional

O commit `56687b2d36a1373a10fca3274f160b228d04ee14` foi integrado localmente após parecer independente. A migração aditiva `0016` registra políticas e comprovantes, sem seed. O vínculo por versão, escopo e jurisdição impede apagar uma política ainda referenciada ou associar o comprovante a outra série. A retirada usa `active=false`. A troca por prazo maior não renova o vencimento histórico.

O harness usou três bancos PostgreSQL 16.15 e identidades sintéticas. A captura fixou o corte antes da publicação posterior e distinguiu limite de histórico indisponível. B partiu do checkpoint 0. C usou uma base positiva já confirmada. A aplicação gravou a faixa e a referência no mesmo commit. A falha no último `INSERT` reverteu posição, provas e checkpoint. O replay exigiu bytes idênticos, política corrente, validade original e ausência de suspensão `0015`. As barreiras cobriram revogação em ambas as ordens e esgotamento do prazo após verificação B1.

| Verificação do commit | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` em PostgreSQL descartável | 608 aprovados, zero falhas e um ignorado por desenho. |
| Teste ignorado de política ausente, em banco próprio | Um aprovado. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 70 invariantes em banco criado do zero. |
| Atualização pelo binário da `0015` para a `0016` | Histórico anterior, posição legada, reserva, checkpoint 0 e suspensão 1 preservados. A reexecução aplicou zero migrações. |
| Regressão A2 em três processos IPv6 com a `0016` | Duas execuções completas do binário do commit confirmaram cópia, partição, revogação, suspensão e recuperação via HTTPS. |

Na regressão A2, a fixture original usava 120 ms para a rechecagem de configuração M3. Uma execução persistiu a primeira cópia nos dois destinos e ficou sem uma confirmação observada pela origem. Outra expirou aguardando a posição 5 em B. A compilação instrumental reproduziu um caso com ACK e FIN recebidos antes de interromper a observação final. A causa exata do encerramento não foi isolada. As duas execuções completas usaram 750 ms na configuração M3 da fixture, com os demais critérios conservados e sem alteração de código ou configuração de produto.

A entrada de snapshot desta fatia só existe no harness. Transporte B2-B e reabertura após restore B2-C têm aceites separados.
