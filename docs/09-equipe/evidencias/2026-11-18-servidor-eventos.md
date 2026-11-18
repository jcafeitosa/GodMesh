# Evidência — eventos de domínio no servidor (18/11/2026)

**Branch:** `room/sb-eventos-de-dominio-no-servidor-furh`
**Ambiente:** macOS arm64; Rust 1.99.1 (`cargo 1.99.1 (5f94df478 2026-08-27)`); Postgres de
desenvolvimento local em `postgres://godmesh:godmesh_dev_only@localhost:55432` (desenvolvimento
local-first; sem contêiner nesta fase). Banco próprio desta suíte: **`godmesh_sb`**, criado
do zero com `migrations/0001_core.sql` + `migrations/0002_incident_tenancy.sql`. Nenhum
`DROP`/`TRUNCATE` no banco de dev compartilhado `godmesh`.

**Escopo entregue:** `POST /v1/events` e `GET /v1/events`, atrás do `require_grant` já
existente (mesma semântica de auth, nada alterado em `auth.rs`).

**Regras de referência:** ADR-057 (posição canônica, cursor opaco versionado, epoch
obrigatório), ADR-058 (ACK é evidência de fronteira; teto `persisted_local` nesta fatia),
ADR-062/065 (nada persistido sem classe + cópia primária), ADR-066 (jurisdição vem de
`tenant_region`), `docs/07-contratos/eventos-sincronizacao.md` §§4–5 (cursor nunca traduzido;
lacuna nunca saltada).

## 1. Dependências

Nenhuma dependência externa nova. Uma dependência **interna** passou a ser usada pelo
servidor: `godmesh-sync` (path crate do próprio workspace), para reusar
`godmesh_sync::position::EventPosition` em vez de recriar a tupla no handler. `Cargo.lock`
ganhou apenas a aresta do pacote local.

## 2. Arquivos alterados

| Arquivo | Mudança |
|---|---|
| `crates/godmesh-sync/src/position.rs` | novo `EventPosition::admit(&self, prefixo_contíguo) -> Admission {Next, Duplicate, Gap{expected}}` — regra de lacuna do Inbox sobre a posição canônica |
| `crates/godmesh-sync/tests/position.rs` | 3 testes: só `prefixo+1` é próximo, dentro do prefixo é duplicata, salto nomeia a posição esperada |
| `crates/godmesh-protocol/src/cursor.rs` | token de cursor opaco e versionado: `Cursor::encode(last_seq)` / `Cursor::decode(token)`, `DecodedCursor`, `CursorTokenError`, base64url interno |
| `crates/godmesh-protocol/tests/cursor.rs` | 5 testes: round-trip, vínculo partição/epoch/filtro/principal, campos com separadores, token malformado/versão desconhecida, token sem partição |
| `crates/godmesh-server/src/events.rs` | **novo**: casos de uso `create_event`, `list_events`; resolução de jurisdição e de política na mesma transação; prefixo contíguo; mapeamento de erro de cursor |
| `crates/godmesh-server/src/api.rs` | variantes de erro `SequenceGap`, `CursorInvalid`, `CursorIncomparable` |
| `crates/godmesh-server/src/http.rs` | rotas `POST/GET /v1/events`, handlers, códigos HTTP e mensagem de retenção generalizada (a mesma para incidente e evento) |
| `crates/godmesh-server/src/lib.rs` | `pub mod events` |
| `crates/godmesh-server/Cargo.toml` | dependência interna `godmesh-sync` |
| `crates/godmesh-server/tests/events.rs` | **novo**: 8 testes de integração HTTP no fio contra `godmesh_sb` |

`crates/godmesh-server/src/auth.rs` **não** foi tocado. Nenhum teste existente foi alterado
ou enfraquecido.

## 3. Contrato entregue

### `POST /v1/events` (autenticado por `Authorization: Bearer`)

Corpo (`deny_unknown_fields`, então `tenant_id`/`principal` no corpo são recusados):

```json
{"origin_node":"node-wire","stream_id":"…","partition":1,"authority_epoch":7,"seq":1,"retention_class":"mensagem"}
```

| Situação | Status | Corpo |
|---|---|---|
| posição nova | `201` | `ack_class: "persisted_local"` |
| mesma tupla repetida | `200` | mesmo `ack_class` do original, sem segundo `INSERT` |
| classe sem política na jurisdição | `409` | `retention_policy_missing` |
| `seq` salta o prefixo contíguo | `409` | `sequence_gap` + posição esperada |
| tenant sem jurisdição declarada | `409` | `retention_scope_unresolved` |
| credencial ausente/desconhecida | `401` | `unauthenticated` |

