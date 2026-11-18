# Desenvolvimento e produção

## Desenvolvimento — OrbStack

Só **dependências de infraestrutura** rodam em contêiner. Backend, frontend e apps rodam locais, como manda a diretriz do projeto.

```bash
docker compose -f deploy/dev/compose.yaml up -d
# Postgres em localhost:55432, banco godmesh, usuário godmesh
```

Migrações são aplicadas na inicialização do volume (`/docker-entrypoint-initdb.d`). Para reaplicar do zero:

```bash
docker compose -f deploy/dev/compose.yaml down -v
docker compose -f deploy/dev/compose.yaml up -d
```

Verificação dos invariantes no banco — comando único, contra banco criado do zero:

```bash
PGHOST=127.0.0.1 PGPORT=55432 PGUSER=godmesh PGPASSWORD=godmesh_dev_only \
  migrations/tests/run_all.sh
```

O script cria um banco descartável (`godmesh_inv_*`), aplica
`migrations/0001_core.sql` + `migrations/0002_incident_tenancy.sql`, roda
`migrations/tests/0001_invariants.sql` e `migrations/tests/0002_invariants.sql`
e falha com exit != 0 se qualquer bloco `NOTICE: OK` esperado faltar. Esperado:
`RESULTADO: OK — 14 invariantes verificados em banco criado do zero.`

O banco de desenvolvimento compartilhado não é tocado — nada de `DROP`/`TRUNCATE`
nele. Só o banco descartável do próprio script é removido ao final (`--keep`
preserva para inspeção). `--sabotage <arquivo.sql>` aplica SQL extra antes dos
testes, apenas para provar que a falha é detectada.

## Produção — contêiner

`deploy/prod/Dockerfile` é multistage:

- **builder**: `rust:1.100-slim-bookworm` (1.100.0, publicado em 12/11/2026) compila
  `cargo build --release --locked --bin godmesh-server --package godmesh-server`
  com `CARGO_PROFILE_RELEASE_STRIP=symbols` (binário sem `.symtab`/`.strtab`);
- **runtime**: `debian:trixie-slim` (Debian 13, publicado em 05/11/2026), só com
  `ca-certificates`, usuário `godmesh` (uid/gid 10001) sem home e sem shell,
  migrações em `/migrations` com `chmod a-w`, `EXPOSE 8080`, `HEALTHCHECK` em
  `/healthz` + alcance do Postgres, `ENTRYPOINT` do binário.

As bases são fixadas por **tag explícita + digest do índice multi-arch** nos `ARG`
`RUST_IMAGE` e `RUNTIME_IMAGE`. Sem `latest`. Build não anda sozinho quando a tag for
repontada.

### Variáveis lidas pelo binário

| Variável | Lida pelo binário? | Regra |
|---|---|---|
| `DATABASE_URL` | **sim** | obrigatória; ausente/vazia → exit 78 |
| `BIND_ADDR` | **sim** | obrigatória fora de `GODMESH_ENV=dev|development` (exit 78, mensagem cita a variável); em dev o default é `127.0.0.1:8080` |
| `GODMESH_ENV` | **sim** | ausente = `production` (falha fechada: exige `BIND_ADDR`) |
| `GODMESH_MIGRATIONS_DIR` | **não** | definida no Dockerfile como reserva; o binário atual não aplica migração. **Não** conte com ela |
| `RUST_BACKTRACE` | indireto | só afeta mensagem de panic; `0` na imagem |

`EXPOSE 8080` é **metadado**: não publica porta, não abre firewall e não substitui
`-p 8080:8080`. Quem publica porta é o operador, na execução.

Build:

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
```

Teste da imagem:

```bash
# 1. sem configuração: falha clara, exit 78 (EX_CONFIG)
docker run --rm godmesh-server:dev

# 2. produção sem BIND_ADDR: exit 78 citando a variável
docker run --rm -e BIND_ADDR= \
  -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  godmesh-server:dev

# 3. servidor real contra o Postgres de dev
docker run --rm --network host \
  -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
  -e BIND_ADDR=0.0.0.0:18081 godmesh-server:dev
curl -s http://127.0.0.1:18081/healthz   # {"ack_class":"transport_accepted","status":"ok"}
```

O binário **não** implementa `--help` nem subcomandos: qualquer argumento é ignorado.

### Execução recomendada (flags de runtime)

O `chmod -R a-w /migrations` é **só DAC** e não resiste a `--user 0:0`. Somente
`--read-only` fecha o rootfs:

```bash
docker run --read-only --cap-drop=ALL --security-opt no-new-privileges \
  --user 10001:10001 -p 8080:8080 \
  -e DATABASE_URL=postgres://USUARIO:SENHA@HOST:5432/godmesh \
  godmesh-server:TAG
