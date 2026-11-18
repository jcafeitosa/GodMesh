# Evidência — gate BC (domínio e sync), 18/11/2026

**Ambiente:** macOS, worktree `room/bc-dominio-e-sync-dw5h`, branch `room/bc-dominio-e-sync-dw5h`. **Comandos:** `cargo test --workspace`, `cargo clippy --workspace --all-targets`. **Dependências externas:** nenhuma adicionada.

**Origem:** achados do gate `wfr_414e32e9` sobre `crates/godmesh-emergency/src/incident.rs` e `crates/godmesh-sync/src/outbox.rs`.

**Regras de referência:** ADR-058 (taxonomia de ACK e proibição de escalada), ADR-061 (publicar ≠ confirmar; replay não reemite efeito irreversível), ADR-065 (classe `incidente_sos`), `migrations/0001_core.sql` (`sos_ack` append-only, CHECK de `ack_class`).

## Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `crates/godmesh-emergency/src/incident.rs` | guarda de estado em `fail_persistence`/`mark_outcome_unknown`; `ack_class_name` de `OutcomeUnknown`; `persist` recebe `at_unix` e recusa `0`; novo `IncidentError::MissingTimestamp`; doc do módulo |
| `crates/godmesh-emergency/tests/incident.rs` | 10 testes novos + 4 existentes ajustados ao novo contrato |
| `crates/godmesh-sync/src/outbox.rs` | `confirm` exige entrada publicada; `replay_from` valida prefixo; `publish_next` devolve `PublishOutcome`; doc corrigida; `last_sequence` |
| `crates/godmesh-sync/src/lib.rs` | doc passa a descrever o que existe (inbox/outbox em memória) e o que não existe (payload, projeção, publisher) |
| `crates/godmesh-sync/tests/outbox.rs` | 6 testes novos + 4 existentes ajustados ao novo contrato |
| `crates/godmesh-core/src/lib.rs`, `crates/godmesh-core/tests/integration.rs` | `persist_incident` passa `at_unix`; `fail_persistence` tratado como `Result` (não regride estado já persistido) |
| `crates/godmesh-server/src/api.rs` | **adaptação de chamada forçada pelo contrato**: `persist` agora exige `at_unix`, suprido com o relógio do host. Nenhuma outra alteração; `deploy/` intocado |

## Fase vermelha (antes da implementação)

Testes escritos primeiro. O contrato novo não existia, então a suíte não compilava — cada erro aponta uma regra ausente:

```
error[E0061]: this method takes 2 arguments but 3 arguments were supplied
   --> crates/godmesh-emergency/tests/incident.rs:21:9
   --> crates/godmesh-emergency/src/incident.rs:143:12
error[E0599]: no method named `expect` found for unit type `()` in the current scope
  --> crates/godmesh-emergency/tests/incident.rs:59:54
error[E0599]: no method named `expect` found for unit type `()` in the current scope
  --> crates/godmesh-emergency/tests/incident.rs:76:52
error[E0061]: this method takes 2 arguments but 3 arguments were supplied
  --> crates/godmesh-emergency/tests/incident.rs:103:9
error[E0308]: mismatched types
   --> crates/godmesh-emergency/tests/incident.rs:120:52
error[E0308]: mismatched types
   --> crates/godmesh-emergency/tests/incident.rs:135:64
error[E0308]: mismatched types
  --> crates/godmesh-sync/tests/outbox.rs:23:35
error[E0308]: mismatched types
```

Depois da primeira correção de `incident.rs`, um teste novo falhou por asserção minha, não por produto — `closed_incident()` produz 4 ACKs (persistido, replicado, materializado, humano), não 3. Corrigido para comparar a trilha inteira antes/depois, o que prova mais que a contagem:

```
---- fail_persistence_from_closed_is_refused_and_state_is_unchanged stdout ----
thread panicked at crates/godmesh-emergency/tests/incident.rs:122:5:
assertion `left == right` failed: nenhum ACK novo é criado por transição ilegal
  left: 4
 right: 3
```

## Testes novos (16)

`crates/godmesh-emergency/tests/incident.rs`:

