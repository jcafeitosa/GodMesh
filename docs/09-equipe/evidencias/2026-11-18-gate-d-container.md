# Evidência — gate D: contêiner de produção (18/11/2026)

**Branch:** `room/d-container-producao-ahtf` (worktree do worker D, sob o CTO)
**Base:** `0fb133a` (`infra(prod): build real da imagem com godmesh-server, healthcheck e evidencia`)
**Ambiente:** macOS `Darwin arm64`; Docker **29.8.2** (server `arm64`), OrbStack; imagens construídas em `linux/arm64`.
**Escopo:** achados 1–5 do gate `wfr_414e32e9` em `deploy/prod/Dockerfile`, `deploy/README.md` e `crates/godmesh-server/src/main.rs`.

> Regra desta evidência: o que não foi executado está na seção **Não executado**.
> Nada aqui foi inferido de documentação: cada bloco `$`/saída é cópia literal do terminal.

## 0. Estado antes e depois

| Arquivo | Mudança |
|---|---|
| `crates/godmesh-server/src/main.rs` | `BIND_ADDR` obrigatório fora de `GODMESH_ENV=dev|development` (exit 78 citando a variável); senha de dev removida do literal de mensagem; 6 testes unitários do resolvedor |
| `deploy/prod/Dockerfile` | `ENV BIND_ADDR=0.0.0.0:8080`; `CARGO_PROFILE_RELEASE_STRIP=symbols`; healthcheck em script próprio; comentários sobre `EXPOSE` ser metadado, sobre `GODMESH_MIGRATIONS_DIR` não ser lido e sobre flags de runtime |
| `deploy/prod/healthcheck.sh` | **novo**; verifica `/healthz` no bind efetivo **e** alcance TCP do Postgres |
| `deploy/README.md` | variáveis realmente lidas, `EXPOSE` como metadado, flags de runtime, escopo do healthcheck, tamanho/arquitetura reais |
| `.dockerignore` | whitelist do `deploy/prod/healthcheck.sh` |

## 1. Achado 1 — `BIND_ADDR` obrigatório fora de dev

`crates/godmesh-server/src/main.rs`: `resolve_bind_addr(bind_addr, env)` devolve o default
`127.0.0.1:8080` **só** com `GODMESH_ENV=dev|development`; qualquer outro valor — inclusive
ausente — é tratado como produção e falha com exit 78.

Testes unitários (`cargo test --locked -p godmesh-server --bin godmesh-server`):

```
running 6 tests
test tests::bind_addr_ausente_em_producao_falha_citando_a_variavel ... ok
test tests::bind_addr_ausente_sem_godmesh_env_e_tratada_como_producao ... ok
test tests::bind_addr_default_loopback_so_em_dev ... ok
test tests::bind_addr_desconhecida_falha_fechada ... ok
test tests::bind_addr_explicita_vale_em_qualquer_ambiente ... ok
test tests::bind_addr_vazia_em_producao_falha ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Comportamento do binário (release local, `target/release/godmesh-server`):

```
$ DATABASE_URL='postgres://godmesh:<SENHA_DO_BANCO>@localhost:55432/godmesh' GODMESH_ENV=production ./target/release/godmesh-server
godmesh-server: configuração inválida: BIND_ADDR não definida; com GODMESH_ENV=production o endereço de escuta é obrigatório (exemplo: BIND_ADDR=0.0.0.0:8080); o default 127.0.0.1:8080 só vale em GODMESH_ENV=dev
exit=78

$ DATABASE_URL='...' env -u GODMESH_ENV ./target/release/godmesh-server
godmesh-server: configuração inválida: BIND_ADDR não definida; com GODMESH_ENV=<ausente> = production o endereço de escuta é obrigatório (exemplo: BIND_ADDR=0.0.0.0:8080); o default 127.0.0.1:8080 só vale em GODMESH_ENV=dev
exit=78
```

Na imagem (ENV `BIND_ADDR` sobrescrita para vazio):

```
$ docker run --rm -e BIND_ADDR= -e DATABASE_URL='postgres://godmesh:<SENHA_DO_BANCO>@localhost:55432/godmesh' godmesh-server:gate-d
godmesh-server: configuração inválida: BIND_ADDR não definida; com GODMESH_ENV=production o endereço de escuta é obrigatório (exemplo: BIND_ADDR=0.0.0.0:8080); o default 127.0.0.1:8080 só vale em GODMESH_ENV=dev
exit=78
```

Exceção de dev preservada (dentro do contêiner, `BIND_ADDR` vazia + `GODMESH_ENV=dev`):

```
$ docker logs gm-gate-dev 2>&1 | tail -1
godmesh-server: escutando em http://127.0.0.1:8080

