# Evidência — correção de estrutura/lint em `godmesh-server/src/main.rs` (18/11/2026)

**Worker:** F1 (estrutura e lint do servidor), sob o CTO.
**Branch:** `room/f1-corrige-lint-e-estrutura-do-main-rs-u75j`.
**Base com o defeito:** `798d662` (integração das fatias de auth e retenção no servidor).

## 1. Defeito

`cargo clippy --workspace --all-targets` falhava em `main`:

```
error: items after a test module
   --> crates/godmesh-server/src/main.rs:109:1
    |
109 | mod tests {
    | ^^^^^^^^^
...
161 | fn main() -> ExitCode {
    | ^^^^^^^^^^^^^^^^^^^^^
...
191 | async fn run(config: Config) -> Result<(), String> {
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
...
225 | async fn shutdown_signal() {
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.99.0/index.html#items_after_test_module
    = note: `-D clippy::items-after-test-module` implied by `-D clippy::all`
    = help: to override `-D clippy::all` add `#[allow(clippy::items-after-test-module)]`
    = help: move the items to before the test module was defined

error: could not compile `godmesh-server` (bin "godmesh-server" test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
```

**Causa:** efeito do merge de `798d662`. Duas versões de `main.rs` cresceram em
paralelo (uma acrescentou `regex`/grants de auth e `run`/`shutdown_signal`; a
outra trouxe o módulo de testes de `resolve_bind_addr`). O merge ficou correto em
conteúdo, mas o `mod tests` acabou posicionado **antes** de `fn main`, `run` e
`shutdown_signal`. `clippy::items_after_test_module` (negado por `-D clippy::all`
via `-D warnings` no workspace) reprova essa ordem.

Não era falha de lógica, contrato ou teste: nenhum comportamento estava errado.

## 2. Correção

Movido **um único bloco** (`#[cfg(test)] mod tests { .. }`) de sua posição
intermediária para o fim do arquivo, após `shutdown_signal`. Adicionado um
comentário de duas linhas explicando a restrição do lint, para evitar regressão
na próxima vez que o arquivo crescer.

O que **não** foi feito:

- Nenhum `#[allow(...)]` para silenciar o lint (regra do worker).
- Nenhuma linha de lógica de negócio reordenada: `Config`, `Config::from_env`,
  `resolve_bind_addr`, `fn main`, `fn run`, `fn shutdown_signal` mantêm corpo,
  ordem relativa e assinatura idênticos.
- Nenhum teste removido, renomeado ou enfraquecido.

O diff é um movimento puro (55 inserções / 53 remoções = bloco movido + 2 linhas
de comentário), como mostrado em `git diff`.

### Blocos `#[cfg(test)]`

Havia exatamente **um** bloco `#[cfg(test)]` em `main.rs` (linha 108), portanto
não houve consolidação a fazer. O único outro do crate está em
`crates/godmesh-server/src/http.rs:172` e é um módulo de testes legítimo, com
itens de produção antes dele — não afetado.

### Testes redundantes?

Não identifiquei redundância, portanto nada foi removido. Os 6 testes cobrem
caminhos distintos de `resolve_bind_addr`:

| Teste | Caminho coberto |
|---|---|
| `bind_addr_ausente_em_producao_falha_citando_a_variavel` | `None` + `production` → erro, mensagem cita `BIND_ADDR` |
| `bind_addr_ausente_sem_godmesh_env_e_tratada_como_producao` | `None` + `None` → fail-closed |
| `bind_addr_vazia_em_producao_falha` | `Some("   ")` (preenchida mas em branco) → erro |
| `bind_addr_desconhecida_falha_fechada` | `"staging"` → fail-closed |
| `bind_addr_default_loopback_so_em_dev` | `dev`, `development`, `DEV` → default loopback |
| `bind_addr_explicita_vale_em_qualquer_ambiente` | valor explícito vence em produção |

O caso "vazia" e o caso "ausente" parecem próximos, mas exercitam ramos
diferentes (variável definida em branco vs. `VarError::NotPresent`) e ambos
protegem falha de configuração em produção. Mantidos.

## 3. Verificação

### `cargo clippy --workspace --all-targets`

**Antes** (literal, recorte final):

```
error: items after a test module
   --> crates/godmesh-server/src/main.rs:109:1
error: could not compile `godmesh-server` (bin "godmesh-server" test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
```

**Depois** (saída completa, literal):

```
    Checking godmesh-server v0.1.0 (/Users/.../GodMesh/crates/godmesh-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

0 erro, 0 aviso.

### `cargo test --workspace`

**Antes** — 112 aprovados, 0 falhas (o lint é do clippy, não do rustc, então a
suíte já passava; o defeito só quebrava o gate de lint):

```
passed=112 failed=0
```

**Depois**:

```
passed=112 failed=0
```

Também 1 teste `ignored` (`tests/retention_policy_missing.rs`), já ignorado antes
da mudança, sem relação com este defeito.

Detalhe do alvo do binário, que é o afetado:

```
     Running unittests src/main.rs (target/debug/deps/godmesh_server-fca64c1fb2f5b40b)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

6 testes antes (`#[test]` no arquivo) e 6 depois. Nada perdido.

### Formatação

`cargo fmt --check` no workspace **já reporta desvio pré-existente** (144 diffs em
HEAD, sem nenhuma mudança minha — crates como `godmesh-protocol`,
`godmesh-sync` e `godmesh-server/tests/contract.rs` estão fora do rustfmt).
`crates/godmesh-server/src/main.rs` não aparece nessa lista: o arquivo continua
formatado sob a edition 2024. Não reformatei o workspace — está fora do escopo
desta tarefa e tocaria código de outros workers.

Prova de que o diff é movimento puro: comparando linhas removidas e adicionadas
ordenadas, a única diferença são as 2 linhas de comentário novas
(53 removidas / 55 adicionadas).

## 4. Limitações

- Verificação feita em ambiente local (macOS), sem execução de migrations,
  Postgres real ou testes de integração que dependem de banco (`ignored`).
- Nenhuma dependência adicionada, atualizada ou removida: não houve consulta de
  versão necessária para esta mudança (edição textual de um arquivo Rust).
- O defeito é de **gate de lint**, não de comportamento; a correção não altera
  contrato de API, formato persistido ou mensagens de erro.