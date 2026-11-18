# Evidência — Programa A, segundo ciclo TDD (17/11/2026)

**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma.

## Resultado observado

```
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
```

- **39 testes, 39 aprovados.**
- `cargo clippy --workspace --all-targets`: **0 avisos / 0 erros**.

## Novos invariantes cobertos neste ciclo

| Contrato | Teste que falha se violado |
|---|---|
| ADR-056 (janela N/N-1) | Peer duas majors atrás é rejeitado; peer futuro é rejeitado; janela zero é inválida |
| ADR-056 (feature crítica) | Peer dentro da janela **sem** feature crítica é rejeitado; feature opcional ausente não é erro |
| ADR-065 (retenção) | Classe sem política não persiste (`PolicyMissing`); tenant não amplia prazo; cópia em outra jurisdição sem política própria é recusada; prazo zero é inválido |
| ADR-064 (anti-rollback) | Contador atrás da marca = `RestoreSuspected`; igual = `ReplaySuspected`; outro dispositivo no vínculo = `CloneSuspected`; marca só avança; dispositivo sem contador protegido não prova nada |

## O que isso NÃO prova

- Nada de wire, HTTP, banco, mídia, cliente, plugin ou rede foi executado.
- Nenhum parâmetro quantitativo foi medido; janela, prazo e marca são valores de teste, não valores aprovados de produto.
- Os crates ainda não se integram entre si: cada um valida seu contrato isoladamente.
