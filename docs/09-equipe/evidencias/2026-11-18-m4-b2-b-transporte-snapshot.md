# M4-B2-B — snapshot por QUIC em laboratório

A coordenação aprova a [SPEC M4-B2-B v1.1](../../08-specs/federacao/m4-b2-b-transporte-snapshot.md), SHA-256 `6c6bbea03d15a9335e25977439efef2df9b3def90cc71997f462b5d8c755ff40`, conforme a ADR-075. A revisão independente aprovou o conteúdo, e o delta final corrigiu o número de versão no campo Estado. A dependência [M4-B2-A](2026-11-18-m4-b2-a-snapshot-transacional.md#aceite-transacional) foi aceita. A aprovação cobre o escape para snapshot no build de integração, a migração aditiva `0017` e a política sintética de intenção e marcador.

1. **Transporte.** Reconhecer o escape somente no início da stream e com opt-in de laboratório. Aplicar tetos por tipo antes de ler o corpo. Preservar o framing positivo e o teto A2 com e sem opt-in.
2. **Controles.** Implementar objetos JSON estritos e correlacionar pedido, pacote e ACK na mesma sessão, stream e namespace. Exercitar limites, divergência, truncamento, FIN e bytes extras.
3. **Intenção durável.** Registrar token, detalhe versionado e marcador. Ordenar criação, aplicação e resolução pelo guard comum. O Pull e a aplicação A2 consultam o marcador também sem opt-in. Provar fronteiras de TTL e as duas ordens entre aplicação e resolução, além de revogação e reinício.
4. **Integração.** Ligar o fluxo ao harness de processos. Compartilhar a exclusão com a A2 por chave. Reler o estado durável antes de retomar após incerteza. Emitir ACK apenas na stream original depois do commit.
5. **Aceite.** Executar A/B/C com bancos e chaves próprios, ingresso oficial e piso M3. Verificar perda de ACK, efeito incerto, isolamento dos destinos e regressão A2. Registrar revisão independente, suíte, invariantes e comportamento após restart.

A implementação permanece restrita ao laboratório. Restore e reabertura exigem a B2-C, que segue como proposta documental.
