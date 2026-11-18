# Evidência — correção dos testes de outbox (18/11/2026)

**Ambiente:** macOS, worktree `room/s4-corrige-testes-do-outbox-4kd8`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma.

**Escopo:** `crates/godmesh-sync/tests/outbox.rs` e este registro documental. `crates/godmesh-sync/src/outbox.rs` e os demais crates **não** foram alterados.

## Antes

`cargo test -p godmesh-sync --test outbox` (reproduzível, não intermitente):

```
running 7 tests
test duplicate_sequence_is_refused ... ok
test gap_in_confirmation_is_refused ... ok
test gap_in_sequence_is_refused ... ok
test publish_advances_prefix_only_when_acknowledged ... ok
test replay_from_prefix_returns_only_unconfirmed ... ok
test append_keeps_declaration_order ... FAILED
test replay_never_reemits_irreversible_effect ... FAILED

---- append_keeps_declaration_order stdout ----
assertion `left == right` failed
  left: 0
 right: 2

---- replay_never_reemits_irreversible_effect stdout ----
assertion `left == right` failed
  left: 0
 right: 1

test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

A evidência do ciclo 5 afirmava 62/62 verde. Ver a seção "Correção de evidência (18/11/2026)" em `2026-11-17-programa-a-ciclo5.md`: o 62 era a soma dos binários que já tinham passado antes da falha, com fail-fast do `cargo test`.

## Causa raiz

Duas asserções estavam erradas. A implementação segue a ADR-061 e não foi tocada.

**1. `append_keeps_declaration_order`** exigia `last_contiguous() == 2` logo após dois `append`. A ADR-061 define que o prefixo contíguo representa recebimento duravelmente reconhecido pelo destino, portanto **só `confirm()` avança** o prefixo; `append()` apenas enfileira e `publish_next()` apenas marca como publicado. Era o mesmo invariante que `publish_advances_prefix_only_when_acknowledged` já provava — este teste o contradizia.

**2. `replay_never_reemits_irreversible_effect`** confirmava a entrada irreversível e depois chamava `replay_from(1)`. `replay_from(p)` percorre somente `seq > p`, então a entrada confirmada saía do replay e `skipped_irreversible` nunca poderia chegar a 1: a asserção era insatisfazível por construção. Além disso, um teste nessa forma não provaria nada sobre a regra da ADR-061, porque o caso perigoso é justamente reemitir efeito externo de entrada **pendente**.

Confirmação no código-fonte (lido antes da mudança, sem alteração): `append` só faz `entries.push`, `last_contiguous()` devolve `self.confirmed`, `confirm` exige `seq == confirmed + 1`, e `replay_from` faz `if *seq <= confirmed { continue; }`. Nenhuma violação de contrato encontrada.

## Depois

Diff aplicado (somente asserções e comentários de contrato):

```diff
@@ fn append_keeps_declaration_order
     assert_eq!(ob.pending(), 2);
-    assert_eq!(ob.last_contiguous(), 2);
+    // ADR-061: append enfileira, não confirma. O prefixo contíguo representa
+    // recebimento duravelmente reconhecido pelo destino (`confirmed`), então ele
+    // só avança em `confirm()` — nunca em `append()`. Ver
+    // `publish_advances_prefix_only_when_acknowledged`, que prova o mesmo para
+    // `publish_next()`. Asserir 2 aqui seria afirmar que enfileirar é confirmar.
+    assert_eq!(ob.last_contiguous(), 0, "append não avança prefixo; só confirm() avança");
+    // Ordem de declaração preservada: a publicação sai na ordem em que foi enfileirada.
+    assert_eq!(ob.publish_next(), Some(PublishOutcome::Published(1)));
+    assert_eq!(ob.publish_next(), Some(PublishOutcome::Published(2)));
+    assert_eq!(ob.last_contiguous(), 0, "publicar também não confirma");
 }

@@ fn replay_never_reemits_irreversible_effect
     ob.publish_next();
-    ob.confirm(1).unwrap();
+    // A entrada irreversível precisa continuar NÃO confirmada. `replay_from(p)`
+    // considera apenas o que está depois do prefixo confirmado (`seq > p`);
+    // confirmar a entrada a excluiria do replay e o teste passaria sem exercitar
+    // a regra. O risco real da ADR-061 é reemitir efeito externo de uma entrada
+    // que ficou pendente — é exatamente esse caso que provamos aqui.

-    let plano = ob.replay_from(1);
+    let plano = ob.replay_from(0);
+    assert_eq!(ob.last_contiguous(), 0, "entrada irreversível segue pendente");
     assert_eq!(plano.entries, 0, "replay reconstrói projeção, não reenvia");
-    assert_eq!(plano.skipped_irreversible, 1);
+    assert_eq!(plano.skipped_irreversible, 1, "efeito irreversível é contado e pulado");
 }
```

Nenhuma asserção foi afrouxada, nenhum `#[ignore]`, nenhum teste removido; o número de testes do binário continua 7. A cobertura aumentou: `append_keeps_declaration_order` agora também verifica a ordem de publicação efetiva, e o teste de replay também verifica que o prefixo continua em 0.

`cargo test -p godmesh-sync --test outbox`:

```
running 7 tests
test append_keeps_declaration_order ... ok
test duplicate_sequence_is_refused ... ok
test gap_in_confirmation_is_refused ... ok
test gap_in_sequence_is_refused ... ok
test publish_advances_prefix_only_when_acknowledged ... ok
test replay_from_prefix_returns_only_unconfirmed ... ok
test replay_never_reemits_irreversible_effect ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test --workspace` (28 blocos `test result`, todos `ok`):

| Binário | Testes |
|---|---|
| `godmesh-core` integration | 7 |
| `godmesh-emergency` incident | 10 |
| `godmesh-identity` grant_predicate | 8 |
| `godmesh-policy` high_water | 6 |
| `godmesh-policy` retention | 6 |
| `godmesh-protocol` ack_class | 4 |
| `godmesh-protocol` ack_composition | 5 |
| `godmesh-protocol` cursor | 5 |
| `godmesh-protocol` version | 6 |
| `godmesh-server` contract | 3 |
| `godmesh-sync` inbox | 5 |
| `godmesh-sync` outbox | 7 |
| `godmesh-sync` position | 4 |
| **Total** | **76 aprovados, 0 falhas, 0 ignorados** |

Os demais binários (unittests de lib e doc-tests) têm 0 testes. O total é 76 e não 62 porque inclui `outbox` (7), `position` (4) — que nunca chegaram a aparecer completos na evidência do ciclo 5 — e `godmesh-server/tests/contract.rs` (3), acrescentado depois pelo commit `70f0a57`.

`cargo clippy --workspace --all-targets`: `Finished dev profile`, 0 avisos, 0 erros (nenhuma linha `warning:` ou `error` na saída).

## O que isso NÃO prova

- Outbox continua em memória: sem durabilidade real, WAL ou transação de banco.
- `replay_from` recebe o prefixo como argumento em vez de usar `self.confirmed`, o que permite ao chamador passar qualquer valor. É um cheiro de API, não violação da ADR-061; corrigir exige decisão de contrato e não foi feito aqui.
- Nenhum publisher, consumidor, broker ou retry real foi exercitado.