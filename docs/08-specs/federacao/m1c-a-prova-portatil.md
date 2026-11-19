# SPEC M1C-A v1.1 — prova portátil de ingresso e verificação da cadeia

**Base normativa e técnica:** `e2a6262`, ADR-071 e sua emenda, ADR-072 e sua emenda, ADR-075, ADR-076, SPEC M1B/I v1.3 e `docs/08-specs/federacao/m1c-cadeia-confianca.md`.

**Estado:** proposta para revisão independente e aprovação da fatia conforme a ADR-075. A direção de usar v2 e revogação assinada está autorizada. Os detalhes de formato e verificação abaixo integram a proposta submetida à revisão.

## 1. Objetivo e limites

Adicionar ao crate `godmesh-federation` um certificado portátil v2 e um verificador determinístico de cadeia linear. O verificador recebe a identidade esperada, o cluster, o instante e a política do chamador. Um distribuidor pode transportar o artefato sem participar da decisão de confiança criptográfica.

Reutilizar `SignedManifest`, o codec `push_field`, `NodeIdentity`, a verificação Ed25519 estrita de `identity` e SHA-256 já disponível no crate. Preservar os tipos, os bytes canônicos, os domínios, os erros e o comportamento de aprovação e rejeição v1. A v2 é um tipo separado. Uma decisão v1 não pode ser convertida em certificado portátil.

O resultado desta fatia é uma **cadeia criptograficamente verificada**. Emissão operacional, entrega autenticada, comprovação de posse na sessão, atualização de pares e consumo de revogação pertencem às fatias seguintes. O aceite A genesis → B → C em três servidores, sem semear pares por SQL, continua pendente após M1C-A. Bundles de confiança da M3 e autorização do Inbox e replicação da M4 seguem seus próprios contratos.

## 2. Objeto e bytes assinados

Um `JoinCertificateV2` contém os campos da tabela, exceto o manifesto. Cada elo transporta o certificado e o `SignedManifest` completo do candidato no objeto `{"certificate":{...},"manifest":{...}}`. Uma `JoinChainV2` tem a forma JSON `{"version":2,"links":[...]}`, com a raiz configurada fora do envelope, o primeiro certificado emitido pela raiz e o último emitido para o candidato esperado.

| Campo do elo | Contrato |
|---|---|
| `join_request_id` | Identificador do pedido no formato `fjr_` seguido de 32 dígitos hexadecimais minúsculos, conforme a M1B. |
| `cluster` | UTF-8 não branco, até `MAX_FIELD_LEN` bytes. Comparação exata com a política e com o manifesto. |
| `candidate_node_id`, `candidate_public_key` | Identidade e chave Ed25519 do candidato. Devem coincidir com as do manifesto validado. |
| `manifest_digest` | SHA-256 do manifesto assinado, conforme a fórmula abaixo. |
| `issuer` | União discriminada por `kind`. `{"kind":"genesis","node_id":"…"}` identifica uma raiz configurada. `{"kind":"parent","digest":"…"}` identifica o certificado imediatamente anterior. |
| `can_approve` | Booleano obrigatório. Delegação explícita de autoridade de ingresso no cluster. `false` permite que o certificado seja a folha, mas impede seu uso como pai. Esta versão tem uma única capacidade delegável e não expressa escopos de tenant, stream ou ingest. |
| `not_before`, `expires_at` | Segundos Unix UTC, `i64`, com `not_before < expires_at`. |
| `epoch` | Geração da decisão do aprovador, inteiro de `0` a `i64::MAX`, carregado no artefato para uso posterior pelo piso durável da decisão. A verificação pura não compara epochs de aprovadores distintos nem consulta histórico. |
| `signature` | Assinatura Ed25519 do aprovador. |
| `manifest` | `SignedManifest` existente. O certificado assina seu digest. |

Chaves e digests usam 64 caracteres hexadecimais minúsculos, assinaturas usam 128, e identificadores seguem `gmn_` mais 64 caracteres hexadecimais minúsculos. Campos textuais são assinados exatamente como recebidos, em UTF-8. Para v2, o envelope, os elos, o objeto `certificate` e os objetos `issuer` recusam campos desconhecidos, duplicados ou obrigatórios ausentes. O manifesto mantém as regras existentes de `SignedManifest`.

**Codificação canônica.** Começar pelos bytes ASCII de `godmesh.federation.join.approval.v2`. Acrescentar, nesta ordem, os campos com `push_field`:

```text
join_request_id, cluster, candidate_node_id, candidate_public_key,
manifest_digest, issuer_kind, issuer_ref, can_approve,
not_before, expires_at, epoch
```

`push_field` grava comprimento da tag em `u32` big-endian, tag, comprimento do valor em `u32` big-endian e valor. IDs, chaves e digests entram como texto canônico. `issuer_kind` é `genesis` ou `parent`. `issuer_ref` é o `node_id` ou o digest da variante correspondente. `can_approve` é um byte `00` ou `01`. Tempos usam `i64` big-endian e epoch usa `u64` big-endian. A assinatura e o objeto `manifest` ficam fora desses bytes. O domínio identifica ação e versão. A versão do envelope deve ser `2` antes de qualquer verificação criptográfica. A assinatura usa a identidade explícita fornecida ao `sign` do tipo v2 e preserva os demais campos. A verificação da cadeia é a etapa que estabelece se o signatário corresponde ao emissor autorizado.