`tenant_id` vem do grant verificado e `jurisdiction` de `tenant_region` — os dois resolvidos
na **mesma transação** do `INSERT` em `event_position`. A política é lida com
`FOR SHARE OF rc, cp`, e a FK `(retention_class, jurisdiction)` fecha a corrida.

### `GET /v1/events?stream_id&partition&authority_epoch&limit[&origin_node][&cursor]`

Resposta: `{"events":[{origin_node, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction}], "next_cursor": string|null}`.

| Situação | Status | Código |
|---|---|---|
| leitura autorizada (cursor ausente = início) | `200` | — |
| cursor de outra partição/epoch/tenant/stream/filtro | `400` | `cursor_incomparable` |
| cursor ilegível, versão desconhecida, vazio ou de outro principal | `400` | `cursor_invalid` |
| `limit` fora de `1..=100` | `422` | `invalid_request` |

`limit` é **obrigatório**: não há default publicado, então nenhum limite foi inventado.
O teto `100` é limite desta fatia, não SLO.

Cursor: `v1.<base64url>` de campos prefixados por tamanho, carregando versão, tenant, stream,
partição, epoch, principal, filtros, perfil (`gmp.event.v1`), escopo de retenção (jurisdição)
e a última `seq` entregue. Não é assinado — não há chave nem emissão de credencial
decididas — e por isso **não** é alegado como autenticado; a autorização continua sendo
integralmente do grant, e toda query filtra por `tenant_id` do grant.

## 4. Prova de vermelho (a regra mutada quebra o teste)

Antes da versão final, mutação deliberada de duas regras e execução dos testes de integração:

```
thread 'sequence_jump_is_409_and_the_expected_position_is_named' panicked:
  left: 201
 right: 409
  (POST seq 2 numa stream vazia foi aceito com `Admission::Next` para qualquer seq > prefixo)
thread 'cursor_from_another_partition_is_400_cursor_incomparable' panicked:
assertion `left == right` failed: cursor de outra partição precisa ser 400: HTTP/1.1 200 OK
{"events":[],"next_cursor":null}
  left: 200
 right: 400
  (sem `check_against`, o cursor da partição 1 foi usado na partição 2)
test result: FAILED. 6 passed; 2 failed
```

Regras restauradas; a suíte voltou a 8/8 (seção 5). Isso mostra que os testes medem a regra e
não apenas o caminho feliz.

## 5. Verificações executadas

### `cargo test --workspace` (com `DATABASE_URL` apontando para `godmesh_sb`)

```
test result: ok. 7 passed                 (godmesh-core/tests/integration.rs)
test result: ok. 19 passed                (godmesh-emergency/tests/incident.rs)
test result: ok. 8 passed                 (godmesh-identity/tests/grant_predicate.rs)
test result: ok. 6 passed                 (godmesh-policy/tests/high_water.rs)
test result: ok. 6 passed                 (godmesh-policy/tests/retention.rs)
test result: ok. 4 passed                 (godmesh-protocol/tests/ack_class.rs)
test result: ok. 5 passed                 (godmesh-protocol/tests/ack_composition.rs)
test result: ok. 10 passed                (godmesh-protocol/tests/cursor.rs)
test result: ok. 6 passed                 (godmesh-protocol/tests/version.rs)
test result: ok. 1 passed                 (godmesh-server/unittests src/lib.rs)
test result: ok. 6 passed                 (godmesh-server/unittests src/main.rs)
test result: ok. 17 passed                (godmesh-server/tests/contract.rs)
test result: ok. 8 passed                 (godmesh-server/tests/events.rs)
test result: ok. 0 passed; 1 ignored      (godmesh-server/tests/retention_policy_missing.rs)
test result: ok. 5 passed                 (godmesh-sync/tests/inbox.rs)
test result: ok. 13 passed                (godmesh-sync/tests/outbox.rs)
test result: ok. 7 passed                 (godmesh-sync/tests/position.rs)
```

Total: **128 passaram, 0 falharam, 1 ignorado** (`retention_policy_missing`, que altera o
registro de política e roda sozinho com `--ignored`).

### `cargo clippy --workspace --all-targets`

```
    Checking godmesh-sync v0.1.0 (…/crates/godmesh-sync)
    Checking godmesh-core v0.1.0 (…/crates/godmesh-core)
    Checking godmesh-server v0.1.0 (…/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.24s
```

Sem warnings: o workspace nega `clippy::all`; nada foi suprimido com `allow`.

### `migrations/tests/run_all.sh` (nenhuma migração alterada)

```
RESULTADO: OK — 14 invariantes verificados em banco criado do zero.
```

## 6. Prova no fio (binário real, `BIND_ADDR=127.0.0.1:18099`, banco `godmesh_sb`)

