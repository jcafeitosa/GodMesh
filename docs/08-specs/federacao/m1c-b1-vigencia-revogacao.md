# SPEC M1C-B1 v1.1 — formatos assinados de vigência e revogação

**Estado:** proposta para revisão independente. Deriva da [ADR-076](../../06-planejamento/decisoes.md#adr-076--prova-portátil-e-revogação-de-ingresso) e da [M1C-A](m1c-a-prova-portatil.md). O recorte é criptográfico e puro. Emissão autorizada, ordenação durável, importação, efeitos operacionais e aceite com três servidores pertencem à M1C-B.

## Contratos

| Tipo | Domínio ASCII | Campos assinados na ordem indicada |
|---|---|---|
| `JoinValidityV1` | `godmesh.federation.join.status.v1` | `cluster`, `root_node_id`, `leaf_digest`, `issued_at`, `expires_at` |
| `JoinRevocationV1` | `godmesh.federation.join.revocation.v1` | `cluster`, `root_node_id`, `target_digest`, `revoker_node_id`, `epoch` |
| `JoinRevocationReceiptV1` | `godmesh.federation.join.revocation-receipt.v1` | `cluster`, `root_node_id`, `revocation_digest`, `accepted_at` |

Cada objeto JSON tem os campos da sua linha e `signature`. Campos obrigatórios, duplicados ou desconhecidos seguem a decodificação estrita da M1C-A. Arrays posicionais são recusados. `cluster` é UTF-8 não branco, com até `MAX_FIELD_LEN` bytes. IDs usam `gmn_` e 64 dígitos hexadecimais minúsculos. Digests são 64 dígitos hexadecimais minúsculos. Assinaturas são 128 dígitos hexadecimais minúsculos. Textos são assinados exatamente como recebidos.

Antepor o domínio ASCII aos campos, codificando cada um com o `push_field` da M1C-A. Tempos são `i64` big-endian em segundos Unix UTC. `epoch` é `u64` big-endian, limitado a `i64::MAX`. A assinatura fica fora dos bytes canônicos. O digest da revogação é SHA-256 em hex minúsculo do domínio ASCII `godmesh.federation.join.revocation-digest.v1`, `push_field("body", bytes_canônicos)` e `push_field("signature", bytes_decodificados)`.

## API e comportamento

1. Expor `canonical_bytes()`, assinatura com `NodeIdentity` e verificação com chave explícita nos três tipos. Ao assinar vigência ou recibo, preencher `root_node_id` com a identidade usada. Ao assinar revogação, preencher `revoker_node_id`. A assinatura preserva os demais campos. A verificação confere formato, vínculo entre a chave fornecida e o ID do signatário e assinatura Ed25519 estrita, nessa ordem. A escolha da chave autorizada é responsabilidade do consumidor.
2. Expor cálculo do digest da revogação. A verificação composta recebe a chave do revogador e uma `VerifiedJoinChain` obtida independentemente para o alvo. Depois de verificar a assinatura, exige que cluster, raiz e `target_digest` coincidam com cluster, raiz e digest da folha da cadeia. Apenas essa operação constrói a evidência `VerifiedJoinRevocation`, sem construtor público nem desserialização. A verificação composta do recibo recebe a chave confiável da raiz e a evidência `VerifiedJoinRevocation`. Depois de verificar a assinatura do recibo, exige que cluster, raiz e digest coincidam com a revogação verificada. O digest isolado não produz a evidência.
3. A vigência exige intervalo estrutural `issued_at < expires_at`. Depois da verificação de assinatura, conferir a correspondência exata de cluster, raiz e digest com a cadeia verificada da M1C-A. Exigir que o início e o fim estejam na janela efetiva da cadeia e que a duração seja no máximo `max_validity_seconds` positivo fornecido pelo chamador. Comparar primeiro o vínculo, depois contenção e duração, depois `issued_at <= now < expires_at`. Usar diferença inteira sem overflow. Usar somente o instante e a política fornecidos.

Erros tipados distinguem campo inválido, vínculo de chave divergente, assinatura inválida, janela invertida, artefato divergente, janela fora da cadeia, duração excedida, início futuro e expiração. A fatia preserva domínios e comportamento v1/M1C-A, trabalha com as dependências já resolvidas e não altera o banco ou as rotas.

## Aceite

- **Positivo.** Codificador independente gera vigência da raiz A para C, revogação da prova A → B e recibo de A. Comparar bytes, assinaturas e digest e validar os vínculos pela API pública.
- **Negativo.** Recusar chave divergente, assinatura de outro domínio, digest alterado, dados divergentes de recibo ou cadeia, epoch acima de `i64::MAX` e entrada JSON com campo desconhecido, duplicado, ausente ou objeto posicional. Um recibo de assinatura válida vinculado aos bytes de uma revogação inválida não permite completar a verificação e a revogação retorna erro de assinatura. Uma revogação válida para outro elo retorna erro de artefato divergente.
- **Borda.** Aceitar o início e o último segundo da vigência. Recusar o fim, a janela invertida e duração acima do limite. Cobrir `i64::MIN` e `i64::MAX` sem overflow.
- **Recuperação.** Após uma recusa, verificar o artefato íntegro com as mesmas entradas válidas. Manter o vetor M1C-A e os testes de aprovação e rejeição v1 passando.

**Rastreabilidade.** RNF-002 → ADR-071/076 → identidade do signatário, vigência e revogação verificáveis → testes dirigidos. A aplicação dos artefatos na rede depende da M1C-B e da revisão exigida pela ADR-075.
