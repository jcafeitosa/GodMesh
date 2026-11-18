# Evidência — S-sec-2: evento assinado por nó no ingest (`POST /v1/events`)

> Execução: 18/11/2026, worktree da thread `thr_9b8z5359xf`. Branch local, sem push.
> Escopo: regra dura de `docs/04-seguranca/abuso-e-malha-maligna.md` §5 — **evento sem
> assinatura de nó autorizado não entra no log durável**. Fatia S-sec-2 (§8).

## O que foi implementado

| Peça | Onde |
|---|---|
| Envelope `SignedEvent` (domínio `godmesh.federation.event.v1`, bytes canônicos no padrão `SignedManifest`) | `crates/godmesh-federation/src/event.rs` |
| Verificação no ingest (assinatura → `node_id`↔chave → `authorized` → tenant/escopo → epoch → janela) | `crates/godmesh-server/src/events.rs::create_event` |
| Tabela de escopo `(node_id, tenant_id, stream_id)` + índice de lookup | `migrations/0010_event_signed_ingest.sql` |
| Códigos de erro 422 (`event_signature_invalid`, `event_node_unauthorized`, `event_scope_denied`, `event_epoch_stale`, `event_not_fresh`) | `crates/godmesh-server/src/api.rs` + catálogo em `http/contract.rs` |
| Contrato (`/v1/meta`, `openapi.json`): corpo `SignedEvent`, erros da rota | `http/events.rs`, `http/openapi.rs` |
| Janela de frescor própria (`GODMESH_EVENT_MAX_AGE_SECONDS`, default 300 s) | `events::EventWindow` |
| Testes de fio (HTTP/1.1 cru + SQL antes/depois) | `crates/godmesh-server/tests/event_signature.rs` |
| Invariantes de banco | `migrations/tests/0010_invariants.sql` (6 blocos `OK`) |

Sem criptografia própria: verificação via `godmesh_federation::identity`
(`ed25519_dalek` 3.0.0, `verify_strict`). Sem caminho "sem assinatura" e sem flag:
o formato antigo do corpo é recusado com `invalid_request` (cenário 9 do suíte).

## Comando 1 — suíte do workspace

```
DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh cargo test --workspace
```

Saída (resumo literal; exit code 0):

```
test result: ok. 432 passed; 0 failed; 0 ignored; ...
```

Contagem fechada: 415 (base anterior à fatia) + 7 unitários de
`godmesh-federation::event` + 10 de `tests/event_signature.rs` = **432**.

## Comando 2 — lint

```
cargo clippy --workspace --all-targets -- -D warnings
```

Sem saída (exit 0).

## Comando 3 — invariantes de banco

```
PGHOST=localhost PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only bash migrations/tests/run_all.sh
```

Saída (fim):

```
RESULTADO: OK — 50 invariantes verificados em banco criado do zero.
```

(44 já existiam; +6 da migração 0010: PK, FK de nó não registrado, CHECK de vazio,
RESTRICT no apagamento de par com escopo ativo, índice do lookup, ausência de
coluna de segredo).

## Comando 4 — os seis cenários no fio, com contagem SQL

```
DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  cargo test -p godmesh-server --test event_signature -- --test-threads=1 --nocapture
```

```
test assinatura_invalida_e_422_event_signature_invalid_e_nada_e_gravado ... (status=422)
test authority_epoch_regressivo_e_422_event_epoch_stale ... (201, depois 422, depois 201 em epoch maior)
test corpo_sem_assinatura_e_recusado_na_fronteira_sem_efeito ... (422)
test evento_assinado_por_no_autorizado_e_201_e_replay_e_200_sem_duplicar ... (201, 200)
test issued_at_fora_da_janela_e_422_event_not_fresh ... (422, 422)
test no_autorizado_nao_pode_injetar_em_nome_de_outro_no ... (422)
test no_autorizado_sem_escopo_e_422_event_scope_denied_e_nada_e_gravado ... (422)
test no_pendente_e_422_event_node_unauthorized_e_nada_e_gravado ... (422)
test no_sem_registro_e_422_event_node_unauthorized ... (422)
test tenant_assinado_diferente_do_grant_e_422_event_scope_denied ... (422)
test result: ok. 10 passed; 0 failed;
```

Cada teste faz `SELECT count(*) FROM event_position WHERE tenant_id=$1 AND
stream_id=$2` **antes e depois** da recusa e exige igualdade. Prova SQL
independente, após a execução:

```
SELECT stream_id, count(*) FROM event_position
 WHERE tenant_id LIKE 'tenant-evt-sig%' GROUP BY 1 ORDER BY 1;
```

Somente as streams dos cenários positivos aparecem: `stream-ok-*` com 1 linha
(primeira escrita; o replay de 200 não duplicou) e `stream-epoch-*` com 2
linhas (`epoch 7 seq 1` e `epoch 9 seq 1` — o `epoch 5` recusado não está lá).
Nenhuma stream dos negativos (`stream-assinatura-*`, `stream-pendente-*`,
`stream-sem-escopo-*`, `stream-janela-*`, `stream-forjado-*`,
`stream-tenant-*`, `stream-sem-registro-*`, `stream-sem-assinatura-*`) tem
linha no banco: as recusas não escreveram.

## Limite desta evidência

- O **Inbox** (`POST /v1/sync/inbox`) continua gravando posições sem o envelope
  assinado; fechar essa porta pertence à fatia de sync entre nós (M4, emenda
  ADR-072), não a S-sec-2. Nada aqui prova o caminho do Inbox.
- Concessão/revogação de `federation_peer_scope` **não tem API**: nesta fatia o
  operador escreve a linha. Auditoria de emissão de escopo (§6 da matriz) e
  validade/expiração do escopo (S-sec-3) continuam backlog.
- `GODMESH_NODE_KEY` para o nó local é consumido pelo runtime de federação
  (`main.rs`); a rota de ingest apenas verifica — a conveniência "nó local assina
  com a própria chave" está coberta pelo caminho de código `signed_event()` e
  pelos testes que a exercitam, **não** por um smoke do binário completo com a
  variável apontando para arquivo real.