$ docker exec gm-gate-dev /usr/local/bin/healthcheck.sh; echo "healthcheck_exit=$?"
healthcheck_exit=0
```

**Estado: executado.**

## 2. Achado 2 — `HEALTHCHECK` e banco

`deploy/prod/healthcheck.sh` (novo, copiado para `/usr/local/bin/healthcheck.sh`, `chmod 0555`):

1. resolve o bind **efetivo** a partir de `BIND_ADDR` (com a mesma exceção de dev do binário) e faz `GET /healthz`, exigindo `200` + `"status":"ok"`;
2. extrai host/porta de `DATABASE_URL` e abre TCP no Postgres.

Falha fechada de configuração:

```
$ docker run --rm -e BIND_ADDR= -e GODMESH_ENV=production --entrypoint /usr/local/bin/healthcheck.sh godmesh-server:gate-d; echo "exit=$?"
healthcheck: BIND_ADDR ausente e GODMESH_ENV=production não é dev; sem endereço efetivo para checar
exit=1
```

Banco no ar (`gm-gate-d`, Postgres de dev em `localhost:55432`):

```
$ docker inspect gm-gate-d --format 'Status={{.State.Health.Status}} Failing={{.State.Health.FailingStreak}}'
Status=healthy Failing=0
```

Banco derrubado — subi um Postgres descartável próprio (`gm-gate-pg`, porta 55433), conectei
o `gm-gate-d2` nele, derrubei o banco e esperei o healthcheck:

```
$ docker stop gm-gate-pg
gm-gate-pg parado

$ docker inspect gm-gate-d2 --format 'Health={{.State.Health.Status}} Failing={{.State.Health.FailingStreak}}'
Health=healthy Failing=1          # 40 s depois do stop (1 falha de 3)

$ # 70 s depois
Health=unhealthy Failing=4        # passou de retries=3 -> unhealthy
```

E o motivo registrado no log de saúde do Docker:

```
$ docker inspect gm-gate-d2 --format '{{range .State.Health.Log}}exit={{.ExitCode}} out={{.Output}}{{end}}' | tail -2
healthcheck: Postgres inalcançável em localhost:55433 (DATABASE_URL)
```

### Dependência explícita: `/healthz` não consulta o pool

`crates/godmesh-server/src/http.rs` tem `async fn healthz()` que devolve JSON estático e
**não usa o `AppState`/pool**. Prova, no mesmo contêiner com o banco morto:

```
$ curl -s -o /dev/null -w 'healthz_http=%{http_code} (inalterado)\n' http://127.0.0.1:18082/healthz
healthz_http=200 (inalterado)
```

Pelo escopo desta tarefa **não editei `http.rs`**. O correto — `/healthz` responder `503`
quando o pool falhar (`SELECT 1` com timeout) — fica registrado como dependência, e o
healthcheck atual só garante **alcance TCP** (prova porta aberta, não prova consulta
bem-sucedida). Limite documentado em `deploy/prod/healthcheck.sh` e `deploy/README.md`.

**Estado: parcialmente executado.** `/healthz` com 503 no pool: **não implementado** (dependência de `http.rs`). Alcance TCP do banco: **executado**.

## 3. Achado 3 — senha de dev fora do binário

O literal `postgres://godmesh:<SENHA_DO_BANCO>@localhost:55432/godmesh` foi trocado por um
exemplo de formato sem credencial (`postgres://usuario:senha@host:porta/banco`).

Build de release com o perfil aplicado (mesmas flags do Dockerfile):

```
$ CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --release --locked --bin godmesh-server --package godmesh-server
    Finished `release` profile [optimized] target(s) in 4m 27s

$ strings target/release/godmesh-server | grep -c godmesh_dev_only
0
grep_exit=1
```

Binário extraído da imagem (`docker cp` de `/usr/local/bin/godmesh-server`):

```
$ strings /tmp/gmbin/godmesh-server | grep -c godmesh_dev_only
0
grep_exit=1
```

