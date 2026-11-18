# Evidência 2026-11-18 — A1: contrato de API para máquinas (`GET /v1/meta`, OpenAPI 3.1, erros acionáveis)

**Escopo desta fatia:** ADR-074 itens (1) e (2) para a superfície HTTP já
implementada — contrato legível por máquina, semântica segura para agente
(validação de `Idempotency-Key` e `dry_run` em rotas destrutivas). A
implementação completa de idempotência (deduplicação de efeito por chave) é a
próxima fatia e está **declarada como pendente no próprio contrato**.

Ambiente: macOS arm64, `rustc 1.99.1`, `cargo 1.99.1`, Postgres 16.15 em
cluster temporário local (porta 55499, **sem Docker**, socket `/tmp`),
binário real `godmesh-server` escutando em `127.0.0.1:18080` com
`GODMESH_OUTBOX=on`, federação desligada (sem `GODMESH_FEDERATION_BIND`).

## 1. Dependências novas (registradas conforme AGENTS.md)

| crate | versão | fonte | data | justificativa |
|---|---|---|---|---|
| `utoipa` | 6.0.0 | crates.io (índice sparse `index.crates.io`, entrada `vers":"6.0.0"`, pubtime 2026-09-22) | 2026-11-18 | modelo de dados OpenAPI 3.1 + derive `ToSchema` dos tipos serde reais; `rust_version` 1.88 ≤ toolchain 1.99 |

**`utoipa-axum 0.3.0` — NÃO adicionado (reporte obrigatório da tarefa).**
Verificação no índice crates.io: `utoipa-axum 0.3.0` declara `axum ^0.8.4` e
`utoipa ^6.0.0` — compatível em versões com `axum 0.8.9`. O que falha não é a
versão, é o **encaixe estrutural**: seu `Api<...>` exige um router único por
`Api` com macros `#[utoipa::path]` por handler, e esta base compõe portões de
escopo por rota (`route_layer` sobre grupos) a partir de um registro próprio.
Optou-se pelo extrator próprio previsto na tarefa: o documento OpenAPI é
gerado do **mesmo `Registry` que constrói o router**
(`crates/godmesh-server/src/http/openapi.rs`), e os schemas vêm do derive
`ToSchema` das mesmas structs serde que os handlers serializam (inclusive
`SignedManifest`/`PeerRecord`/`LinkSnapshot` em `godmesh-federation`, com o
derive habilitado por dependência `utoipa` naquele crate). A verdade do
documento é garantida por teste de contrato, não por convenção.

## 2. Arquitetura: uma fonte, três leituras

- `src/http/contract.rs` — `Registry`: registrar uma rota **é** documentá-la
  (a assinatura exige escopo, semântica `mutating/destructive` e o `RouteDoc`
  com request/response/erros/parâmetros). O router público/protegido sai
  daqui; `.route()` não existe em nenhum outro módulo de domínio.
- `GET /v1/meta` (público, sem credencial, sem dado de tenant) →
  `api_version`, `supported_versions` (N/N-1, ADR-056: um único perfil
  publicado hoje → `["v1"]`, com a política explícita em
  `compatibility_policy`), `routes[]` derivado do registro, `errors[]` do
  catálogo, `capabilities` do estado efetivo do processo.
- `GET /openapi.json` → documento 3.1 serializado uma vez na composição, do
  mesmo registro.
- Catálogo de erros: `(status, código, detail)` por `match` **exaustivo**
  (`error_parts`) e `(when, retryable, hint)` por `match` **exaustivo**
  (`advice`) — variante nova de `ApiError` sem catálogo não compila.

## 3. `GET /v1/meta` efetivo (servidor real)

