# SPEC M3-A v1.1 — codec e verificação pura de bundle assinado

**Estado:** proposta para revisão independente e aprovação. A implementação depende do aceite operacional da M1C e do gate de planejamento, revisão e aprovação por fatia da ADR-075. A M3 antecede a M4, conforme a ADR-072.

**Objetivo.** Especificar no crate `godmesh-federation` o primeiro perfil de configuração assinada, restrito a `federation.minimum_protocol_version`. O resultado é evidência criptográfica e uma avaliação pura para entradas explícitas do chamador. HTTP, SQL, publicação, agendamento e aplicação no runtime pertencem a fatias posteriores. Reusar o TLV de `SignedManifest`, SHA-256 e a verificação Ed25519 estrita do crate, sem dependências novas.

As autoridades permanecem distintas: `federation` pertence aos control-voters do cluster, `regional` à autoridade regional e `tenant` à autoridade daquele tenant. `local` pertence ao operador local e nunca é replicado. Este perfil só aceita `federation` de um cluster, com escopo vazio. Os demais contratos exigem seus próprios perfis e regras de exportação.

## 1. Entradas e formato

A política confiável do chamador contém cluster, epoch fixo, `max_frame_bytes` positivo e o conjunto estático de votantes. A origem proposta para política e pins é provisionamento autenticado fora de banda, com papel atribuído explicitamente. Identidade M1C, certificado apresentado pelo remetente e conteúdo do bundle não atribuem papel de votante. Validar o vínculo entre cada ID e sua chave Ed25519, recusando IDs ou chaves duplicados. Este recorte usa exatamente 3 ou exatamente 5 votantes, com assinatura de todos os votantes configurados. A política é verificável sem acesso a rede, banco, ambiente ou relógio internos.

O frame é um único objeto JSON UTF-8. Todos os campos abaixo são obrigatórios. Campos desconhecidos e duplicados são recusados em cada objeto do perfil 1.

| Campo | Regra |
|---|---|
| `profile` | Inteiro JSON igual a 1. |
| `cluster` | UTF-8 com 1 a 256 bytes, não branco, comparado exatamente com a política. |
| `namespace`, `scope_id` | Respectivamente `federation` e string vazia. |
| `epoch` | Inteiro de 0 a `i64::MAX`. Deve coincidir com o epoch pinado. |
| `version` | Inteiro de 1 a `i64::MAX`. Versão do snapshot, independente do piso de protocolo. |
| `applies_at`, `expires_at` | Segundos Unix UTC em toda a faixa `i64`, com `applies_at < expires_at`. |
| `required_signatures` | Inteiro 3 ou 5, igual ao tamanho da política. |
| `entries` | Array com exatamente uma entrada, definida abaixo. |
| `hash` | SHA-256 do corpo canônico, 64 caracteres hex minúsculos. |
| `signatures` | Entre zero e cinco pares `{signer_node_id,signature}` no formato estrutural. A autenticação exige o quórum completo. IDs são `gmn_` seguido de 64 caracteres hex minúsculos. Assinaturas têm 128 caracteres hex minúsculos. |

A entrada tem `key="federation.minimum_protocol_version"`, `value` inteiro de 1 a `i64::MAX`, `effective_after` em segundos Unix `i64` e `min_version` em string `major.minor.patch`. Cada componente dessa string vai de 0 a `i64::MAX`, em decimal ASCII, sem zeros à esquerda exceto `0`. Exigir `effective_after < expires_at`. É admissível que `effective_after < applies_at`. Inteiros JSON usam a gramática lexical `-?(0|[1-9][0-9]*)`. Sinal `+` inicial e zeros decimais à esquerda violam a sintaxe JSON. Fração e expoente são números JSON válidos, mas não são inteiros deste perfil. As faixas são restrições de interoperabilidade do perfil. Os valores operacionais são fornecidos por política.

## 2. Bytes canônicos e assinatura

O corpo começa pelos bytes ASCII de `godmesh.federation.config.bundle.v1`. Aplicar `push_field` a cada tag e valor nesta ordem:

```text
profile, cluster, namespace, scope_id, epoch, version,
applies_at, expires_at, required_signatures, entry_count,
key, value, effective_after, min_version
```

Cada TLV contém tamanho da tag em `u32` big-endian, tag ASCII, tamanho do valor em `u32` big-endian e valor. Usar UTF-8 exato para textos, `u64` big-endian para inteiros não temporais, inclusive contagens, e `i64` big-endian para tempos. `min_version` ocupa 24 bytes com os três componentes `u64` big-endian em ordem. `entry_count` vale 1. Hash e assinaturas ficam fora do corpo.