Para ambos os digests, retornar SHA-256 codificado em hex minúsculo. O conteúdo de cada hash começa pelos bytes ASCII do domínio indicado, seguidos de `push_field("body", bytes_canônicos)` e `push_field("signature", assinatura_decodificada_em_64_bytes)`:

| Digest | Domínio | Corpo |
|---|---|---|
| Manifesto | `godmesh.federation.join.manifest-digest.v2` | `SignedManifest::canonical_bytes()` |
| Elo | `godmesh.federation.join.link-digest.v2` | Bytes canônicos da aprovação v2 |

O digest independe de representação JSON e segue a normalização de conjuntos do manifesto. O vínculo com o pai cobre o certificado exato, inclusive sua assinatura.

## 3. API e algoritmo de verificação

Expor no crate uma API equivalente a:

```rust
verify_join_chain(
    frame: &[u8],
    expected: &ExpectedJoinIdentity,
    now: i64,
    policy: &JoinChainPolicy,
) -> Result<VerifiedJoinChain, JoinChainError>
```

`ExpectedJoinIdentity` contém `node_id` e `public_key` obtidos pelo chamador independentemente da cadeia. `JoinChainPolicy` contém o cluster esperado, raízes genesis daquele cluster, `max_depth: NonZeroUsize` e `max_frame_bytes: NonZeroUsize`. Não há política implícita, leitura de ambiente ou relógio interno. A política deve conter ao menos uma raiz, com vínculo válido entre ID e chave e IDs únicos. O chamador fornece limites explícitos. A profundidade é o número de certificados, excluída a raiz.

1. Validar política e depois identidade esperada. Recusar frame acima de `max_frame_bytes` antes de desserializar. Na primeira leitura, exigir um único objeto JSON completo com `version` presente uma vez, como inteiro compatível com `u64`. Ignorar o esquema das demais propriedades nessa leitura. JSON inválido, valor raiz que não seja objeto, versão ausente, duplicada ou de tipo inválido dão `MalformedFrame`. Uma versão inteira diferente de `2` dá `UnsupportedVersion` mesmo que `links` seja ausente ou tenha outra estrutura.
2. Para versão 2, decodificar a estrutura completa e estrita. Campos e formatos JSON divergentes dão `MalformedFrame`. Antes da criptografia, `links` vazio dá `EmptyChain` e tamanho acima de `max_depth` dá `DepthExceeded`.
3. Percorrer da raiz à folha e concluir todas as verificações do elo atual antes de olhar o seguinte. Em cada elo, conferir nesta ordem:
   1. Campos assinados e seu formato, cluster, depois a condição estrutural `not_before < expires_at`. Uma janela invertida ou vazia é `InvalidWindow`.
   2. Emissor e vínculo com o pai. O primeiro elo referencia raiz da política. Elo posterior referencia por digest o elo imediatamente anterior, já validado, cujo `can_approve` deve ser `true`. A chave vem da raiz ou da chave candidata do pai. Depois, verificar se a identidade candidata se repete na raiz emissora ou nos elos anteriores. Elo ausente ou fora de ordem é recusado.
   3. Validade estrutural, tamanho e assinatura do manifesto com `SignedManifest::to_frame()` e `SignedManifest::verify()`. Conferir o vínculo exato entre identidade, chave e cluster do certificado e do manifesto, depois o digest. O manifesto é um documento histórico vinculado à decisão. Seu frescor para ingresso cabe à emissão operacional.
   4. Assinatura Ed25519 do elo com `identity::verify_signature`. Em seguida, aplicar a janela ao instante observado, com `NotYetValid` se `now < not_before` e `Expired` se `now >= expires_at`. Verificar por último a contenção da janela do filho na do pai, admitindo inícios ou fins iguais. A validade efetiva é a interseção calculada pelo maior início e menor fim, sem aritmética sobre timestamps.
4. Conferir a identidade e a chave da folha com as esperadas. Retornar somente após a cadeia inteira passar.

`VerifiedJoinChain` tem construção restrita ao verificador e expõe cluster, raiz, identidade e digest da folha, profundidade, janela efetiva e `can_approve` da folha. Esse resultado vale para o instante e a política usados. O crate não consulta banco ou rede e não produz estados `authorized` ou `ready`.

## 4. Estados, erros e revogação

A entrada começa **não verificada**. Decodificar cria apenas dados para validação. O resultado terminal é **verificada para os parâmetros fornecidos** ou **recusada**, sem sucesso parcial. Repetir a chamada com as mesmas entradas produz o mesmo resultado. Erros são variantes tipadas, sem status HTTP nesta fatia.

