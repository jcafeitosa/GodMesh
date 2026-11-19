# M4-B2-B — snapshot por QUIC em laboratório

A coordenação aprova a [SPEC M4-B2-B v1.1](../../08-specs/federacao/m4-b2-b-transporte-snapshot.md), SHA-256 `6c6bbea03d15a9335e25977439efef2df9b3def90cc71997f462b5d8c755ff40`, conforme a ADR-075. A revisão independente aprovou o conteúdo, e o delta final corrigiu o número de versão no campo Estado. A dependência [M4-B2-A](2026-11-18-m4-b2-a-snapshot-transacional.md#aceite-transacional) foi aceita. A aprovação cobre o escape para snapshot no build de integração, a migração aditiva `0017` e a política sintética de intenção e marcador.

1. **Transporte.** Reconhecer o escape somente no início da stream e com opt-in de laboratório. Aplicar tetos por tipo antes de ler o corpo. Preservar o framing positivo e o teto A2 com e sem opt-in.
2. **Controles.** Implementar objetos JSON estritos e correlacionar pedido, pacote e ACK na mesma sessão, stream e namespace. Exercitar limites, divergência, truncamento, FIN e bytes extras.
3. **Intenção durável.** Registrar token, detalhe versionado e marcador. Ordenar criação, aplicação e resolução pelo guard comum. O Pull e a aplicação A2 consultam o marcador também sem opt-in. Provar fronteiras de TTL e as duas ordens entre aplicação e resolução, além de revogação e reinício.
4. **Integração.** Ligar o fluxo ao harness de processos. Compartilhar a exclusão com a A2 por chave. Reler o estado durável antes de retomar após incerteza. Emitir ACK apenas na stream original depois do commit.
5. **Aceite.** Executar A/B/C com bancos e chaves próprios, ingresso oficial e piso M3. Verificar perda de ACK, efeito incerto, isolamento dos destinos e regressão A2. Registrar revisão independente, suíte, invariantes e comportamento após restart.

A implementação permanece restrita ao laboratório. Restore e reabertura exigem a B2-C, que segue como proposta documental.

## Aceite operacional

Código validado em `eaab4a676e079fc32e2116c447500aea9de338a5`. A revisão independente aprovou codec, transporte, persistência, integração e a correção do vínculo externo da fixture. O build de integração verifica o registro corrente, o identificador do sistema PostgreSQL e o OID da base antes de migrar ou abrir listeners. Os negativos recusaram uma geração superada e uma base recriada com o mesmo nome, endereço e identidade. Uma cópia física que preserve ambos os identificadores fica fora da cobertura dessa checagem.

- **A/B/C em IPv4 e IPv6.** Três processos no mesmo host, com bancos PostgreSQL 16.15 e chaves distintos. O ingresso usou as APIs oficiais e o piso M3 estava efetivo. A observou o ACK de C na stream original após o commit. B confirmou a cópia antes de uma perda de ACK simulada. Após reinício sem opt-in, a intenção persistida ainda bloqueava Pull em B e C continuou por A2. B reconciliou o comprovante e retomou pelo checkpoint confirmado.
- **Incerteza e isolamento.** Em C, a parada após gravar a intenção preservou o checkpoint. Após reiniciar, C confirmou ausência de efeito e fez novo pedido. Uma falha injetada durante outro commit preservou a intenção, sem comprovante novo nem ACK. C releu a base 6 e confirmou um novo corte 8. B continuou progredindo e a comparação final confirmou os envelopes originais byte a byte.
- **Banco e suíte.** O upgrade `0016 → 0017` preservou checksums anteriores, posição legada, reserva, checkpoint e suspensão. A reexecução não reaplicou migrações. Passaram 76 invariantes SQL. `cargo test --workspace --locked --offline` reportou 620 aprovados e um ignorado. A execução com `--all-features` reportou 629 aprovados e dois ignorados. A regressão de retenção passou isoladamente e o entrypoint ignorado do harness foi executado nos ensaios de rede. Clippy com `--all-targets -D warnings` passou com e sem a feature. O ensaio A2 em IPv6 também passou. `Cargo.lock` e as migrações `0001`–`0016` foram preservados.

O aceite cobre o transporte de snapshot de laboratório. A regressão de progresso na discagem sequencial, reproduzida quando B fica silencioso desde o boot, segue no [recorte de discagem QUIC](../../08-specs/federacao/quic-discagem-pares-pendentes.md) antes da B2-C. O ensaio em três hosts independentes permanece pendente.