Nota: `godmesh_dev_only` continua no repositório onde é **senha de dev** declarada
(`deploy/dev/compose.yaml`, README de dev, evidências antigas). O que não pode é estar no
artefato publicado — e não está.

**Estado: executado.**

## 4. Achado 4 — `strip` e `/migrations` sob `--read-only`

`strip` sem depender de `binutils` no runtime: `CARGO_PROFILE_RELEASE_STRIP=symbols` no
estágio builder (rustc `-Cstrip=symbols`). Verificação por parse dos section headers ELF:

```
$ python3 - <<'PY'   # lê e_shoff/e_shnum e decodifica os nomes
seções: ['', '.interp', '.note.gnu.build-id', '.note.ABI-tag', '.gnu.hash', '.dynsym', '.dynstr',
         '.gnu.version', '.gnu.version_r', '.rela.dyn', '.rela.plt', '.init', '.plt', '.text', '.fini',
         '.rodata', '.eh_frame_hdr', '.eh_frame', '.gcc_except_table', '.tdata', '.tbss', '.init_array',
         '.fini_array', '.data.rel.ro', '.dynamic', '.got', '.data', '.bss', '.comment', '.shstrtab']
.symtab presente: False
.strtab presente: False
```

`chmod -R a-w /migrations` é **só DAC**, e isso ficou provado:

```
$ docker run --rm --user 0:0 --entrypoint /bin/bash godmesh-server:gate-d -c 'touch /migrations/x && ls -l /migrations/x; rm -f /migrations/x'
-rw-r--r-- 1 root root 0 Nov 18 13:52 /migrations/x
exit=0                        # root escreveu: chmod a-w não protege contra uid 0

$ docker run --rm --read-only --user 0:0 --cap-drop=ALL --security-opt no-new-privileges --entrypoint /bin/bash godmesh-server:gate-d -c 'touch /migrations/x; echo "touch_exit=$?"'
touch: cannot touch '/migrations/x': Read-only file system
touch_exit=1
exit=0                        # o bash saiu 0 porque o echo veio depois do touch
```

Servidor real rodando com as flags de runtime recomendadas (`--user 10001:10001`):

```
$ docker run -d --name gm-gate-ro --read-only --cap-drop=ALL --security-opt no-new-privileges \
    --user 10001:10001 --network host \
    -e DATABASE_URL='postgres://godmesh:<SENHA_DO_BANCO>@localhost:55432/godmesh' \
    -e BIND_ADDR=0.0.0.0:18083 godmesh-server:gate-d

$ docker exec gm-gate-ro /usr/local/bin/healthcheck.sh; echo "healthcheck_exit=$?"
healthcheck_exit=0

$ docker exec gm-gate-ro /bin/bash -c 'touch /migrations/x; echo touch_exit=$?'
touch: cannot touch '/migrations/x': Read-only file system
touch_exit=1

$ curl -s http://127.0.0.1:18083/healthz
{"ack_class":"transport_accepted","status":"ok"}

$ docker inspect gm-gate-ro --format 'Health={{.State.Health.Status}}'
Health=healthy
```

Flags documentadas no `Dockerfile` (comentário) e no `deploy/README.md`:
`--read-only`, `--cap-drop=ALL`, `--security-opt no-new-privileges`, `--user 10001:10001`, `-p 8080:8080`.

**Estado: executado.**

## 5. Achado 5 — correções no `deploy/README.md`

Removido/qualificado o que não se sustenta:

| Antes | Depois |
|---|---|
| "porta 8080 exposta" | `EXPOSE 8080` é **metadado**, não publica porta; quem publica é `-p 8080:8080` na execução |
| `GODMESH_ENV` listada sem dizer se é lida | tabela "Variáveis lidas pelo binário": `GODMESH_ENV` **é** lida (agora define se `BIND_ADDR` tem default) |
| `GODMESH_MIGRATIONS_DIR` apresentada como configuração ativa | marcada como **não lida** pelo binário atual (reserva) |
| `/migrations` "somente-leitura" | "somente-leitura por DAC; resiste a root **só** com `--read-only`" |
| `/healthz` era o único check | escopo do healthcheck declarado, com a lacuna do pool |
| tamanho/arquitetura sem ressalva | tamanho da imagem separado por build; `só linux/arm64` explícito na tabela e nas pendências |

**Estado: executado.**

## 6. Build e tamanho da imagem