1. `fail_persistence_from_triggered_locally_is_accepted`
2. `fail_persistence_from_closed_is_refused_and_state_is_unchanged`
3. `fail_persistence_from_persisted_local_is_refused`
4. `outcome_unknown_from_closed_is_refused_and_state_is_unchanged`
5. `outcome_unknown_is_refused_before_persistence`
6. `outcome_unknown_only_after_a_real_send`
7. `unknown_outcome_never_claims_it_was_replicated`
8. `persist_records_the_real_instant`
9. `persist_refuses_zero_instant`

(9 testes; `closed_incident` é helper.)

`crates/godmesh-sync/tests/outbox.rs`:

10. `confirm_without_entry_is_refused`
11. `confirm_of_unpublished_entry_is_refused`
12. `confirm_refuses_sequence_that_was_never_appended`
13. `publish_next_reports_nothing_to_publish`
14. `replay_from_prefix_ahead_of_confirmed_signals_incomplete_recovery`
15. `replay_from_behind_confirmed_prefix_replays_conservatively`

(6 testes.)

## Decisões de contrato (com justificativa)

**1. `fail_persistence` e `mark_outcome_unknown` devolvem `Result<(), IncidentError>`.**
Antes, as duas transições levavam `Closed` de volta a `NotPersisted`/`OutcomeUnknown`, reescrevendo o passado de uma trilha que a ADR-058 quer append-only. A falha contábil de uma tentativa não apaga evidência de outra. Regras: `NotPersisted` só a partir de `TriggeredLocally`; `OutcomeUnknown` só a partir de `PersistedLocal`/`ReplicatedToAuthority`; em qualquer outro estado, `IllegalTransition` e estado inalterado.

**2. `ack_class_name` de `OutcomeUnknown` = `transport_accepted`.**
`OutcomeUnknown` é envio sem recibo: nada prova replicação. `replicated_to_authority` era sobre-alegação e violava a proibição de escalada de ACK da ADR-058. `transport_accepted` existe no CHECK de `sos_ack` em `migrations/0001_core.sql` e não promete mais que o transporte.

**3. `persist(inbox_ref, retention_policy, at_unix)`.**
`at_unix: 0` era um instante inventado gravado em `sos_ack.at_unix` (`NOT NULL`). Agora o instante vem do chamador e `0` é recusado com `IncidentError::MissingTimestamp`: ausência de instante não é meia-noite de 1970. O núcleo (`persist_incident`) e o servidor passam o relógio real; nenhum crate inventa tempo.

**4. `confirm` recusa entrada ausente e entrada não publicada.**
Duas variantes novas, e não a reutilização de `ConfirmationOutOfOrder`: são causas distintas e acionáveis de forma distinta. `ConfirmationWithoutEntry` = não há o que confirmar (o bug original aceitava `confirm(1)` em outbox vazio e depois o `append(1)` nascia abaixo do prefixo, invisível para `pending`/`publish_next`); `ConfirmationOfUnpublished` = publicar não é confirmar, e confirmar o que nunca saiu é mentira; `ConfirmationOutOfOrder` fica só para lacuna no prefixo. A ordem das checagens é existência → publicado → ordem, porque um outbox não tem opinião sobre a ordem de uma sequência que nunca enfileirou.

**5. `replay_from(prefix) -> Result<ReplayPlan, OutboxError>`.**
Prefixos à frente do confirmado devolvem `RecoveryIncomplete { requested, confirmed }`. `replay_from(99)` devolvia plano vazio, indistinguível de "nada a reproduzir" — ou seja, recuperação incompleta se passava por recuperação completa. Prefixo anterior ao confirmado continua permitido (reproduz mais, sempre sobre sequências), o que é recuperação conservadora.

**6. Documentação como descrição, não como prova.**
O módulo afirmava "replay reconstrói projeção". O tipo guarda `(seq, EffectKind, published)` — sem payload, identidade de evento ou digest — e portanto só produz um **plano** (contagem e primeira sequência). A doc do módulo, de `ReplayPlan`, de `replay_from` e de `lib.rs` agora dizem isso, e `lib.rs` deixou de negar a existência de `inbox`/`outbox`. `PublishOutcome::NothingToPublish` era variante pública inalcançável: `publish_next` deixou de devolver `Option<PublishOutcome>` e passa a devolver `PublishOutcome`, tornando o desfecho "nada a publicar" construível em vez de removê-lo.

