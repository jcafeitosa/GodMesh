# M4-A2 — transporte da réplica de metadados

A coordenação aprova o [wire M4-A2 v0.3](../../08-specs/federacao/m4-a2-replica-quic.md) no SHA-256 `be34dd612ff93075ddaf644941452755145237c7f932e9f4940c0f614143f9d7` e o [delta de runtime](../../08-specs/federacao/m4-a2-delta-runtime.md) no SHA-256 `267beec97faf622a6b420269e440c3d12651a00f58f0d290a46e46d5a60bed7f`, sob a autorização por fatia da ADR-075. A revisão independente conferiu os contratos e os deltas. A [M3-B2](2026-11-18-m3-b2-integracao-operacional.md#aceite-em-três-instâncias) e a [M4-A1](2026-11-18-m4-a1-persistencia-transacional.md#aceite-da-persistência) são as dependências aceitas.

1. **Mensagens.** Implementar o codec JSON estrito, a correlação da tentativa e a negociação dos três tetos. Preservar cada envelope original incluindo whitespace externo e escapes. Testar limites, truncamento, campos inválidos e a paginação sem salto.
2. **Sessão QUIC.** Habilitar o dispatcher após promoção operacional, preservando manifesto e heartbeat. Controlar abertura e aceitação de streams, framing, FIN, cotas e cancelamento por sessão. Testar uma stream antecipada e o isolamento de enlaces paralelos.
3. **Efeitos transacionais.** Obter o mesmo guard M1C e o lock de namespace antes de amostrar o segundo. Revalidar M1C, piso M3 e ACL por efeito, com prazo SQL local. Usar o checkpoint A1 e gravar a suspensão da `0015` quando a próxima prova fica indisponível.
4. **Runtime.** Validar opt-in, política e reserva antes do bind. Rodar pull por destino e namespace, com uma tentativa em voo, retry limitado e rechecagem de ACL. Emitir ACK `persisted_local` após commit, com retomada pelo checkpoint.
5. **Aceite.** Testar A/B/C com processos, chaves, bancos e políticas sintéticas próprios. Exercitar perda de ACK, partição, revogação isolada, suspensão após reconexão e reinício, bloqueio M3 e ausência de escrita parcial. Registrar suíte, Clippy, SQL, revisão independente e ensaio do SHA integrado.

Na verificação preparatória, A obteve o registro de autoridade local por `POST /v1/federation/join-requests` (`201`) e aprovação M1B assinada (`200`). Após reiniciar em M1C com a mesma chave e o mesmo banco, o runner autenticou um grant sintético e usou o pin local para autorizar a admissão A1. O evento produziu posição, prova publicada e prefixo em um commit. O ensaio preparatório confirma essa etapa do bootstrap. As verificações de M3 efetiva e cópia no fio ficam no aceite A2.

## Aceite operacional em três instâncias

O código revisado foi integrado localmente em `9941d0fd10a156766b6dcab0a2a4a0298de9da49`. A migração aditiva `0015` registra a suspensão no receptor e preserva o checkpoint. Os checksums das migrações `0001` a `0014` e o `Cargo.lock` foram preservados.

Os ensaios em IPv4 e IPv6 usaram três processos no mesmo host, com bancos PostgreSQL 16.15 descartáveis, chaves e políticas sintéticas próprias. A obteve seu registro local pelas APIs M1B e B/C ingressaram pelas APIs M1C. Os três ativaram o piso M3 antes da réplica. O boot com dependências válidas e reserva ausente falhou antes de escutar. Os testes do binário também confirmaram saída 78 para configuração inválida, inclusive cota acima do teto.

1. A publicou pela admissão A1. B e C confirmaram seus próprios prefixos. A observou ACK `persisted_local` no QUIC. A consulta nos três bancos confirmou igualdade byte a byte do envelope, incluindo whitespace externo.
2. C ficou offline enquanto B avançava. Ao voltar, C retomou do seu checkpoint com a contagem exata de posições. A revogação de exportação para B recusou novas cópias para B enquanto C avançava. B retomou após concessão explícita.
3. Após o descarte em A da posição 5 ainda necessária em C, C gravou `blocked_next_seq=5` e conservou o checkpoint 4. Reinício e nova sessão mantiveram a suspensão, sem novo Pull de C. B continuou.
4. O piso 2 encerrou enlaces operacionais. A atualização pelo HTTPS mTLS permitiu restaurar o piso 1. B confirmou a posição 7. C permaneceu suspenso.
5. Em compilação instrumental do mesmo commit, a interrupção entre o commit de B e o envio do ACK deixou duas provas e checkpoint 2 no banco de B. A não observou o ACK 2. Após reiniciar, B solicitou somente posições acima de 2. Em outra injeção do mesmo ensaio, duas falhas transitórias locais consumiram retentativas dentro do ciclo. Um erro remoto de armazenamento aguardou a cadência normal. O protocolo preserva `storage_unavailable` para armazenamento e `authority_blocked` para autoridade ou piso.

| Verificação do commit | Resultado |
|---|---|
| `cargo test --locked --offline --workspace` em PostgreSQL descartável | 595 aprovados, zero falhas e um ignorado por desenho. |
| Teste ignorado de política ausente, em banco próprio | Um aprovado. |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | Exit 0. |
| `bash migrations/tests/run_all.sh` | 64 invariantes em banco criado do zero. |
| Regressão M3-B2 em três processos IPv6 | Publicação mTLS, Pull/304, bloqueio pelo piso, recuperação HTTPS, partição, conflito e encerramento sob lock PostgreSQL passaram. |

A revisão independente aprovou a limpeza de conexão PostgreSQL sob cancelamento, a retenção do lock pelo permit e a correção da classificação no wire. O teste cancelou repetidas admissões contra um pool de uma conexão enquanto o bloqueador externo continuava ativo. Uma consulta independente voltou a obter sessão limpa antes de liberar o bloqueador.

A primeira execução IPv6 expirou aguardando a confirmação inicial em B/C. Não foi possível determinar a causa pelo registro daquela execução. Duas novas execuções do binário do commit passaram, além do ensaio instrumental em IPv6. O aceite é da réplica direta de metadados sintéticos. Snapshots, políticas com dados reais, relay e o ensaio final em três hosts Linux independentes seguem seus próprios gates.
