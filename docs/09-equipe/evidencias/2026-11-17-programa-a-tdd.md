# Evidência — Programa A, primeiro ciclo TDD (17/11/2026)

**Ambiente:** macOS, `rustc 1.99.1`, `cargo 1.99.1`. **Comando:** `cargo test --workspace` e `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma.

## Resultado observado

```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- `cargo clippy --workspace --all-targets`: sem avisos (lints do workspace em `deny`).
- 21 testes executados, 21 aprovados, 0 falharam.

## O que isso prova

1. **ADR-058**: classe de ACK desconhecida ou ausente é rejeitada; alegar classe superior à evidência é erro `PromotionRefused`; não existe API que promova por tempo.
2. **ADR-057**: cursor sem partição ou de epoch incomparável é rejeitado; cursor é vinculado a principal e filtros; posição canônica ordena por epoch e distingue partições; lacuna exige rebootstrap.
3. **ADR-059/064**: grant offline nega sem fonte de frescor confiável, com revogação desconhecida, epoch incomparável, ação não enumerada ou classe de retenção ausente; compartilhar posição precisa offline é sempre negado.

## O que isso NÃO prova

- Nenhum servidor, wire, transporte, banco, API HTTP, mídia, cliente ou plugin foi executado.
- Nenhum parâmetro quantitativo (janela N/N-1, frescor, TTL, RPO/RTO) foi medido.
- Não há integração entre os crates; cada um valida seu próprio contrato.
- Nenhuma SPEC é declarada implementada; o código cobre uma fatia dos contratos aprovados no gate técnico.