```console
$ curl -s http://127.0.0.1:18080/v1/meta | python3 -m json.tool | head -60
{
    "api_version": "v1",
    "capabilities": {
        "federation_enabled": false,
        "host_metrics_available": true,
        "outbox_enabled": true,
        "retention_classes": [
            "auditoria", "crowdfind", "incidente_sos", "mensagem",
            "posicao", "telemetria", "transcricao_asr"
        ]
    },
    "compatibility_policy": "N/N-1 (ADR-056)",
    "docs_url": "/openapi.json",
    "errors": [
        {
            "error": "credential_expired",
            "hint": "emita um novo grant em POST /v1/admin/grants; o token atual não será revalidado",
            "http_status": 401,
            "retryable": false,
            "when": "o grant tem expires_at no passado"
        },
        ...
```

Saída do comparador documento × router × meta (contra o binário real):

```console
api_version: v1 | supported: ['v1'] | policy: N/N-1 (ADR-056)
routes: 24 | errors: 31
rotas destrutivas: ['POST /v1/incidents/{id}/close',
 'POST /v1/admin/grants/{token_id}/revoke',
 'POST /v1/federation/join-requests/{join_request_id}/approve',
 'POST /v1/federation/join-requests/{join_request_id}/reject']
openapi (método,caminho): 24 | meta: 24 | iguais: True
securitySchemes: ['bearer']
schemas registrados: 33
```

Contas: 24 rotas = 7 incidente + 2 evento + 2 sync + 3 admin + 6 federação +
`/metrics` + `/healthz` + `/v1/meta` + `/openapi.json`. Cada rota protegida
sondada sem credencial devolve `401` **no portão** (prova de que existe e de
que nada roda sem credencial); `GET /v1/rota-que-ninguem-registrou` → `404`.

## 4. Erros acionáveis (corpo real, sem mudar códigos existentes)

```console
$ curl -s http://127.0.0.1:18080/v1/incidents
{"ack_class":"transport_accepted","detail":"credencial ausente ou desconhecida",
 "docs_url":"/v1/meta","error":"unauthenticated",
 "hint":"autentique com Authorization: Bearer <token>; token é emitido por POST /v1/admin/grants com escopo admin:grants",
 "retryable":false}
```

`error`, `detail`, `ack_class` e os extras (`state`, `expected`, …) ficam
intactos; `hint`, `docs_url` e `retryable` são acrescentados pelo catálogo.

## 5. `dry_run` em rota destrutiva — prova de "sem escrever nada"

Ciclo legal até `acknowledged_human`, depois:

```console
$ curl -s -X POST -H "$T" "$B/v1/incidents/a1-evidence-close2/close?dry_run=true"
{"applied":false,"dry_run":true,"would_do":["POST /v1/incidents/a1-evidence-close2/close"]}

sos_ack antes: 4 | state: acknowledged_human
sos_ack depois dry_run: 4 | state: acknowledged_human      # nada escrito
close real: 200
state: closed                                              # a rota continua viva
```

Revogação de grant:

```console
grant_revocation antes: 0
$ curl -s -X POST ... "/v1/admin/grants/gtr_cc9f.../revoke?dry_run=true"
{"applied":false,"dry_run":true,"would_do":["POST /v1/admin/grants/gtr_cc9f.../revoke"]}
grant_revocation depois dry_run: 0                          # 200 sem linha no banco
```

Valores fora de `true|false` são recusados (sem sinonimia silenciosa);
`dry_run=true` em mutante **não** destrutiva → `422` declarado, não ignorado.

## 6. `Idempotency-Key` — declarar e validar (esta fatia)

```console
$ curl -s -X POST -H "$T" -H "Idempotency-Key: aaaa…(129)" ... 
{"detail":"Idempotency-Key deve ter 1 a 128 bytes ASCII imprimíveis (0x21..=0x7E)",
 "docs_url":"/v1/meta","error":"invalid_request",
 "hint":"corrija o pedido conforme detail; contratos aceitos estão em GET /v1/meta",
 "retryable":false}
```

- Ausente: continua aceito (compatibilidade; nenhuma suíte existente quebra).
- Válido (≤128, ASCII imprimível): aceito; a **deduplicação do efeito** é a
  próxima fatia e está escrita na descrição de cada rota mutante no openapi.
- charset/length violados: `422` **antes do handler** (inclusive na rota
  pública de ingresso, que roda sem portão de credencial).

