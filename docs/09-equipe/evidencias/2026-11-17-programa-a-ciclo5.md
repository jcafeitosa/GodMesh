# Evidência — Programa A, quinto ciclo: outbox, inbox e composição de ACK (17/11/2026)

**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma.

## Resultado observado

```
     Running unittests src/lib.rs (target/debug/deps/godmesh_core-a095a805211a2e95)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration.rs (target/debug/deps/integration-fab648a5a1d0f4df)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_emergency-f39efea3153ac1de)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/incident.rs (target/debug/deps/incident-93aacc247794ba80)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_identity-696ba6dbf6e50470)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/grant_predicate.rs (target/debug/deps/grant_predicate-7fbfe43dfbf3ac2b)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_policy-09762a999124f403)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/high_water.rs (target/debug/deps/high_water-7e45140ca16037cb)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/retention.rs (target/debug/deps/retention-a0c5472fc7771260)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_protocol-b8dc5401b00577b9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_class.rs (target/debug/deps/ack_class-3e00e907730063f1)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/ack_composition.rs (target/debug/deps/ack_composition-402abbad330ac268)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cursor.rs (target/debug/deps/cursor-e12bb93bab62134c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/version.rs (target/debug/deps/version-1de034f4d18006a0)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_sync-32803dc125902f9b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/inbox.rs (target/debug/deps/inbox-d0ef9283da158834)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/outbox.rs (target/debug/deps/outbox-e4a655bb1877cb2f)
```

- **62 testes, 62 aprovados, 0 falhas.**
- `cargo clippy --workspace --all-targets`: 0 avisos, 0 erros.

## Invariantes novos

| Contrato | Teste |
|---|---|
| ADR-061: publicar ≠ confirmar | `publish_advances_prefix_only_when_acknowledged` |
| ADR-061: confirmação em ordem, sem salto | `gap_in_confirmation_is_refused` |
| ADR-061: replay reconstrói projeção e não reemite efeito irreversível | `replay_never_reemits_irreversible_effect` |
| ADR-057: append duplicado e salto recusados | `duplicate_sequence_is_refused`, `gap_in_sequence_is_refused` |
| ADR-057: Inbox deduplica e não salta lacuna | `duplicate_event_is_deduplicated_not_reprocessed`, `gap_is_reported_and_does_not_advance_prefix` |
| ADR-057: evento de outra partição/epoch recusado | `event_from_other_partition_is_refused`, `event_from_other_epoch_is_refused` |
| ADR-058: compor ACKs nunca promove; a perna mais fraca rebaixa o conjunto | `composition_returns_lowest_class_across_layers`, `adding_weaker_layer_lowers_the_claim` |
| ADR-058: sem evidência não há alegação | `empty_composition_has_no_claim` |

## O que isso NÃO prova

- Outbox e Inbox são em memória; nenhuma durabilidade real, nenhum WAL, nenhuma transação de banco.
- Não há publicador, consumidor, broker, rede ou retry real.
- A composição de ACK não é usada ainda pelo `godmesh-core`; falta ligar.