| Origem | Variantes mínimas |
|---|---|
| Entradas locais | `InvalidPolicy`, `InvalidExpectedIdentity` |
| Envelope | `FrameTooLarge`, `MalformedFrame`, `UnsupportedVersion`, `EmptyChain`, `DepthExceeded` |
| Estrutura do elo | `InvalidField`, `ClusterMismatch`, `InvalidWindow` |
| Autoridade e topologia | `UnknownRoot`, `InvalidIssuerPosition`, `ParentDigestMismatch`, `DelegationDenied`, `RepeatedIdentity` |
| Manifesto e prova | `InvalidManifest`, `ManifestBindingMismatch`, `ManifestDigestMismatch`, `InvalidSignature` |
| Tempo e folha | `NotYetValid`, `Expired`, `ChildWindowOutsideParent`, `CandidateMismatch` |

Erros de elo incluem índice a partir de zero. Nos testes de contrato, isolar uma falha por caso. Para múltiplas falhas, reportar a primeira na ordem das etapas acima e dos elos.

Nenhum objeto de revogação é necessário para verificar as assinaturas e os vínculos desta cadeia. O digest identifica o elo exato para a fatia seguinte. A ADR-076 registra a autoridade de revogação, os efeitos sobre descendentes e o uso de `single_approver`. A persistência, a aplicação dessas decisões, o frescor da revogação e o piso durável de epoch exigem contrato e verificação no consumo operacional. A ADR-072 rege o quórum dos bundles de confiança.

## 5. Vetores e aceite P/N/B/F

**Vetor determinístico de referência.** Derivar chaves Ed25519 das sementes de teste de 32 bytes repetidos `01`, `02` e `03` para A, B e C. Configurar somente A como raiz de `cluster-teste`. B e C assinam manifestos com `region="regiao-teste"`, `software_version="1.0.0"`, `alpn=""`, listas vazias, `epoch=1` e `issued_at=90`. A assina `fjr_` seguido de 32 letras `b` para B, com janela `[100,200)`, `epoch=7` e `can_approve=true`. B assina `fjr_` seguido de 32 letras `c` para C, referenciando o digest de A → B, com janela `[120,180)`, `epoch=1` e `can_approve=false`. Verificar para C em `now=150`, com `max_depth=2` e `max_frame_bytes=32768`. Esperar raiz A, folha C, profundidade 2, janela `[120,180)` e delegação falsa. Esses valores são dados de teste.

Congelar no teste os bytes canônicos dos dois elos, os digests dos manifestos e elos e as assinaturas. Produzir os valores de referência com um codificador independente da função de produção, conforme o §2. Comparar os valores literalmente e verificar a cadeia pelo caminho público.

| Classe | Casos de aceite |
|---|---|
| **P — positivo** | A → B e A → B → C a partir da raiz configurada. Confirmar que um segundo verificador com a mesma política e entradas devolve o mesmo resultado. Reordenar propriedades JSON e listas do manifesto sem alterar o resultado canônico. |
| **N — negativo** | Raiz desconhecida, assinatura inválida, domínio v1, chave ou ID divergente, manifesto inválido ou diferente, cluster divergente, folha diferente da esperada, pai ausente ou fora de ordem, digest incorreto, identidade repetida e pai sem delegação. Exercitar também janela do filho fora da do pai, mesmo quando ambas contêm `now`. |
| **B — borda** | Profundidade exatamente no limite aceita e limite inferior ao comprimento recusa. Frame exatamente no limite segue para decodificação e um byte acima recusa. Cadeia vazia recusa. Aceitar em `now=120` e `179`, recusar em `119` e `180`. Cobrir janelas iguais e timestamps nos extremos de `i64` sem overflow. |
| **F — falha e recuperação** | Frame truncado, JSON inválido, versão desconhecida, campo duplicado e política inválida retornam erro sem pânico. Após qualquer recusa, repetir o vetor íntegro com entradas válidas e obter o resultado de referência, sem estado residual. |

Testes de discriminação devem verificar `UnsupportedVersion` para versão futura com corpo incompatível e `MalformedFrame` para versão duplicada ou certificado com campo desconhecido, duplicado ou obrigatório ausente. No vetor A → B → C com `now=180`, uma assinatura de C com forma válida e verificação inválida deve resultar em `InvalidSignature` no índice 1 antes de `Expired`. Com falhas em elos distintos, a primeira falha no elo de menor índice prevalece. Para testar a normalização de listas, usar manifesto auxiliar com pelo menos dois itens distintos e inverter sua ordem.

**Verificação planejada.** Trabalhar em três subtarefas após o gate da ADR-075: fixar formato e vetores, implementar o verificador pelo ciclo TDD e executar os testes do crate, a suíte do workspace e o lint aplicável. Incluir regressão explícita do domínio, da assinatura e da rejeição v1. Submeter o diff e as evidências à revisão independente.

**Rastreabilidade.** RF-015 → ADR-071 → prova vinculada ao candidato e cadeia ancorada → casos P/N. RNF-002, cobertura parcial → ADR-071 → identidade Ed25519, vínculo com o manifesto e janelas verificáveis → casos P/N/B/F. ADR-072 delimita as dependências operacionais. ADR-075 rege a aprovação da fatia e preserva o aceite futuro em três servidores.
