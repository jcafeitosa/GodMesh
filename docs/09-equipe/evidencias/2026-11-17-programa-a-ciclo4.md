# Evidência — Programa A, quarto ciclo: composição do núcleo (17/11/2026)

**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma (só crates do workspace).

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
     Running tests/cursor.rs (target/debug/deps/cursor-e12bb93bab62134c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/version.rs (target/debug/deps/version-1de034f4d18006a0)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/godmesh_sync-32803dc125902f9b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/position.rs (target/debug/deps/position-631b6363c3fec994)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- **56 testes, 56 aprovados.**
- `cargo clippy --workspace --all-targets`: 0 avisos, 0 erros.

## O que a composição passou a garantir

| Teste de integração | Fronteiras atravessadas |
|---|---|
| `incident_is_refused_when_grant_predicate_denies` | identidade → emergência: predicado nega, **nada é criado nem enfileirado** |
| `incident_opens_only_after_grant_allows_and_stays_unpersisted` | identidade → emergência: abrir não é persistir |
| `persist_requires_retention_class_and_marks_not_persisted_when_missing` | política → emergência: sem classe, estado vira `NotPersisted` |
| `ack_class_round_trips_between_emergency_and_protocol` | emergência ↔ protocolo: o nome da classe é aceito só se pertencer ao conjunto fechado |
| `human_ack_requires_technical_stage_across_crates` | emergência → protocolo: ordem não pode ser saltada |
| `event_is_refused_when_retention_class_is_unknown` | política → sync: evento sem classe não grava |
| `offline_incident_requires_high_water_mark_to_advance` | política → emergência: restore, replay e clone negados antes do incidente |

## Correções que os próprios testes forçaram

1. Abrir incidente **não** passou a exigir política de retenção: o teste mostrou que persistência é a fronteira correta do fail-closed, não a criação do objeto. Lição: fail-closed tem que estar na fronteira do efeito, não um passo antes.
2. `clippy` recusou `record_event` com 8 argumentos; a correção foi `EventDraft`, que também deixa a chamada legível.

## O que isso NÃO prova

- Nenhuma persistência real: `inbox_ref` é string, não banco.
- Nenhuma rede, servidor, mídia, cliente ou plugin.
- Não há verificação de que a política de retenção da cópia está declarada (só a classe de origem).
- Integração é em memória e mono-processo.