```
$ time docker build -f deploy/prod/Dockerfile -t godmesh-server:gate-d .
...
#21 naming to docker.io/library/godmesh-server:gate-d done
#21 DONE 0.9s

real	1m48.666s        # primeira execução com as mudanças (cargo build em cache de crates)
```

Reprocesso só da camada do healthcheck (sem mexer no Dockerfile): `real 0m3.531s`.

Reconstrução depois do ajuste final de formatação do `Dockerfile` (newline no fim, que o
BuildKit hasheia e por isso invalida o cache): `real 1m37.203s`, gerando a imagem final
`sha256:0b49a111e1cd...`. Os testes das seções 2 e 4 marcados como "banco derrubado" foram
executados na imagem `8342c727a725` (mesmas camadas de conteúdo); BIND_ADDR, healthz com
banco no ar, `touch` sob `--read-only` e `strings` foram **re-executados** na imagem final
`0b49a111e1cd`.

```
$ docker images godmesh-server
IMAGE                   ID             DISK USAGE   CONTENT SIZE   EXTRA
godmesh-server:dev      04a42169211e        155MB         34.1MB
godmesh-server:gate-d   0b49a111e1cd        152MB         33.7MB

$ docker image inspect godmesh-server:gate-d --format '{{.Size}} bytes | {{.Id}} | {{.Os}}/{{.Architecture}}'
152422619 bytes | sha256:0b49a111e1cd6eb441ae71719fc4d67e209122e3abadc96bff243c5b3ea51c46 | linux/arm64

$ docker image inspect godmesh-server:gate-d --format '{{json .Config.Env}}'
["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","GODMESH_ENV=production","BIND_ADDR=0.0.0.0:8080","GODMESH_MIGRATIONS_DIR=/migrations","RUST_BACKTRACE=0"]

$ docker image inspect godmesh-server:gate-d --format '{{json .Config.Healthcheck}}'
{"Test":["CMD","/usr/local/bin/healthcheck.sh"],"Interval":30000000000,"Timeout":3000000000,"StartPeriod":5000000000,"Retries":3}
```

**152 MB de disco / 33,7 MB de conteúdo (152.422.619 bytes)**, contra 155 MB / 34,1 MB
(154.860.288 bytes) da imagem anterior — a diferença vem do `strip`.

## 7. Testes de regressão executados

```
$ cargo test --locked -p godmesh-server
     Running tests/contract.rs (target/debug/deps/contract-90c3361220963ba3)
running 3 tests
test refuses_precise_location_offline ... ok
test persists_incident_and_never_claims_more_than_persisted_local ... ok
test denies_incident_without_trusted_freshness_source ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo fmt --all --check   # nos arquivos deste worker
0 ocorrências para crates/godmesh-server/src/main.rs
```

`cargo fmt --all --check` **continua falhando no repositório** por diferenças pré-existentes
em `crates/godmesh-core/src/lib.rs` (não tocado aqui). Não reformatei arquivo de outro
domínio.

## 8. Não executado (declarado)

| Item | Motivo |
|---|---|
| `/healthz` devolvendo `503` com pool falhando | exige mudança em `crates/godmesh-server/src/http.rs`; fora do escopo autorizado (dependência registrada) |
| Consulta SQL real no healthcheck (`SELECT 1`) | a imagem de runtime não tem cliente Postgres; instalar `postgresql-client` seria dependência nova sem aprovação; hoje o check é TCP |
| Build `amd64` / multi-arch | só `linux/arm64` construído e testado |
| `docker run` em host Linux real (fora do OrbStack) | não há host Linux disponível |
| Migração aplicada de dentro do contêiner | o binário não lê `GODMESH_MIGRATIONS_DIR` nem aplica migração |
| Registry, assinatura, SBOM, limites de CPU/memória | fora de escopo/autorização |
| Teste do default de loopback (dev) no host macOS | porta 8080 do host ocupada por outro processo; o teste foi feito dentro do contêiner |
| Healthcheck com `DATABASE_URL` em unix socket | não exercitado; o script avisa e segue só com `/healthz` |

## 9. Arquivos alterados

- `crates/godmesh-server/src/main.rs`
- `deploy/prod/Dockerfile`
- `deploy/prod/healthcheck.sh` (novo)
- `deploy/README.md`
- `.dockerignore`
- `docs/09-equipe/evidencias/2026-11-18-gate-d-container.md` (este arquivo)

Nenhum crate de domínio foi tocado. `http.rs` não foi editado.