Calcular SHA-256 do corpo. Cada votante assina os bytes ASCII de `godmesh.federation.config.vote.v1`, seguidos de `push_field("hash", digest_bruto_de_32_bytes)`. Verificar com a chave pinada do votante identificado. Ordem dos campos JSON, escapes equivalentes e ordem dos votos não alteram hash ou validade. Versão, epoch, escopo, limiar e janela estão cobertos pela assinatura.

## 3. Ordem e resultados das funções puras

**Autenticar** recebe os bytes e a política e retorna `AuthenticatedBundle` com construção restrita ao verificador, ou erro. Aplicar esta precedência:

1. `InvalidPolicy` para política inválida. `FrameTooLarge` se o frame exceder o limite, antes do parse. O limite é inclusivo.
2. Ler um único objeto JSON completo. JSON inválido, dados após o objeto, `profile` ausente, duplicado, de tipo incorreto, com fração ou expoente, com sinal negativo (inclusive `-0`) ou acima de `u64::MAX` dão `MalformedFrame`. Nesta passagem, verificar a sintaxe do restante, sem interpretar tipos, faixas ou schema. Perfil inteiro sem sinal dentro de `u64` e diferente de 1 dá `UnsupportedProfile`, mesmo com campos ausentes ou valores fora das faixas do perfil 1.
3. Decodificar os bytes originais pelo schema estrito. Campos ausentes, desconhecidos ou duplicados, tipos divergentes, frações e expoentes em campos inteiros dão `MalformedFrame`. Concluir as verificações estruturais antes das semânticas. Preservar ou inspecionar os lexemas inteiros antes da conversão para tipos de máquina, sem coerção para ponto flutuante. Em ordem de campos do §1, usar `InvalidField` para faixa ou formato inválido, inclusive sinal negativo em campo não temporal (`-0` incluso), magnitude acima de `u64::MAX` e tempos além de qualquer extremo de `i64`. Nos tempos, `-0` equivale a `0`. Aplicar também os limites de quantidade, chave, namespace, escopo e janela nesta etapa. Cluster e epoch diferentes da política dão `ClusterMismatch` e `EpochMismatch`, nessa ordem. Toda esta etapa precede a comparação do hash e a verificação de assinaturas.
4. Recalcular e comparar hash. Divergência dá `HashMismatch`. Comparar limiar declarado com política, retornando `ThresholdMismatch`. Detectar IDs de voto repetidos antes de verificar assinaturas, com `DuplicateVote`. Em ordem lexicográfica de ID, verificar se o votante é pinado e se a assinatura é válida, retornando `UnknownVoter` ou `InvalidSignature`. Contagem insuficiente dá `InsufficientQuorum`. A verificação não retorna sucesso parcial.

**Comparar com histórico** recebe bundle autenticado e, opcionalmente, o registro confiável de maior versão aceita, identificado pela chave completa `(cluster, namespace, scope_id, epoch)`, com versão e hash associados. Comparar todos os componentes da chave com os do bundle antes de comparar versão ou hash. Qualquer divergência retorna `HistoryKeyMismatch`, inclusive se apenas o cluster diferir. Versão menor resulta em `Rollback`. Mesma versão e hash resulta em `Replay`. Mesma versão e hash diferente resulta em `Conflict`. Ausência de registro ou versão maior resulta em `NewVersion`. Só classificar conflito após autenticar o artefato. A comparação independe do relógio, inclusive para detectar divergência autenticada depois de a janela expirar. Esta função não muda o registro nem autoriza troca de epoch.

**Avaliar tempo e compatibilidade** recebe bundle autenticado, segundo UTC explícito e versão local de software em tupla validada. Primeiro, `Expired` se `now >= expires_at`. Depois, `IncompatibleSoftware` se a versão local for menor que `min_version`. Caso contrário, `Pending` antes de `max(applies_at,effective_after)` e `Eligible` a partir desse instante. Sem tolerância implícita ou aritmética que desloque timestamps. Replay nunca renova a janela. Um conflito autenticado é observável mesmo que a avaliação temporal falhe.

**Piso efetivo.** `value` não é o piso do histórico de versões. A M3-A autentica um valor que pode exceder o maior ALPN suportado pelo nó. A incapacidade de negociar esse piso é independente de `min_version`. Na fatia operacional, versão nova e compatível pode ser agendada. O piso anterior governa até a ativação confirmada. A partir do instante em que o novo piso se torna exigível, operações dependentes ficam bloqueadas se a ativação ainda não foi confirmada. Após o commit, se não houver ALPN suportado que satisfaça o piso, o escopo fica `degraded` por incompatibilidade de protocolo e recusa operações de federação afetadas, inclusive sessões que ficaram abaixo do piso. Hoje `godmesh/1` representa nível 1. O número não fabrica outro ALPN. M4 depende desse bloqueio efetivo e de estados operacionais verificáveis, além do codec.