```
POST seq 1/2/3 -> 201
{"origin_node":"node-wire","stream_id":"wire-325762181","partition":1,"authority_epoch":7,"seq":1,"retention_class":"mensagem","ack_class":"persisted_local"}
{"…","seq":2,…,"ack_class":"persisted_local"}
{"…","seq":3,…,"ack_class":"persisted_local"}

POST seq 3 (duplicata) -> HTTP/1.1 200 OK
{"…","seq":3,…,"ack_class":"persisted_local"}

POST classe "nao-existe" -> HTTP/1.1 409 Conflict
{"ack_class":"transport_accepted","detail":"classe de retenção sem política aplicável à jurisdição do tenant","error":"retention_policy_missing"}

POST seq 9 (salto) -> HTTP/1.1 409 Conflict
{"ack_class":"transport_accepted","detail":"posição 9 salta a lacuna: a stream espera 4","error":"sequence_gap"}

GET &limit=2 -> HTTP/1.1 200 OK
{"events":[{…"seq":1,…,"jurisdiction":"BR"},{…"seq":2,…,"jurisdiction":"BR"}],"next_cursor":"v1.MToxfDE3OnRlbmFudC1ldmVudG9zLWJy…"}

GET mesma consulta + next_cursor -> HTTP/1.1 200 OK
{"events":[{…"seq":3,…,"jurisdiction":"BR"}],"next_cursor":null}

GET mesmo cursor com partition=2 -> HTTP/1.1 400 Bad Request
{"ack_class":"transport_accepted","detail":"cursor é de outra partição","error":"cursor_incomparable"}

GET cursor=lixo -> HTTP/1.1 400 Bad Request
{"ack_class":"transport_accepted","detail":"cursor ilegível","error":"cursor_invalid"}

GET cursor= (vazio) -> HTTP/1.1 400 Bad Request
{"ack_class":"transport_accepted","detail":"cursor ilegível","error":"cursor_invalid"}
```

Estado persistido depois da série (a duplicata e o salto não criaram linha):

```
 origin_node |     tenant_id     |   stream_id    | partition | authority_epoch | seq | retention_class | jurisdiction
-------------+-------------------+----------------+-----------+-----------------+-----+-----------------+--------------
 node-wire   | tenant-eventos-br | wire-325762181 |         1 |               7 |   1 | mensagem        | BR
 node-wire   | tenant-eventos-br | wire-325762181 |         1 |               7 |   2 | mensagem        | BR
 node-wire   | tenant-eventos-br | wire-325762181 |         1 |               7 |   3 | mensagem        | BR
(3 rows)
```

## 7. Limitações declaradas (nada disto foi implementado nem alegado)

1. **Sem identidade nem digest de evento.** Esta fatia grava posição + classe; `event_id`,
   hash de conteúdo e payload não existem em `event_position`. Logo a repetição idempotente é
   da **tupla de posição**, e um replay da mesma tupla com `retention_class` diferente
   responde `200` com a classe **já persistida** — conflito de conteúdo
   (`EVENT_IDENTITY_CONFLICT`, ainda rótulo de proposta) não é detectável sem essas colunas.
   Registro como pendência, não como garantia.
2. **Cursor não é expirado por tempo.** Não há TTL aprovado, então nenhum prazo foi
   inventado. Cursor ilegível, de versão desconhecida, sem partição ou de outro principal cai
   em `cursor_invalid`; expiração por intervalo retido (410 `CURSOR_EXPIRED` + snapshot)
   continua pendente de política e de SPEC.
3. **Cursor não é autenticado por assinatura.** A vinculação é por campos no token; a
   autorização é do grant e todo filtro inclui `tenant_id` do grant. Um token adulterado não
   amplia escopo, mas a garantia criptográfica do contrato segue pendente de decisão de
   identidade/chave.
4. **Sem streaming.** O contrato propõe `GET /v1/events/stream`; aqui só existe leitura
   paginada. Não há push, WebSocket nem long-poll.
5. **Sem materialização/projeção/outbox de evento.** O ACK para em `persisted_local`; nada
   replica, materializa ou entrega.
6. **`limit` máximo 100 e `limit` obrigatório** são parâmetros desta fatia, não capacidade
   publicada.
7. O `RetentionPolicyMissing` compartilha código entre incidente e evento, e a mensagem de
   detalhe passou a ser genérica ("classe de retenção sem política aplicável à jurisdição do
   tenant"). Nenhum teste dependia do texto anterior.
8. `cargo fmt` **não** foi aplicado ao repositório: o código existente não é rustfmt-clean e
   reformatá-lo misturaria ruído de estilo com esta entrega. Só os blocos acrescentados estão
   em estilo rustfmt.
