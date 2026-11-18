# Evidência — Portação de três melhorias do SEC1 concorrente (`1799e51`) — 18/11/2026

**Branch:** `room/sec1-d-porta-melhorias-de-cotas-r64h` (base `b3623d7`, o main com cotas/admissão
já integradas)
**Ambiente:** macOS arm64; PostgreSQL 18.x em `localhost:55432` (banco `godmesh`; nada de
DROP/TRUNCATE).
**O que foi portado — e só isto:** três pontos de `1799e51` (implementação concorrente sobre base
antiga) que a implementação integrada no main não tinha. A arquitetura de dois portões
(`http_gate` + `grant_gate`) foi preservada; nenhum limite/default testado mudou; nenhum teste foi
apagado.

## 1. Teto de corpo do axum alinhado ao teto configurado

**Problema.** O axum aplica `DefaultBodyLimit` de 2 MiB por conta própria aos extratores
(`Json`/`Bytes`/`String`/`Form`). O portão respeita o teto configurado (ex.: inbox 3 MiB), mas o
extrator recusava acima de 2 MiB com resposta **do framework** — no nosso caso, `422 invalid_request`
com `detail: "Failed to buffer the request body: length limit exceeded"` (observado em runtime, não
presumido), fora do contrato `{error, detail, hint, ack_class}`. Prova de não-falsificação: com a
camada fixada em 2 MiB, o teste novo falha exatamente nessa resposta; com o alinhamento, o corpo de
2,5 MiB chega ao handler e a recusa passa a ser de domínio (`409 retention_policy_missing`).

**Implementação.** `AdmissionConfig::payload_ceiling()` (maior teto configurado, `max(1)`);
`gate::extractor_body_limit` → `DefaultBodyLimit::max(ceiling)` aplicado como camada mais interna
do router (`http/mod.rs`). O portão continua sendo quem recusa por tamanho; o alinhamento apenas
impede que o teto silencioso do framework discorde do declarado. Defaults (64/64/256 KiB) são
menores que 2 MiB, nenhum comportamento existente muda.

**Testes.** `teto_acima_de_2_mib_e_respeitado_e_a_recusa_continua_no_formato_do_contrato`
(integração, `tests/limits.rs`: corpo 2,5 MiB dentro do teto de 3 MiB → handler; corpo declarado
acima de 3 MiB → 413 `payload_too_large` com `error`, `detail`, `hint`, `ack_class` presentes);
`teto_maior_configurado_alinha_o_extrator_do_axum` (unit, `admission.rs`).

## 2. Campo `effect: unknown` na recusa por timeout

O corpo de timeout agora declara `{"effect": "unknown", "timeout_ms": N}`: o futuro do handler é
cancelado no meio e **uma transação pode ter cometido** — a resposta não pode negar efeito. O
`hint` manda reenviar com a mesma chave de idempotência ("o replay resolve, não duplica"), que é a
única estratégia segura dado o efeito desconhecido. `ack_class` permanece `transport_accepted`
(piso honesto, ADR-058). Todas as recusas do portão ganharam `hint` acionável, o formato que a
ADR-074 já exigia de erros.

## 3. Status do timeout: **503**, com evidência RFC 9110

O main usava **504**; `1799e51` usava **408**. Nenhum dos dois resiste ao texto da RFC 9110
(https://www.rfc-editor.org/rfc/rfc9110.html, lido em 18/11/2026):

* **§15.6.5 (504):** *"The 504 (Gateway Timeout) status code indicates that the server, while
  acting as a gateway or proxy, did not receive a timely response from an upstream server it needed
  to access in order to complete the request."* — aqui o servidor **não** atua como
  gateway/proxy: o tempo estourou processando a própria requisição, sem upstream HTTP. 504 é
  indefensável. (O Postgres é dependência de infraestrutura, não "upstream server" no sentido da
  seção, que descreve encadeamento de HTTP.)
* **§15.5.9 (408):** *"The 408 (Request Timeout) status code indicates that the server did not
  receive a complete request message within the time that it was prepared to wait."* — a requisição
  chegou **inteira** (o portão mediu o corpo antes); o que faltou foi tempo de processamento. O
  408 da RFC 9110 descreve falha de *recebimento*, e sua nota de repetição ("MAY repeat that
  request") presupõe o caso em que nada chegou — pressuposto que um timeout de handler com
  transação possivelmente cometida viola.
* **§15.6.4 (503):** *"The 503 (Service Unavailable) status code indicates that the server is
  currently unable to handle the request due to a temporary overload or scheduled maintenance,
  which will likely be alleviated after some delay. The server MAY send a Retry-After header field
  (Section 10.2.3) to suggest an appropriate amount of time for the client to wait before retrying
  the request."* — é a única das três cuja definição descreve a situação: o servidor, sob carga que
  o fez estourar o teto declarado, está temporariamente incapaz de concluir *esta* requisição, com
  expectativa de alívio após um atraso — e a mesma seção autoriza `Retry-After`.

**Decisão implementada:** `503 request_timeout` com `Retry-After: 1` (1 s: o servidor não sabe
quanto o pedido lento levaria; é o mesmo degrau conservador da recusa por admissão) e o corpo do
item 2. Justificativa curta registrada em comentário no código (`gate::request_timeout`).

**Teste.** `handler_lento_alcanca_o_teto_e_recebe_503_request_timeout_com_effect_unknown`
(renomeado do antigo `…504…`, sem apagar cobertura): 503, `error == request_timeout`,
`effect == unknown`, `hint` citando idempotência, `Retry-After: 1`, `ack_class` intocado.

## Verificações (18/11/2026)

* `DATABASE_URL=…@localhost:55432/godmesh cargo test --workspace` — **331 passed, 0 failed, 1
  ignored** (os 329 anteriores + 2 novos; o ignored é preexistente).
* `cargo clippy --workspace --all-targets` — 0 avisos.
* `bash migrations/tests/run_all.sh` — OK, 44 invariantes em banco criado do zero.
* Não-falsificação do teste de 2 MiB verificada locally ao fixar a camada em 2 MiB (falha com o
  corpo de framework do axum) — ver item 1.

## Emenda à evidência anterior

`docs/09-equipe/evidencias/2026-11-18-sec-cotas-admissao.md` (e sua linha de tabela) registra
504 para o timeout como implementado à época. Este documento emenda aquele status: **503** a
partir daqui, com a leitura RFC acima. O mais (cotas, classes, tetos, ordem de portões) permanece
valido.