```

Teste literal (imagem `godmesh-server:gate-d`, 18/11/2026):

```
$ docker run --rm --user 0:0 --entrypoint /bin/bash godmesh-server:gate-d \
    -c 'touch /migrations/x && ls -l /migrations/x; rm -f /migrations/x'
-rw-r--r-- 1 root root 0 Nov 18 13:52 /migrations/x      # root escreveu: chmod não basta

$ docker run --rm --read-only --user 0:0 --cap-drop=ALL \
    --security-opt no-new-privileges --entrypoint /bin/bash godmesh-server:gate-d \
    -c 'touch /migrations/x'
touch: cannot touch '/migrations/x': Read-only file system
```

### O que o `HEALTHCHECK` verifica — e o que não verifica

`deploy/prod/healthcheck.sh` faz duas coisas:

1. `GET /healthz` no **bind efetivo** (`BIND_ADDR`, espelhando a regra do binário),
   exigindo `200` e `"status":"ok"`;
2. **alcance TCP** do Postgres cujo host/porta vêm de `DATABASE_URL`.

Limite declarado: o `/healthz` atual (`crates/godmesh-server/src/http.rs`) devolve
JSON estático e **não consulta o pool**, então ele responde `200` com o banco caído.
O item 2 é alcance TCP — prova porta aberta, não prova consulta bem-sucedida. O
correto é `/healthz` devolver `503` quando o pool falhar; isso é dependência de
`http.rs` e está registrado como pendência (não implementado aqui).

## Estado verificado em 18/11/2026 (build real)

Execução em 2026-11-18T13:14Z–13:20Z, code base `70f0a57`. Evidência literal:
`docs/09-equipe/evidencias/2026-11-17-container-prod.md`, seção 6.

> Os números abaixo são da imagem de `70f0a57`. O build do gate D (18/11/2026,
> branch `room/d-container-producao-ahtf`) está em
> [Build do gate D](#build-do-gate-d-18112026) com tamanho próprio.

| Item | Estado |
|---|---|
| `docker build` da imagem de produção | **verificado**: build completo em 135,59 s (125,7 s de `cargo build --release --locked`) |
| Tamanho da imagem real | **155 MB** de disco / **34,1 MB** de conteúdo (154.860.288 bytes) |
| Bases `rust:1.100-slim-bookworm` e `debian:trixie-slim` | **verificado** na Docker Hub API em 18/11/2026: digests `arm64` e datas idênticos aos já registrados, sem repontamento |
| `docker run` sem `DATABASE_URL` | **verificado**: mensagem clara, sem panic, `exit=78` |
| `POST /v1/incidents` com grant válido | **verificado**: `201` com `ack_class: persisted_local` e linha real em `sos_incident` |
| `POST /v1/incidents` com `freshness: none` | **verificado**: `422` `DenyDeniedNoTrustedTime` |
| `POST /v1/incidents` com `share_precise_location` | **verificado**: `422` `DenyNeverOffline` |
| Runtime: uid 10001, sem cargo/rustc/gcc/cc/make | **verificado** na imagem real |
| `/migrations` resistente a root | **verificado** só com `--read-only`; `chmod a-w` sozinho não resiste a `--user 0:0` |
| `BIND_ADDR` obrigatório em produção | **verificado**: exit 78 com mensagem citando a variável |
| Senha de dev fora do binário | **verificado**: `strings target/release/godmesh-server | grep -c godmesh_dev_only` = `0` |
| `strip` no binário publicado | **verificado**: sem `.symtab`/`.strtab` (parse ELF) |
| `GET /healthz` | **verificado**: `200 {"ack_class":"transport_accepted","status":"ok"}`. **Não** consulta o pool |
| `HEALTHCHECK` da imagem | **verificado**: `healthy` com banco no ar; `unhealthy` após derrubar o Postgres (alcance TCP) |
| `SIGTERM` / graceful shutdown | **verificado**: log "encerrado", exit 0 |
| Build multi-arch (`amd64`) | **não feito** — só `linux/arm64` construído |
| Migração aplicada de dentro do contêiner | **não feito** — a imagem carrega `/migrations`, o binário atual não lê `GODMESH_MIGRATIONS_DIR` nem migra |
| Registry, assinatura/atestação, SBOM | **não feito** |
| Limites de memória/CPU, política de restart | **não configurados** |

O estado anterior (18/11/2026, antes de `crates/godmesh-server` existir) usou um build
com **stub**: o `docker build` falhava em `package ID specification 'godmesh-server'
did not match any packages`. Essa seção continua na evidência, marcada como histórica.

Regras que valem em produção:
- usuário sem privilégio (uid 10001), `--read-only`, `--cap-drop=ALL`, `--no-new-privileges`;
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com tag explícita, versão resolvida e
  digest do índice registrados (no topo do Dockerfile e na evidência).

## Build do gate D (18/11/2026)

Imagem `godmesh-server:gate-d` construída em `linux/arm64` (Docker 29.8.2, OrbStack,
macOS arm64), 18/11/2026 13:47Z. Evidência literal completa:
`docs/09-equipe/evidencias/2026-11-18-gate-d-container.md`.

| Item | Estado |
|---|---|
| `docker build` | **verificado**: 108,67 s na primeira execução; reprocesso 3,53 s (cache); 97,20 s após ajuste de formatação do `Dockerfile` |
| Tamanho | **152 MB** de disco / **33,7 MB** de conteúdo (`{{.Size}}` = 152.422.619 bytes, imagem `0b49a111e1cd`); antes: 155 MB / 34,1 MB |
| Arquitetura construída | **só `linux/arm64`** |
| `BIND_ADDR` obrigatório fora de dev | **verificado** local e no contêiner: exit 78 citando a variável |
| `GODMESH_ENV=dev` sem `BIND_ADDR` | **verificado** no contêiner: sobe em `127.0.0.1:8080` e o healthcheck resolve o mesmo endereço |
| Senha de dev no binário | **verificado** ausente: `strings` = `0`; binário da imagem também `0` |
| `strip` | **verificado**: ELF sem `.symtab` e sem `.strtab` (parse dos section headers) |
| Healthcheck com banco no ar | **verificado**: `healthy`, `Failing=0` |
| Healthcheck com Postgres derrubado | **verificado**: `Failing` crescente → `unhealthy` após 3 falhas (retries=3, intervalo 30 s) |
| `/healthz` com banco caído | **responde `200`** — lacuna conhecida, depende de `http.rs` |
| `touch /migrations/x` | **verificado**: falha com `Read-only file system` sob `--read-only`, mesmo com `--user 0:0`; **sucede** só com `chmod a-w` (root) |
| Servidor sob `--read-only --cap-drop=ALL --no-new-privileges --user 10001:10001` | **verificado**: healthcheck `0`, `/healthz` `200` |
| Build `amd64` | **não feito** |

Regras que valem em produção:
- `BIND_ADDR` explícito e publicação de porta feita pelo operador (`EXPOSE` não publica porta);
- usuário sem privilégio (uid 10001), `--read-only`, `--cap-drop=ALL`, `--no-new-privileges`;
- migrações versionadas em `/migrations`, aplicadas por etapa controlada — nunca por `docker-entrypoint-initdb.d`;
- segredos por variável de ambiente injetada, nunca na imagem;
- nada de `latest`: imagem base e dependências com tag explícita, versão resolvida e
  digest do índice registrados (no topo do Dockerfile e na evidência).

## Pendências registradas

1. **PostGIS**: as imagens `postgis/postgis` 17 e 18 são **amd64-only** (verificado em 17/11/2026). Este Mac é arm64, então o dev roda `postgres:18-alpine` sem PostGIS. O trabalho geoespacial (SPEC-011) fica bloqueado até haver imagem multi-arch homologada ou build próprio.
2. **Postgres 18 e volume**: a imagem exige mount em `/var/lib/postgresql` (não `.../data`). Já corrigido no compose.
3. **Build `amd64`**: a imagem de produção só foi construída e testada em `linux/arm64`.
   Nenhuma evidência de `amd64` com o binário real.
4. **Migração**: o binário não aplica migração e não lê `GODMESH_MIGRATIONS_DIR`. A
   imagem só carrega os `.sql`; a etapa controlada de migração ainda não existe.
5. **Publicação**: registry, assinatura/atestação e SBOM continuam fora de escopo até
   haver autorização para publicar.
6. **`/healthz` não consulta o pool**: depende de mudança em
   `crates/godmesh-server/src/http.rs` (devolver `503` quando o pool falhar). Enquanto
   isso, o `HEALTHCHECK` só prova alcance TCP do Postgres. Detalhes em
   `docs/09-equipe/evidencias/2026-11-18-gate-d-container.md`.