## 7. Testes de contrato (`crates/godmesh-server/tests/api_contract.rs`)

```
running 8 tests
test rota_fora_do_registro_nao_compila ... ok
test corpo_de_erro_real_e_acionavel_e_estavel ... ok
test documento_declara_bearer_e_ref_resolvem ... ok
test capacidades_do_meta_refletem_o_processo ... ok
test idempotency_key_fronteira_e_dry_run_em_mutante ... ok
test dry_run_em_rota_destrutiva_nao_escreve_nada ... ok
test catalogo_de_erros_cobre_todo_codigo_documentado ... ok
test contrato_publicado_e_o_mesmo_registro_do_router ... ok
test result: ok. 8 passed; 0 failed
```

Os quatro primeiros rodam **sem banco** (pool preguiçoso para rotas que o
portão bloqueia antes de qualquer query); os demais usam banco descartável
migrado pelo runner oficial, com prova por contagem SQL antes/depois.

**Sabotagens executadas (o teste precisa falhar, e falhou):**
1. rota registrada no `Registry` sem mudar o contrato → `contracto_publicado…`
   falha por contagem (`25 ≠ 24: mudou a API, atualize contrato **e** contagem
   deliberadamente`); revertido.
2. `Router::new().route(...)` manual num módulo de domínio →
   `rota_fora_do_registro_nao_compila` falha apontando arquivo e linha;
   revertido.

## 8. Gates

```
$ DATABASE_URL=… cargo test --workspace
TOTAL: 318 passed, 0 failed, 1 ignored (1 ignored pré-existente) em 45 binários de teste

$ cargo clippy --workspace --all-targets        # sem warnings, sem erros

$ PGHOST=/tmp PGPORT=55499 PGUSER=godmesh migrations/tests/run_all.sh
RESULTADO: OK — 44 invariantes verificados em banco criado do zero. (exit 0)
```

## 9. Arquivos alterados

- novos: `src/http/contract.rs`, `src/http/openapi.rs`, `tests/api_contract.rs`;
- reescrita de composição: `src/http/mod.rs` (portões, `router_runtime`, corpo
  de erro com `hint/docs_url/retryable`), `src/http/{incidents,events,sync,admin,federation}.rs`
  (rotas viram registros no `Registry`), `src/main.rs` (usa `router_runtime`
  com estado efetivo do publicador);
- derives `ToSchema`: tipos de request/response em `api/lifecycle/incidents/events/inbox/admin/federation/auth` (server) e `manifest/peer/link` (federação);
- `Cargo.toml` (server + federação) e `Cargo.lock`: `utoipa 6.0.0`.

## 10. Limitações e pendências honestas

- Idempotência plena (armazenar chave → devolver o mesmo efeito) **não**
  implementada — declarada no contrato como próxima fatia.
- `limits.rate` é `null` em `routes[]`: não existe limitador implementado; o
  contrato publica o estado efetivo, não o desejado. `max_body_bytes` reflete
  o `DefaultBodyLimit` real do axum (2 MiB, axum-core 0.5.6, verificado no
  código vendado do registry em 2026-11-18).
- `capabilities.retention_classes` vem de `SELECT DISTINCT class` em
  `retention_class`; sem banco acessível vira `null` (piso `unknown`, ADR-072),
  nunca lista inventada.
- `supported_versions` hoje é `["v1"]`: N-1 só pode ser anunciado quando
  existir um perfil anterior publicado (ADR-056 pendura janela numérica ao
  produto).
- Escopo por operação no OpenAPI: esquema `http;bearer` não tem campo de
  escopo na 3.1; o escopo normatizado vive em `GET /v1/meta → routes[].scope`
  e é citado na descrição de cada operação (fingir OAuth2 seria contrato
  falso).
- O `dry_run` desta fatia curta-circuita **antes** do handler: `would_do`
  nomeia a ação (`"POST <caminho>"`) e prova ausência de efeito; a validação
  de estado legal (ex.: transição jurídica do `close`) antes de responder
  `would_do` é refinamento da próxima fatia.
