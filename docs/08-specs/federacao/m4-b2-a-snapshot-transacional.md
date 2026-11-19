# SPEC proposta M4-B2-A v1.0 — snapshot transacional em destino elegível

**Estado:** proposta para revisão e aprovação conforme ADR-075. Depende do codec B1 integrado e aceito, do aceite A2 e do [mapa B2 revisado](m4-b2-recorte-restore.md) em `a676d03`, na ordem da ADR-072. Esta SPEC não declara aprovação de implementação.

**Recorte.** Capturar, aplicar e confirmar replay de snapshot B1 de metadados sintéticos, em serviço PostgreSQL interno. Uma origem A, dois destinos B/C, uma reserva A1, epoch fixo e mesma jurisdição. O harness transfere bytes entre três bancos descartáveis em processo de teste. Não há socket, wire, listener, rota administrativa, restore, reabertura, novo mecanismo de autoridade ou dependência.

## 1. Pré-condição de elegibilidade

O ponto de entrada da B2-A fica restrito ao harness. O harness cria ou provisiona destinos descartáveis de histórico conhecido, sem restaurar backups. Fornece contexto explícito da fixture para A, B/C e namespace. O serviço revalida M1C local/remota, M3 efetiva, ACL, residência, reserva e `0015` no banco de cada efeito. Identidades esperadas vêm do contexto confiável da fixture, nunca do manifesto. Nova prova M1C, ausência de linha `0015` ou snapshot íntegro, isoladamente, não demonstram que um destino restaurado está elegível.

**Dependência do gate.** Sem evidência de que o receptor é elegível, a aplicação é proibida. A B2-A consegue testar a aplicação em destino criado no próprio ensaio. Conectar o serviço a um destino que possa ter sido restaurado exige o gate da B2-C antes de qualquer aplicação normal. Estado desconhecido ou restore incompleto permanece bloqueado. A B2-A não expõe opção que declare um restore seguro nem liga o serviço ao runtime de produção.

## 2. Captura integral em A

Receber destino, reserva/epoch, base pretendida e limites explícitos do perfil B1 e da tentativa. A base vem do receptor no ensaio, sem inventar um checkpoint remoto durável em A. Sob o guard da seção 4, fixar o corte no prefixo confirmado em A. Exigir corte maior que a base e obter a faixa completa `base + 1 ..= corte`, sem suprimir qualquer posição. Revalidar retenção primária e janela da cópia secundária pretendida segundo a política A1, sem reaplicar ao histórico a janela online de `issued_at`.

Reusar a seleção A1 ou um equivalente compartilhado sob o guard. Mesmo que a seleção devolva uma página utilizável, recusar se `has_more=true`, se a faixa não começar e terminar nos números esperados ou se a contagem for menor que `corte - base`. Em caso de falta de prova, lacuna, expurgo ou limite, retornar a falha correspondente. Não reinterpretar paginação A2 como corte, não truncar a faixa para caber e não assinar pacote parcial.

Montar e assinar o manifesto B1 com a identidade A da fixture e os bytes originais. O prazo da janela assinada é explícito na fixture, sem default de produção. Exigir `not_before <= now < expires_at` na captura. Entregar o pacote preparado somente depois de concluir com sucesso a transação de captura. A captura não modifica prefixo nem grava recibo remoto em A.

## 3. Aplicação e replay no destino

Verificar pacote pelo codec B1 antes de abrir a transação quando possível, mantendo bytes e resultado imutáveis. Sob o guard, verificar a janela assinada no instante do efeito, retenção por cópia, ACL corrente e reserva. Bloquear imediatamente se houver `0015`, inclusive replay com comprovante antigo. Preservar a suspensão, próxima sequência bloqueada e checkpoint. Recepção ou repetição do snapshot não libera a suspensão.

**Referência durável.** Na aplicação nova, gravar no destino um comprovante mínimo com chave do destino/namespace/epoch, base, corte, quantidade, digest do corpo canônico assinado do manifesto e digest SHA-256 dos bytes integrais do pacote B1. Usar os campos de identidade do pacote autenticado. A referência confirma exatamente o manifesto e o pacote aplicados. Não duplicar envelopes na referência. A migração é aditiva. A fixture declara retenção para a referência; sem política efetiva, recusar a aplicação. Referência vencida ou ausente não dá direito a replay. Nenhum dado de produção é semeado.

| Estado sob lock | Decisão |
|---|---|
| Checkpoint igual à base e sem comprovante conflitante | Aplicação nova. Reusar `Replication::apply_copy_in_tx` com o mesmo guard e segundo, exigindo retorno com `inserted == event_count`, primeira posição igual a `base+1`, última igual ao corte e prefixo igual ao corte. Qualquer divergência faz rollback, inclusive sobreposição que a A1 normalmente aceitaria. Gravar provas, posições, prefixo e referência na mesma transação. |
| Checkpoint igual ao corte e comprovante durável exato | Replay somente se cada posição estiver presente, elegível e byte a byte igual à fatia original do pacote. Usar a leitura de `store::slot`/`usable` ou equivalente compartilhado, com a mesma transação e guard. O comparador canônico + assinatura de `confirm_replay` da A1 não basta para B1, pois o digest distingue whitespace e escapes. Não alterar o contrato de replay A1. |
| Checkpoint igual ao corte sem comprovante exato, abaixo da base, entre base e corte ou acima do corte | Recusar sem escrita. Divergência de comprovante ou de prova local é conflito. Não avançar parcialmente, baixar o checkpoint nem promover sobreposição a aceitação nova. |

