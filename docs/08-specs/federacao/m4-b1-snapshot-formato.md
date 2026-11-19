# SPEC proposta M4-B1 v1.0 — formato e verificação pura do snapshot de metadados

**Estado:** para revisão independente e aprovação por fatia. Base: mapa M4-B em `480f5f8`, parecer independente e M4-A1. Implementação condicionada ao aceite operacional A2 e ao gate ADR-075, na ordem da ADR-072. O perfil cobre bootstrap e restore sintéticos, com uma origem, um destino explícito por pacote e um corte fixo de provas originais. A B1 acrescenta apenas codec e verificador puro a `godmesh-federation`, com dependências existentes.

## 1. Formato e escolha da representação

Um array JSON como na A2 exigiria extrair fatias entre delimitadores sem perder whitespace externo. Um digest de objetos reserializados perderia diferenças lexicais. Para este perfil, usar frames binários com comprimento explícito e manter JSON estrito em cada frame. O wire QUIC e sua identificação ficam na B2.

O pacote é a concatenação de:

1. `manifest_len:u32 BE` e exatamente esses bytes de JSON UTF-8 do manifesto.
2. Para cada um dos `event_count` eventos, `event_len:u32 BE` e exatamente esses bytes originais de um objeto JSON `SignedEvent`, inclusive whitespace externo permitido pelo JSON.
3. Fim do pacote. Comprimentos zero, truncamento e bytes sobrando são inválidos.

O manifesto é um objeto JSON estrito, com todos os campos obrigatórios, sem desconhecidos ou duplicatas.

| Campos | Regra do perfil 1 |
|---|---|
| `profile` | Inteiro sem sinal, igual a 1. |
| `cluster`, `tenant_id`, `stream_id` | UTF-8 exato, não branco, até `MAX_FIELD_LEN` bytes. Stream com prefixo `m4a.metadata.` e sufixo não branco. |
| `origin_node`, `destination_node` | IDs distintos, cada um `gmn_` seguido de 64 hex minúsculos. |
| `partition`, `authority_epoch` | Partição `u32` igual a 0. Epoch em `1..=i64::MAX`. |
| `base_seq`, `cut_seq`, `event_count` | Base em `0..=i64::MAX`. Corte em `1..=i64::MAX`. Quantidade em `1..=u32::MAX`, também limitada pela política. Exigir `cut_seq > base_seq` e `event_count == cut_seq - base_seq`, com aritmética verificada. Faixa vazia é inválida. |
| `not_before`, `expires_at` | Segundos Unix UTC em toda a faixa `i64`, com início estritamente anterior ao fim. Janela assinada semiaberta `[not_before, expires_at)`. |
| `events_digest`, `signature` | SHA-256 em 64 hex minúsculos e assinatura Ed25519 em 128 hex minúsculos. |

Inteiros JSON exigem lexema decimal inteiro, sem fração ou expoente. Campos sem sinal não aceitam sinal negativo, inclusive `-0`. Para tempos, `-0` equivale a 0. Preservar lexemas até verificar faixa. Não converter por ponto flutuante.

## 2. Bytes canônicos e prova

Reusar `push_field`: tamanho da tag `u32 BE`, tag ASCII, tamanho do valor `u32 BE`, bytes do valor. O corpo assinado começa pelos bytes ASCII de `godmesh.federation.snapshot.v1`, seguidos dos campos nesta ordem:

```text
profile, cluster, origin_node, destination_node, tenant_id, stream_id,
partition, authority_epoch, base_seq, cut_seq, event_count,
not_before, expires_at, events_digest
```

Textos entram em UTF-8 exato. `profile`, epoch, base e corte usam `u64 BE`. Partição e contagem usam `u32 BE`. Tempos usam `i64 BE`. O digest entra como 32 bytes brutos, decodificados do hex. Assinatura não integra o corpo.

`events_digest` é SHA-256 do domínio ASCII `godmesh.federation.snapshot.events.v1`, seguido de `event_count:u32 BE` e dos pares `event_len:u32 BE || bytes_originais` na ordem do pacote. Sem trim, reserialização ou ordenação. Ordem das propriedades do manifesto não altera os bytes assinados. Variações lexicais dos eventos alteram o digest mesmo quando suas assinaturas individuais continuam válidas.

## 3. Política, precedência e resultado

A API pura recebe pacote e política explícita do chamador. A política contém cluster, origem e destino esperados com IDs e chaves Ed25519, tenant, stream, partição, epoch, classe de retenção esperada e quatro limites positivos: `max_package_bytes`, `max_manifest_bytes`, `max_event_bytes` e `max_events`. Limites de manifesto, item e contagem cabem em `u32`. O pacote conta todos os prefixos e bytes. A identidade esperada é independente do pacote. O vínculo de IDs a chaves é validado para ambos. O destinatário não precisa assinar o pacote.

Aplicar a seguinte ordem, devolvendo a primeira falha:

