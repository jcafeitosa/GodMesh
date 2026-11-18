# Evidência — Inbox de sync no servidor (18/11/2026)

**Branch:** `room/se-sync-inbox-no-servidor-v7t3`
**Ambiente:** macOS arm64; Rust 1.99.1 (`cargo 1.99.1 (5f94df478 2026-08-27)`); Postgres 18.7
(`PostgreSQL 18.7 on aarch64-unknown-linux-musl`) em
`postgres://godmesh:godmesh_dev_only@localhost:55432` — desenvolvimento local-first, **sem
contêiner nesta fase**. Banco próprio desta suíte: **`godmesh_se`**, criado do zero com
`migrations/0001_core.sql` + `0002_incident_tenancy.sql` + `0005_sync_inbox.sql`. Nenhum
`DROP`/`TRUNCATE` no banco de desenvolvimento compartilhado `godmesh`.

**Escopo entregue:** `POST /v1/sync/inbox` e `GET /v1/sync/state`, atrás do `require_grant`
já existente. `auth.rs` não foi tocado.

**Regras de referência:** ADR-057 (posição canônica, `partition` e `authority_epoch`
obrigatórios), ADR-058 (ACK é evidência de fronteira; teto `persisted_local`), ADR-061
(inbox/outbox duráveis), ADR-062/065/066 (nada persistido sem classe + cópia primária;
jurisdição vem de `tenant_region`), `docs/07-contratos/eventos-sincronizacao.md` §6
("evento futuro chega antes da lacuna → guardar como pendente; não confirmar cursor além do
prefixo contíguo").

## 1. Dependências

**Nenhuma dependência nova.** Verificação de versão resolvida (18/11/2026, fonte docs.rs):

```
docs.rs axum 0.8.9: 200      → /axum/latest/ também resolve 0.8.9
docs.rs sqlx 0.9.1: 200      → /sqlx/latest/ também resolve 0.9.1
```

O servidor já usava essas versões; nenhuma foi alterada nesta fatia. Reuso **interno** de
`godmesh-sync` (já em `Cargo.toml` desde a fatia de eventos) para a decisão de Inbox. Nada
foi atualizado nem migrado.

## 2. Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `crates/godmesh-sync/src/inbox.rs` | novo `Inbox::resuming(origem, tenant, stream, partição, epoch, prefixo_durável)` e dedupe contra o **prefixo durável** (`seq <= contiguous → Duplicate`), não só contra o `seen` do lote; `Gap` passa a usar `saturating_add` |
| `crates/godmesh-sync/tests/inbox.rs` | +3 testes: retomada trata posição coberta como duplicata e não regride o prefixo; só `prefixo+1` é aceito após retomada; retomar de 0 é igual a inbox nova |
| `migrations/0005_sync_inbox.sql` | **novo**: `sync_inbox_state(tenant_id, origin_node, stream_id, partition, authority_epoch, contiguous_prefix, updated_at)`, PK nas cinco colunas, `CHECK (contiguous_prefix >= 0)`, `partition >= 0`, `authority_epoch > 0`, textos não vazios |
| `migrations/tests/0005_invariants.sql` | **novo**: 6 invariantes (PK, prefixo negativo, partição negativa, epoch zero, dimensões do namespace, avanço do prefixo) |
| `migrations/tests/run_all.sh` | aplica `0005` e exige os 6 `OK` do arquivo novo |
| `crates/godmesh-server/src/inbox.rs` | **novo**: casos de uso `receive_batch` e `read_state`, prefixo durável, lacunas derivadas, teto `MAX_INBOX_BATCH = 100` |
| `crates/godmesh-server/src/events.rs` | visibilidade `pub(crate)` em `contiguous_prefix`, `resolve_jurisdiction`, `resolve_retention_class` — reuso, sem mudança de comportamento |
| `crates/godmesh-server/src/api.rs` | variante `ApiError::InboxGap { expected, received, accepted, duplicates, contiguous_prefix }` |
| `crates/godmesh-server/src/http.rs` | rotas `/v1/sync/inbox` e `/v1/sync/state`, handlers, 409 `sequence_gap` com corpo acionável; `extra()` devolve lista de campos |
| `crates/godmesh-server/src/lib.rs` | `pub mod inbox` |
| `crates/godmesh-server/tests/inbox.rs` | **novo**: 12 testes de integração HTTP no fio contra `godmesh_se` |

Nenhum teste existente foi alterado ou enfraquecido. `crates/godmesh-server/src/auth.rs` não
foi tocado.

## 3. Contrato entregue

### `POST /v1/sync/inbox` (autenticado por `Authorization: Bearer`)

```json
{
  "retention_class": "mensagem",
  "positions": [
    {"origin_node":"node-wire","stream_id":"s1","partition":1,"authority_epoch":7,"seq":1},
    {"origin_node":"node-wire","stream_id":"s1","partition":1,"authority_epoch":7,"seq":2}
  ]
}
```

Corpo com `deny_unknown_fields`: `tenant_id`/`principal` no corpo são recusados (tenancy vem
do grant). `retention_class` fica no **envelope**, não em cada posição: toda posição aceita
vira linha em `event_position`, que não existe sem classe com política na jurisdição do tenant
(ADR-062). **Teto de 100 posições por lote** (`MAX_INBOX_BATCH`), declarado e recusado — nunca
truncado em silêncio. Um lote é de **um único namespace** `(origem, stream, partição, epoch)`.

| Situação | Status | Corpo |
|---|---|---|
| lote contíguo aceito | `200` | `{accepted, duplicates, contiguous_prefix, ack_class:"persisted_local"}` |
| reenvio do mesmo lote | `200` | `accepted: 0`, `duplicates: n`, mesmo `contiguous_prefix` |
| posição já vista (coberta pelo prefixo) | `200` | conta em `duplicates`, não reprocessa, não regrava |
| lacuna | `409` | `sequence_gap` + `expected`, `received`, `accepted`, `duplicates`, `contiguous_prefix` |
| posição de outra partição/epoch/stream/origem no mesmo lote | `400` | `cursor_incomparable` (nada é gravado) |
| lote vazio ou com mais de 100 posições | `422` | `invalid_request` |
| `seq` zero / campo vazio | `422` | `invalid_request` |
| classe sem política na jurisdição | `409` | `retention_policy_missing` |
| tenant sem jurisdição declarada | `409` | `retention_scope_unresolved` |
| grant ausente/desconhecido | `401` | `unauthenticated` |

`ack_class` da resposta nunca passa de `persisted_local`; o corpo de erro usa o piso
`transport_accepted` (o erro não confirma nada).

### `GET /v1/sync/state?stream_id&partition&authority_epoch[&origin_node]`

```json
{"contiguous_prefix": 4, "last_recorded_at": "2026-11-18T15:59:23.584617Z", "gaps": []}
```

`last_recorded_at` é ISO-8601 UTC do último avanço do checkpoint. `gaps[]` são intervalos
`{origin_node, from, to}` derivados das posições duráveis (um buraco em `event_position`
aparece mesmo depois de reinício; cada intervalo é inclusivo). Stream desconhecida →
`{"contiguous_prefix":0,"last_recorded_at":null,"gaps":[]}` com **200**, não erro. Sem
`origin_node`, o estado agrega as origens do namespace de forma conservadora: prefixo =
**mínimo** entre elas.

### Persistência do prefixo

`sync_inbox_state` é gravada na **mesma transação** que insere as posições em
`event_position` (ADR-057/061), com `ON CONFLICT ... DO UPDATE SET contiguous_prefix =
GREATEST(atual, novo)` — o prefixo nunca regride. A leitura retoma de
`max(checkpoint gravado, prefixo derivado de event_position)`, então uma escrita feita por
`POST /v1/events` (que também grava em `event_position`) não deixa o Inbox tratando posição já
durável como novidade.

## 4. Prova de vermelho (a regra mutada quebra o teste)

Três mutações deliberadas, cada uma revertida depois de medida:

```
# 4.1 lacuna continuava o lote em vez de parar nela (removeu `break`)
test mixed_batch_accepts_the_contiguous_prefix_and_reports_the_gap ... FAILED
  panicked at crates/godmesh-server/tests/inbox.rs:397:5:
  left: Number(7)   right: 6
test result: FAILED. 11 passed; 1 failed

# 4.2 checagem de namespace do lote desativada
test batch_mixing_partitions_is_400_cursor_incomparable_and_writes_nothing ... FAILED
test result: FAILED. 11 passed; 1 failed

# 4.3 upsert do checkpoint desativado
test state_reports_prefix_last_recorded_at_and_gaps ... FAILED
test gap_is_409_sequence_gap_and_does_not_advance_past_it ... FAILED
test mixed_batch_accepts_the_contiguous_prefix_and_reports_the_gap ... FAILED
test contiguous_batch_is_accepted_and_advances_the_prefix ... FAILED
test prefix_survives_a_new_process_state_by_being_read_from_the_database ... FAILED
test result: FAILED. 7 passed; 5 failed
```

Observação medida, não suposta: em 4.2 só o teste de **partição** falhou; o de **epoch**
continuou verde porque a rejeição de epoch vem do domínio (`InboxError::EpochIncomparable` →
`400 cursor_incomparable`), enquanto a de partição dentro do mesmo lote depende da checagem de
fronteira. As duas camadas foram exercitadas.

## 5. Verificações executadas

### `cargo test --workspace` (com `DATABASE_URL` apontando para `godmesh_se`)

```
test result: ok. 7 passed      (godmesh-core/tests/integration.rs)
test result: ok. 19 passed     (godmesh-emergency/tests/incident.rs)
test result: ok. 8 passed      (godmesh-identity/tests/grant_predicate.rs)
test result: ok. 6 passed      (godmesh-policy/tests/high_water.rs)
test result: ok. 6 passed      (godmesh-policy/tests/retention.rs)
test result: ok. 4 passed      (godmesh-protocol/tests/ack_class.rs)
test result: ok. 5 passed      (godmesh-protocol/tests/ack_composition.rs)
test result: ok. 10 passed     (godmesh-protocol/tests/cursor.rs)
test result: ok. 6 passed      (godmesh-protocol/tests/version.rs)
test result: ok. 1 passed      (godmesh-server/unittests src/lib.rs)
test result: ok. 6 passed      (godmesh-server/unittests src/main.rs)
test result: ok. 17 passed     (godmesh-server/tests/contract.rs)
test result: ok. 8 passed      (godmesh-server/tests/events.rs)
test result: ok. 12 passed     (godmesh-server/tests/inbox.rs)
test result: ok. 6 passed      (godmesh-server/tests/lifecycle.rs)
test result: ok. 0 passed; 1 ignored  (godmesh-server/tests/retention_policy_missing.rs)
test result: ok. 8 passed      (godmesh-sync/tests/inbox.rs)
test result: ok. 13 passed     (godmesh-sync/tests/outbox.rs)
test result: ok. 7 passed      (godmesh-sync/tests/position.rs)
```

Total: **149 passaram, 0 falharam, 1 ignorado** (`retention_policy_missing`, que altera o
registro de política e roda sozinho com `--ignored`). Nenhum aviso de compilação.

Os 12 testes de `tests/inbox.rs`: lote contíguo aceito; duplicata; lacuna não avança prefixo;
primeira posição de stream vazia precisa ser 1; lote misto (aceita até a lacuna e reporta);
partição mista; epoch misto; teto de 100; sem credencial; stream desconhecida; estado com
prefixo/instante/lacuna (buraco inserido direto no banco); **prefixo após "reinício"**
(pool novo + roteador novo, prefixo lido do banco).

### `cargo clippy --workspace --all-targets`

```
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.18s
```

Sem warnings: o workspace nega `clippy::all`; nada foi suprimido com `allow`.

### `migrations/tests/run_all.sh` (banco descartável criado do zero)

```
===== 0005_invariants.sql =====
NOTICE:  OK 1: namespace de Inbox duplicado recusado (PK)
NOTICE:  OK 2: contiguous_prefix negativo recusado
NOTICE:  OK 3: partição negativa recusada
NOTICE:  OK 4: authority_epoch zero recusado
NOTICE:  OK 5: partição, epoch e origem são dimensões do namespace (4)
NOTICE:  OK 6: prefixo persiste a avançar no mesmo namespace (3 -> 4)

RESULTADO: OK — 20 invariantes verificados em banco criado do zero.
```

## 6. Prova no fio (binário real, `BIND_ADDR=127.0.0.1:18098`, banco `godmesh_se`)

```
POST lote contíguo [1,2,3]
HTTP 200
{"accepted":3,"duplicates":0,"contiguous_prefix":3,"ack_class":"persisted_local"}

POST reenvio [1,2]
HTTP 200
{"accepted":0,"duplicates":2,"contiguous_prefix":3,"ack_class":"persisted_local"}

POST lacuna [5]
HTTP 409
{"accepted":0,"ack_class":"transport_accepted","contiguous_prefix":3,"detail":"posição 5 salta a lacuna: a stream espera 4; 0 posição(ões) antes da lacuna foram persistidas","duplicates":0,"error":"sequence_gap","expected":4,"received":5}

POST lote misto [4,6,7]
HTTP 409
{"accepted":1,"ack_class":"transport_accepted","contiguous_prefix":4,"detail":"posição 6 salta a lacuna: a stream espera 5; 1 posição(ões) antes da lacuna foram persistidas","duplicates":0,"error":"sequence_gap","expected":5,"received":6}

POST lote com duas partições
HTTP 400
{"ack_class":"transport_accepted","detail":"posição fora do namespace do lote: partition difere do namespace declarado (origem node-wire, stream wire-final-42028, partição 1, epoch 7)","error":"cursor_incomparable"}

POST lote de 101 posições
HTTP 422
{"ack_class":"transport_accepted","detail":"lote com 101 posições excede o teto de 100 por pedido","error":"invalid_request"}

POST sem credencial
HTTP 401
{"ack_class":"transport_accepted","detail":"credencial ausente ou desconhecida","error":"unauthenticated"}

GET /v1/sync/state?stream_id=wire-inbox-41330&partition=1&authority_epoch=7
HTTP 200
{"contiguous_prefix":4,"last_recorded_at":"2026-11-18T15:59:23.584617Z","gaps":[]}

GET /v1/sync/state?stream_id=nao-existe-…&partition=1&authority_epoch=7
HTTP 200
{"contiguous_prefix":0,"last_recorded_at":null,"gaps":[]}

GET estado com origin_node=node-wire (mesmo namespace)
HTTP 200
{"contiguous_prefix":4,"last_recorded_at":"2026-11-18T15:59:23.584617Z","gaps":[]}
```

Estado persistido depois da série (a lacuna, o lote misto recusado e o 400 não criaram linha
além do prefixo; o checkpoint acompanha exatamente as 4 posições aceitas):

```
 origin_node |     tenant_id    |   stream_id      | partition | authority_epoch | seq | retention_class | jurisdiction
-------------+------------------+------------------+-----------+-----------------+-----+-----------------+--------------
 node-wire   | tenant-inbox-br  | wire-inbox-41330 |         1 |               7 |   1 | mensagem        | BR
 node-wire   | tenant-inbox-br  | wire-inbox-41330 |         1 |               7 |   2 | mensagem        | BR
 node-wire   | tenant-inbox-br  | wire-inbox-41330 |         1 |               7 |   3 | mensagem        | BR
 node-wire   | tenant-inbox-br  | wire-inbox-41330 |         1 |               7 |   4 | mensagem        | BR
(4 rows)

     tenant_id    | origin_node |   stream_id      | partition | authority_epoch | contiguous_prefix |         updated_at
------------------+-------------+------------------+-----------+-----------------+-------------------+-----------------------------
 tenant-inbox-br  | node-wire   | wire-inbox-41330 |         1 |               7 |                 4 | 2026-11-18T15:59:23.584617Z
(1 row)
```

## 7. Limitações declaradas (nada disto foi implementado nem alegado)

1. **Sem identidade nem digest de evento.** O Inbox continua deduplicando por **tupla de
   posição**; `event_id`, hash de conteúdo e payload não existem em `event_position`. Um
   reenvio da mesma posição com conteúdo diferente não é detectável.
2. **`gaps[]` do estado é derivado, não memória de rejeições.** O servidor não persiste
   "posição recebida e recusada": a autoridade sobre a existência de lacuna é o `409
   sequence_gap`; `gaps[]` mostra buracos em `event_position`. Sem persistir a maior posição
   recusada, o estado não sabe que um cliente tentou `seq 9` — só que `seq 3` falta.
3. **Um namespace por lote.** Lote multipartição/epoch é recusado com 400 em vez de fatiado:
   `contiguous_prefix` e `accepted` precisam de um namespace único. Cliente com dois
   namespaces faz duas chamadas.
4. **Ordem do lote é a ordem de admissão.** O servidor não reordena: `[2,1]` produz lacuna
   (`expected: 1`) mesmo contendo a posição 1. Está documentado no módulo, não é acidente.
5. **Sem reordenação, sem buffer de evento futuro.** O contrato §6 fala em "guardar como
   pendente"; aqui a posição futura é recusada e não guardada. Persistir pendentes exigiria
   classe de retenção própria e decisão de esquema.
6. **Sem retenção executada sobre `sync_inbox_state`.** A tabela não tem prazo nem gatilho
   próprio; a classe de retenção do envelope vale para `event_position` (FK), não para o
   checkpoint. Política de retenção do checkpoint segue pendente.
7. **`contiguous_prefix` derivado custa `generate_series(1, max(seq))`.** É o mesmo algoritmo
   já usado em `POST /v1/events`; não é otimizado nem medido, e não há SLO declarado.
8. **`last_recorded_at` é o avanço do checkpoint**, não o instante do evento nem do
   recebimento da posição individual.
9. **Sem publicação/consumo real.** `outbox` continua em memória e não há publisher, broker,
   DLQ nem retry. O ACK para em `persisted_local` — nada foi replicado, materializado ou
   entregue.
10. **Lote máximo 100 e `retention_class` obrigatória** são parâmetros desta fatia, não
    capacidade publicada.