Na aplicação nova, a confirmação interna `PersistedLocal` só sai após commit bem-sucedido e cobre apenas a faixa gravada naquele destino. Replay exato confirmado pode devolver a mesma faixa, sem nova escrita ou renovação de TTL. Retenção usa o gatilho original A1. Expiração da prova ou da referência impede confirmação de replay. Em perda de resposta ou resultado de commit incerto, reler checkpoint, referência e provas no destino sob os mesmos guards. ACK na origem continua efêmero no contrato A2. A B2-A não envia ACK no fio.

## 4. Guards, prazo SQL e atomicidade

Usar `READ COMMITTED`. Obter primeiro o guard transacional M1C, depois o lock do namespace compartilhado com A1/A2. Só então amostrar um segundo e passar esse mesmo valor a `authorize_local_at`, `authorize_peer_at`, `classify_fence_in_tx` e operações A1. Exigir `Allowed` com o nível confiável da fixture. Conferir autoridade local/remota, políticas e estado do destino antes do efeito. O mesmo lock serializa mudança de autoridade, de ACL, de piso, descarte e efeitos locais correspondentes. Uma recusa não registra comprovante nem altera cópias, prefixo ou suspensão.

Um deadline monotônico por tentativa inclui pool, início de transação, locks, SQL, verificação, preparação do resultado e commit. Aplicar o saldo aos prazos locais de lock e instrução, preservando o limite preexistente mais restritivo. Nunca zerar um timeout para representar saldo esgotado. Sem espera de rede sob lock. Em cancelamento, rollback e sincronização da conexão precedem retorno ao pool. Conexão de estado incerto é retirada. Falha antes do commit não deixa efeito parcial. Commit incerto requer releitura e não autoriza confirmação presumida.

## 5. Aceites e sequência das fatias

1. **Fixture em três bancos.** Preparar A/B/C descartáveis, identidades e política M1C/M3/ACL sintéticas verificáveis. Em B recém-reservado, partir da linha de checkpoint em 0. Capturar em A, importar em B e confirmar bytes originais, prefixo e referência no mesmo commit. C é destino independente. Testar base positiva em C criada por cópia anterior regular, sem restore.
2. **Corte e limites.** Cobrir pacote que cabe exatamente, `has_more`, faixa incompleta, expurgo, prova inválida e publicação posterior ao corte. A captura não entrega pacote parcial.
3. **Replay e suspensão.** Exercitar replay exato e perda de resposta. Demonstrar que uma prova local com whitespace diferente e assinatura válida é recusada. Testar checkpoint intermediário, acima do corte, comprovante ausente/divergente e referência vencida. Em destino suspenso pela `0015`, recusar aplicação e replay sem alterar a posição bloqueada ou o prefixo.
4. **Transação e prazos.** Com barreiras, testar revogação, expiração e pausa de lock/SQL. Forçar falha antes do commit e verificar zero efeito parcial em provas, posição, prefixo e referência. Confirmar reutilização apenas de conexão saudável. Testar ambas as ordens entre revogação e efeito.
5. **Revisão.** Registrar resultados do harness e revisão independente após implementação autorizada. A B2-A não comprova wire nem restore.

**B2-B — transporte.** Especificar transferência de pacote B1 na sessão QUIC oficial, framing, limites, cancelamento e confirmação após commit. Consumir a B2-A apenas para receptores comprovadamente elegíveis. Não usar transporte para contornar o gate de restore.

**B2-C — restore planejado.** Preservar os requisitos do mapa `a676d03` e do parecer: seleção externa do modo fechado antes de bind, bloquear admissões antes da cópia, drenar ou reconciliar efeitos em voo e isolar o efetor anterior. Alta marca assinada fora do backup distingue cópia do corte de cópia anterior, identifica geração, checkpoint, autoridade, piso, revogações e `0015`. Antes de reabrir, confrontar provas/corte, reconstruir prefixo elegível até a alta marca, verificar controle corrente e preservar bloqueios. Se faltar evidência, informar `restore_history_incomplete` e bloquear efeitos dependentes, incluindo Pull A2. A B2-C precisa definir custódia e formato da evidência sintética. Perda não planejada, custódia de produção e remoção de `0015` exigem contratos separados.

**Rastreabilidade.** B1 → manifesto, digest e janela. A1/A2 → locks, provas, checkpoint e `0015`. M1C/M3 → autoridade e escudo. ADR-057/058/061 → sequência, ACK e commit. ADR-062/065/066 → política e retenção por cópia. ADR-072/075 → ordem e aprovação. SPEC-004 §5, `S004-059-F1` e `S004-061-B1` → gate B2-C. Nenhum teste foi executado nesta proposta.