## 4. Plano e aceite P/N/B/F

1. **Codec e vetor independente.** Usar três sementes Ed25519 de 32 bytes, cada uma preenchida respectivamente com `01`, `02` e `03` em hex. Derivar IDs conforme `NodeIdentity`. Fixar `cluster=cluster-teste`, `epoch=7`, `version=9`, janela `[100,200)`, `effective_after=120`, `value=2`, `min_version=1.2.3`, limiar 3 e escopo vazio. O corpo de referência tem 428 bytes e SHA-256 `5b6cc1bf19a820a66a1b66645bad74f85c4dee84fea15875cb56e3a5b9a59436`, calculado com um codificador TLV separado em Python. No aceite, congelar também o corpo, a preimagem dos votos e as assinaturas produzidas independentemente para comparação literal. Esses valores são de fixture.
2. **Verificação e classificação.** Cobrir os casos abaixo pelo caminho público e registrar resultados após implementação autorizada.
3. **Evidência e revisão.** Registrar vetor, resultados do crate, suíte do workspace e revisão independente. O aceite em três servidores da ADR-075 permanece na integração operacional.

| Classe | Aceite planejado |
|---|---|
| **P** | Validar vetor de três votos e outro de cinco. Permutar campos e votos sem mudar o resultado. Em software `1.2.3`, reconhecer `Pending` em 119 e `Eligible` em 120, mesmo com suporte somente a `godmesh/1`. |
| **N** | Recusar outro cluster ou epoch, namespace `local`, chaves desconhecidas, duplicadas ou insuficientes, limiar adulterado, assinatura ou hash incorretos. Classificar rollback, replay e conflito autenticado. Perfil futuro bem-formado prevalece sobre schema incompatível. Erro de assinatura prevalece sobre expiração. Com o vetor de referência, usar histórico de `outro-cluster` e os mesmos namespace, escopo, epoch, versão e hash para obter `HistoryKeyMismatch`. Com `version=18446744073709551616` e hash composto por 64 zeros, esperar `InvalidField("version")`. Corrigir apenas a versão para 9 deve resultar em `HashMismatch`. |
| **B** | Frame de tamanho igual ao limite e um byte acima. Testar mínimo e máximo numérico e fora da faixa, `now=119,120,199,200`, janela vazia, datas extremas `i64` e um segundo fora de cada extremo, `-0` em tempo e em campo não temporal, `profile` negativo e acima de `u64::MAX`, fração e expoente, e tripla de software imediatamente abaixo do mínimo. |
| **F** | Frame truncado, UTF-8 inválido, duplicatas aninhadas e JSON adicional. Em seguida, validar o vetor íntegro na mesma instância e obter o mesmo resultado, sem estado residual. |

## 5. Nota operacional para o pull

O HTTP atual usa listener TCP sem mTLS. O QUIC existente negocia o ALPN próprio e multiplexa o protocolo GodMesh. A rota `GET /v1/federation/config` deve ter autenticação de nó no ponto que toma a decisão de acesso. Proposta para a fatia seguinte: listener HTTPS dedicado com mTLS usando Rustls já adotado, identidade derivada do certificado validado e entregue ao handler em contexto de conexão tipado. Validar autoridade M1C e escopo antes de responder, inclusive antes de `304`. Ignorar cabeçalhos que aleguem identidade. Se houver terminação externa, escolher e aprovar previamente um canal interno autenticado, isolado de acesso direto, com remoção de cabeçalhos de identidade de origem externa e atestação verificável pelo backend. O listener HTTP atual não serve de bypass. A rota de recuperação deve continuar acessível a um nó autenticado que precise corrigir um piso incompatível, sujeita à autoridade e ao escopo, para evitar dependência circular do ALPN.

**Decisões operacionais pendentes.** Aprovar terminação HTTPS e origem confiável da identidade, provisionamento e rotação de pins/epochs, limites de ambiente, compatibilidade no rollout e política de acesso e residência para futuros perfis regionais e de tenant. Mudança de epoch, conjunto de votantes ou regra de limiar exige contrato próprio.

**Rastreabilidade.** ADR-072 e rede-alvo §13 → §§1–3 → vetor e casos P/N/B/F. ADR-075 → gate e aceite integrado. M1C → identidade do nó consumida no pull. M4 → aplicação operacional do piso no efetor. Este documento descreve verificações planejadas.
