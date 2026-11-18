# Evidência — Programa A, terceiro ciclo TDD: máquina de estados do SOS (17/11/2026)

**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma.

## Resultado observado

```
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
```

- **49 testes, 49 aprovados** (19 novos neste crate).
- `cargo clippy --workspace --all-targets`: 0 avisos, 0 erros.

## Invariantes do incidente SOS agora presos em teste

| Regra (ADR/SPEC) | Teste |
|---|---|
| Sem persistência não existe "acionado" | `new_incident_is_local_attempt_and_never_says_acionado` |
| Persistência local produz apenas `persisted_local` | `persistence_moves_state_and_yields_only_persisted_local_ack` |
| Não se salta de gatilho para entrega ou humano | `cannot_jump_from_triggered_to_delivery_or_human` |
| ACK humano exige prova de pessoa | `human_acknowledgement_requires_person_proof` |
| ACK humano só após ACK técnico | `human_acknowledgement_after_technical_ack_moves_to_human` |
| Falha de persistência vira `NotPersisted` com rótulo "não salvo" | `persistence_failure_yields_not_persisted_and_says_nao_salvo` |
| Resultado desconhecido não é sucesso e não promove classe | `unknown_outcome_is_not_success` |
| Encerrar exige ACK humano | `close_requires_human_ack_or_cancel` |
| Classe de retenção do incidente é `incidente_sos` (ADR-065: 12 meses) | `absent_revocation_is_not_unknown_and_blocks_nothing_but_state_does_not_lie` |
| Trilha de ACK é append-only e registra a fonte | `ack_trail_is_append_only_and_records_source` |

## Correção de teste feita durante o ciclo

`human_acknowledgement_requires_person_proof` falhou na primeira execução porque misturava duas violações (estado errado **e** prova ausente). Reescrevi o teste para isolar a regra de prova no estado correto — não afrouxei a asserção, separei o invariante.

## O que isso NÃO prova

- Nada foi enviado, persistido em banco, replicado ou atendido por pessoa.
- Não há integração com o predicado de grant (`godmesh-identity`) nem com retenção (`godmesh-policy`); os crates ainda não se conversam.
- Nenhum prazo, markup ou transporte foi executado.