1. **Política e teto externo.** `InvalidPolicy` para política inválida. `PackageTooLarge` se exceder teto, antes de ler frames. Comprimentos e offsets usam aritmética verificada.
2. **Manifesto e discriminação.** Ler seu prefixo e teto antes de decodificar. Tamanho excedido dá `ManifestTooLarge`. Comprimento zero, truncamento, UTF-8 ou JSON inválido e `profile` ausente, duplicado, fracionário, exponencial, com sinal ou acima de `u64::MAX` dão `MalformedManifest`. Perfil inteiro válido diferente de 1 dá `UnsupportedProfile` antes de examinar o schema 1. Para perfil 1, validar estrutura e tipos estritos, depois faixas e formatos na ordem do §2, com `InvalidField`. Faixa vazia, invertida ou divergente da quantidade dá `InvalidCut`; janela vazia ou invertida dá `InvalidWindow`. Quantidade acima do teto dá `TooManyEvents`. Comparar a tupla e as identidades esperadas, com `ScopeMismatch` se divergirem.
3. **Framing de eventos.** Ler exatamente a quantidade declarada. `EventTooLarge` se o comprimento declarado exceder o teto antes de extrair o frame. Zero, prefixo ou frame truncado, overflow de offset e bytes que sobrem após o último evento dão `MalformedPackage`. Nesta etapa, os frames são bytes, e o JSON de evento ainda não é interpretado.
4. **Autenticação do pacote.** Verificar a assinatura estrita do manifesto com a chave esperada de A, depois comparar o digest ordenado com os bytes recebidos. Retornar `ManifestSignatureInvalid` ou `EventsDigestMismatch`. A verificação de eventos individuais só começa após as duas checagens.
5. **Provas A1.** Para cada frame em ordem, decodificar UTF-8 e um objeto `SignedEvent` estrito. Conferir origem e signatário A, tenant, stream, partição, epoch e classe esperados. Depois exigir sequência exata de `base_seq+1` até `cut_seq`, com aritmética verificada, e verificar a assinatura existente de A. Retornar `InvalidEvent`, `EventScopeMismatch`, `SequenceMismatch` ou `EventSignatureInvalid`, com índice a partir de zero. Campos originais, inclusive `issued_at`, são preservados.

O resultado autenticado retorna manifesto, corte, janela e fatias dos envelopes originais. Falha em qualquer etapa não retorna pacote parcial. O verificador não consulta relógio, banco ou rede. A janela é autenticada e devolvida, sem declarar elegibilidade corrente. B2 revalida a janela em `not_before <= now < expires_at` no efeito, além da retenção por cópia e do prazo da tentativa, que são independentes. Replay de snapshot não reaplica a janela online do ingest ao `issued_at`.

## 4. Vetores independentes e aceites planejados

1. **Vetor positivo.** Usar chaves de teste Ed25519 derivadas de sementes de 32 bytes preenchidas com `01` e `02`, para A e B. Fixar cluster `cluster-teste`, tenant `tenant-teste`, stream `m4a.metadata.vetor`, partição 0, epoch 7, base 0, corte 2, contagem 2 e janela `[100,200)`. Usar classe `m4a_metadados_posicao` e `issued_at=90` em ambos os eventos, com sequências 1 e 2. Serializar a primeira prova com whitespace externo e a segunda com um escape Unicode equivalente em campo textual. Congelar pelo codificador independente o TLV, preimagem do digest, digest, assinatura do manifesto, assinaturas dos eventos, pacote e fatias esperadas. Os limites da fixture são escolhidos a partir dos tamanhos obtidos, sem pressupor valores de produção.
2. **Negativos e precedência.** Trocar apenas whitespace ou escape e esperar `EventsDigestMismatch`. Reassinar um manifesto com digest correto para testar sequência fora de ordem, tupla ou assinatura individual divergente. Cobrir destino errado, contagem incompatível, faixa vazia e janela invertida. Testar perfil futuro com schema estranho, assinatura inválida mais digest divergente e frame truncado mais assinatura inválida.
3. **Borda e recuperação.** Cobrir limites exatos e um byte/item acima, corte no máximo persistível, overflow e truncamentos em cada fronteira. Após cada erro, o mesmo verificador aceita o pacote íntegro. Testar que manifesto autenticável com janela futura ou histórica é devolvido com a janela intacta. As fronteiras temporais de elegibilidade pertencem aos testes B2.

## 5. Fronteira B2 e decisões

B2 define evidência confiável **externa ao backup restaurado** para reconciliar autoridade, piso, revogações, checkpoint, operações em voo e suspensão `0015`. Sem essa evidência, expor `restore_history_incomplete` e bloquear fluxos dependentes, inclusive Pull A2. Backup anterior à suspensão não libera o receptor mesmo com tabela local vazia. O caso positivo precisa demonstrar reconciliação, conforme SPEC-004 §5 e `S004-059-F1`. Fonte, custodiante e formato dessa evidência permanecem pendentes na B2.

B2 distingue aplicação nova com checkpoint exatamente na base, de replay exato com evidência durável do pacote e provas locais ainda elegíveis. Deve cobrir checkpoint no corte, acima dele ou entre base e corte, e commit incerto. O perfil não remove `0015` nem reintroduz prova expurgada. Não há decisão de produto adicional para o codec puro. Abrir namespace novo com descontinuidade após perda definitiva continua sujeito a decisão explícita de produto.

**Rastreabilidade.** Mapa M4-B em `480f5f8` e parecer independente → formato/restore. ADR-057/058 → posição e fronteira de confirmação. ADR-061/062/065/066 → transação, retenção e residência na B2. ADR-072/075/076 → sequência, aprovação e autoridade. A1/S-sec-2 → bytes e verificação das provas originais. Vetores e testes desta SPEC permanecem planejados.