## Ajustes em testes existentes (justificados pelo contrato)

- `persistence_moves_state_and_yields_only_persisted_local_ack`, `securer_incident`, `persist_*`: passam `at_unix` real.
- `persistence_failure_yields_not_persisted_and_says_nao_salvo`, `unknown_outcome_is_not_success`, `outcome_unknown_only_after_a_real_send`: passam a consumir o `Result` (`.expect`/`.unwrap`).
- `unknown_outcome_is_not_success`: passa a exigir `transport_accepted` em vez de `replicated_to_authority` — é a correção do achado.
- `gap_in_confirmation_is_refused`: publica as duas entradas antes de `confirm(2)`. Sem isso, com `ConfirmationOfUnpublished` novo, a recusa poderia vir do motivo errado e o teste não provaria nada sobre lacuna de prefixo. A asserção ficou mais forte (também verifica que o prefixo não avança e que 1→2 confirma na ordem).
- `publish_next` agora devolve `PublishOutcome` (sem `Option`) nos três testes que o chamavam.

Nenhum teste foi removido, nenhum `#[ignore]`, nenhuma tolerância nova. `godmesh-core/tests/integration.rs` também ganhou o instante real.

## Depois

`cargo test --workspace` — 28 blocos `test result`, todos `ok`:

| Binário | Testes |
|---|---|
| `godmesh-core` integration | 7 |
| `godmesh-emergency` incident | 19 |
| `godmesh-identity` grant_predicate | 8 |
| `godmesh-policy` high_water | 6 |
| `godmesh-policy` retention | 6 |
| `godmesh-protocol` ack_class | 4 |
| `godmesh-protocol` ack_composition | 5 |
| `godmesh-protocol` cursor | 5 |
| `godmesh-protocol` version | 6 |
| `godmesh-server` contract | 3 |
| `godmesh-sync` inbox | 5 |
| `godmesh-sync` outbox | 13 |
| `godmesh-sync` position | 4 |
| unittests de lib/main e doc-tests (15 blocos) | 0 |
| **Total** | **91 aprovados, 0 falhas, 0 ignorados** |

Saída literal dos dois binários tocados:

```
     Running tests/incident.rs (target/debug/deps/incident-93aacc247794ba80)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/outbox.rs (target/debug/deps/outbox-e4a655bb1877cb2f)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo clippy --workspace --all-targets`:

```
    Checking godmesh-server v0.1.0 (.../crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

0 avisos, 0 erros (nenhuma linha `warning:`/`error`).

## O que isso NÃO prova

- **Durabilidade continua inexistente.** `Outbox` é `Vec` em memória: sem WAL, sem transação, sem restart. A guarda de `confirm` impede mentira no processo; não impede perda de dados no crash.
- **`unix` do servidor é relógio de host não autenticado.** `SystemTime::now()` prova ordem local, não tempo confiável da autoridade (ADR-059 trata frescor; nada disso foi exercitado aqui). Teste de integração real com PostgreSQL não foi executado nesta rodada.
- **`replay_from` valida o prefixo pelo `confirmed` do próprio outbox**, não por prova externa do destino. Um `confirmed` corrompido na memória ainda seria aceito como verdade.
- **Nenhum publisher, broker, consumidor, retry ou projeção real** foi exercitado; `ReplayPlan` é contagem, não estado.
- **A classe de `OutcomeUnknown` subdeclara por construção.** O estado não retém se antes era `PersistedLocal` ou `ReplicatedToAuthority`, então a classe fica no piso `transport_accepted`. As classes anteriores continuam na trilha `acks()`; nenhuma é apagada. Subdeclarar é seguro para a ADR-058; sobredeclarar era o bug.
- **Compatibilidade:** `persist`, `fail_persistence`, `mark_outcome_unknown`, `publish_next` e `replay_from` são mudanças de assinatura incompatíveis. Todos os chamadores do workspace foram atualizados; não há consumidor externo publicado, então não há versionamento de API a fazer — se houver, a mudança exige plano de migração.