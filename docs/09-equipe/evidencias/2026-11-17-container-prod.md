# Evidência — contêiner de produção `godmesh-server` (17/11/2026)

> Nome do arquivo conforme solicitado (ciclo de 17/11/2026). Consultas ao
> registry e builds foram executados em **18/11/2026 UTC** — os horários abaixo
> são literais.

**Ambiente:** macOS arm64, Docker 29.8.2 (build 7fc2dff), buildx v0.37.2, build via BuildKit.
**Arquivo sob teste:** `deploy/prod/Dockerfile` (multi-stage), `deploy/README.md`, `.dockerignore`.

> **Atualização — 18/11/2026, 13:14–13:20 UTC:** `crates/godmesh-server` foi criado e
> está em `main` (`70f0a57`). O **build real** da imagem foi então executado e passou.
> Resultado em **[seção 6](#6-build-real-repetido-18112026--passou)**. As seções 1 a 5
> abaixo foram mantidas como registro histórico: elas descrevem o período em que o
> binário não existia e o estágio de runtime só pôde ser provado com um **stub**.

## 1. Versões consultadas no Docker Hub (api pública)

Consulta: `curl https://hub.docker.com/v2/repositories/library/<repo>/tags/<tag>`

```
--- rust:1.100-slim-bookworm
name: 1.100-slim-bookworm updated: 2026-11-12T22:46:18.55064Z
   amd64 linux sha256:476f6598b8d14a1f1bc7e58efc67af206ac8a559c37a3401ce512db2582d0ff7
   arm64 linux sha256:a05e7b80377ed6099c177cad2b9f536e182b9574490c7b4c07acaa29a44fda57
   índice multi-arch: sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37

--- debian:trixie-slim
name: trixie-slim updated: 2026-11-05T08:28:05.296886Z
   amd64 linux sha256:7378260c0a098d27ac206225db5e7d92b8c7c84ea7afccc86eeb513756bbd392
   arm64 linux sha256:24a01011d10eba3a023bc8585b9650b8b29021240d4217c05b905e09ef4a5232
   índice multi-arch: sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
```

Série `rust` mais recente no momento da consulta: **1.100.0** (tag `1.100-slim-bookworm`,
publicada em 2026-11-12). A tag anterior da série era `1.99-slim-bookworm` (2026-11-05),
que é o `rust-version` declarado em `Cargo.toml` — o builder usa a série 1.100, compatível.

`debian:latest` = `debian:trixie-slim` (Debian 13, stable atual; `latest` publicado em
2026-11-05T08:25:18Z). Escolhemos a tag explícita `trixie-slim`, nunca `latest`.

## 2. Build da imagem de produção em 18/11/2026 — FALHA ESPERADA (binário ausente)

> **Nota de leitura:** esta seção é registro histórico. Naquele momento não havia
> binário `godmesh-server`; o build falhava de propósito. Ver seção 6 para o build real.

```
$ /usr/bin/time -p docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
#14 [builder 4/5] COPY crates ./crates
#14 DONE 3.5s

#15 [builder 5/5] RUN cargo build --release --locked --bin godmesh-server --package godmesh-server
#15 1.408 warning: unused field `repository` in `workspace.package`
#15 1.408   --> Cargo.toml:16:1
#15 2.103 error: package ID specification `godmesh-server` did not match any packages
#15 ERROR: process "/bin/sh -c cargo build --release --locked --bin godmesh-server --package godmesh-server" did not complete successfully: exit code: 101
------
Dockerfile:35
--------------------
  33 |     COPY crates ./crates
  34 |     
  35 | >>> RUN cargo build --release --locked --bin godmesh-server --package godmesh-server
--------------------
ERROR: failed to build: failed to solve: process "/bin/sh -c cargo build --release --locked --bin godmesh-server --package godmesh-server" did not complete successfully: exit code: 101
real 51.85
```

**Causa:** o pacote `crates/godmesh-server` não existe no workspace (nem na `main`,
nem no branch `room/s1-servidor-http-build-testes-e-smoke-guzr` quando consultado).
Nada foi simulado para mascarar a falha.

Consequências diretas:
- não há imagem `godmesh-server:dev`, portanto não há tamanho real da imagem final
  nem `docker run --rm godmesh-server:dev --help`;
- o comportamento de "falha clara por falta de `DATABASE_URL`" depende do binário
  (S1) e **não** foi verificado aqui.

## 3. Sonda descartável (build com stub) — o que já estava provado da imagem

> **Nota de leitura:** tudo aqui usou um **stub** `sh` no lugar do binário — a seção
> 3 do README antigo chamava isso de "build com stub". Nada desta seção vale como
> prova do binário real. Ver seção 6.

Para não deixar o Dockerfile sem nenhuma verificação, foi construída uma **sonda
descartável** em `/tmp/godmesh-probe/Dockerfile` (não versionada), idêntica em
estrutura ao estágio de runtime real, mas com:
- estágio builder: `cargo build --release --locked --workspace` (prova toolchain,
  lockfile e layout de `COPY`);
- binário `godmesh-server` substituído por um stub `sh` que falha com mensagem clara
  se `DATABASE_URL` estiver ausente.

```
$ /usr/bin/time -p docker build -f /tmp/godmesh-probe/Dockerfile -t godmesh-server:sonda .
#18 exporting manifest sha256:5a54fc7383c56903677e4525913d5b0711a9ea607e400013ea9fe2f8288f8125
#18 naming to docker.io/library/godmesh-server:sonda done
real 15.70
```

O workspace inteiro compilou com `--release --locked` (6 pacotes, lockfile fechado,
todas as dependências são `path` — nenhuma dependência externa baixada).

```
$ docker image ls godmesh-server
IMAGE                  ID             DISK USAGE   CONTENT SIZE   EXTRA
godmesh-server:sonda   44f9f5efeb27        145MB         31.4MB        
$ docker image inspect godmesh-server:sonda --format '{{.Size}} bytes'
145326603 bytes

$ docker run --rm --entrypoint /bin/sh godmesh-server:sonda -c 'id; ...'
uid=10001(godmesh) gid=10001(godmesh) groups=10001(godmesh)
HOME=/nonexistent
-rwxr-xr-x 1 root root 119 Nov 18 13:05 /usr/local/bin/godmesh-server
total 8
-r--r--r-- 1 root root 4999 Nov 18 13:03 0001_core.sql
dr-xr-xr-x 1 root root   38 Nov 18 13:03 tests
sem cargo
sem gcc

$ docker run --rm godmesh-server:sonda
godmesh-server: DATABASE_URL is not set
exit=78

$ docker image inspect godmesh-server:sonda \
    --format '{{json .Config.ExposedPorts}} Usuario={{.Config.User}} Entrypoint={{json .Config.Entrypoint}}'
{"8080/tcp":{}} Usuario=10001:10001 Entrypoint=["/usr/local/bin/godmesh-server"]
```

Provado com isso:
- usuário sem privilégio `10001:10001`, sem home, sem shell de login;
- `ca-certificates` presente; **sem** `cargo`, **sem** `gcc` no runtime (toolchain não vaza);
- `/migrations` copiado da imagem, `chmod a-w` aplicado (`-r--r--r--` / `dr-xr-xr-x`);
- `EXPOSE 8080` e `ENTRYPOINT` do binário em `/usr/local/bin/godmesh-server`;
- imagem de runtime enxuta: **145 MB** de disco / 31,4 MB de conteúdo (145.326.603 bytes).

## 4. O que a sonda (stub) NÃO provava

> **Nota de leitura:** lista histórica do período com stub. Os itens 1–3 e 5 foram
> cobertos na seção 6 com o binário real; os itens 4, 6 e 7 seguem em aberto.

1. Que o binário real `godmesh-server` compila e roda — ele não existe ainda.
2. Tamanho final da imagem de produção (depende do binário real; a sonda usa stub).
3. Que o servidor falha com mensagem clara (não panic) sem `DATABASE_URL` ou com banco
   inacessível — comportamento do binário, não do contêiner.
4. Aplicação das migrações: a imagem só as carrega em `/migrations`; nenhuma etapa de
   migração foi executada contra um Postgres.
5. Healthcheck, graceful shutdown em `SIGTERM`, limites de memória/CPU: não configurados
   nem testados.
6. Publicação em registry, assinatura/atestação de imagem, SBOM.
7. Build multi-arch (`arm64` + `amd64`): só `linux/arm64` foi construído neste Mac.

## 5. Próximo passo (quando o binário existir) — CUMPRIDO em 18/11/2026

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
docker run --rm godmesh-server:dev --help                 # esperado: ajuda sem banco
docker run --rm godmesh-server:dev                        # esperado: erro claro sem DATABASE_URL
docker image ls godmesh-server:dev                        # registrar tamanho
```

Executado na **seção 6**. Observação: o binário real **não implementa `--help`** —
recebe argumentos e os ignora; o `--help` previsto aqui não existe como comportamento.

---

# 6. Build real repetido (18/11/2026) — PASSOU

**Ambiente:** macOS arm64, Docker 29.8.2, BuildKit. Data/hora UTC da execução:
**2026-11-18T13:14Z–13:20Z**.
**Código:** `room/s5-build-real-do-container-azcy`, base `70f0a57`
(`feat(programa-a): servidor HTTP minimo com binario, contrato verificado e smoke real`).

## 6.1 Versões das bases — reconferidas na Docker Hub API em 18/11/2026

```
--- rust:1.100-slim-bookworm
updated: 2026-11-12T22:46:18.55064Z
  amd64 linux sha256:476f6598b8d14a1f1bc7e58efc67af206ac8a559c37a3401ce512db2582d0ff7
  arm64 linux sha256:a05e7b80377ed6099c177cad2b9f536e182b9574490c7b4c07acaa29a44fda57
--- debian:trixie-slim
updated: 2026-11-05T08:28:05.296886Z
  amd64 linux sha256:7378260c0a098d27ac206225db5e7d92b8c7c84ea7afccc86eeb513756bbd392
  arm64 linux sha256:24a01011d10eba3a023bc8585b9650b8b29021240d4217c05b905e09ef4a5232
```

**Nada mudou**: os digests `arm64` e as datas são idênticos aos registrados em 18/11/2026
na seção 1 — nenhuma tag foi repontada. Digests do **índice multi-arch** confirmados
pelo próprio build (linha `FROM ...@sha256:` do BuildKit), que é a prova mais forte:

```
#7 [builder 1/5] FROM docker.io/library/rust:1.100-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
#8 [runtime 1/6] FROM docker.io/library/debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2
```

Mudança feita no Dockerfile nesta rodada: os `ARG` das bases passaram a fixar
**tag + digest do índice** (antes só a tag). Com o digest fixado o build bateu 100% de
cache (2,17 s), o que confirma que o digest corresponde exatamente à tag consultada.

## 6.2 Build

```
$ /usr/bin/time -p docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
#13 [builder 5/5] RUN cargo build --release --locked --bin godmesh-server --package godmesh-server
#13 DONE 125.7s
#16 [runtime 4/6] COPY --from=builder /src/target/release/godmesh-server /usr/local/bin/godmesh-server
#16 DONE 0.6s
#19 naming to docker.io/library/godmesh-server:dev done
real 135.59
```

Build completo **135,59 s** (dos quais 125,7 s são o `cargo build --release --locked`;
houve download de crates.io, `Cargo.lock` respeitado). Reprocesso só de metadados
(após pinar digest e adicionar healthcheck): **2,54 s** e **2,17 s**, tudo cache.

```
$ docker images godmesh-server
IMAGE                  ID             DISK USAGE   CONTENT SIZE   EXTRA
godmesh-server:dev     04a42169211e        155MB         34.1MB
godmesh-server:sonda   44f9f5efeb27        145MB         31.4MB

$ docker image inspect godmesh-server:dev --format '{{.Size}} bytes'
154860288 bytes
```

Binário real: **155 MB** de disco / **34,1 MB** de conteúdo (154.860.288 bytes).
A sonda com stub da seção 3 tinha 145 MB / 31,4 MB — a diferença é o binário real.

## 6.3 `docker run` sem `DATABASE_URL`

```
$ docker run --rm godmesh-server:dev
godmesh-server: configuração inválida: DATABASE_URL não definida; exemplo: DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh
exit=78
```

Falha clara, sem panic/backtrace, `78` = `EX_CONFIG` (sysexits). Verificado.

## 6.4 Os três casos de `/v1/incidents` (Postgres real em `localhost:55432`)

```
$ docker run -d --name gm-final --network host \
    -e DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
    -e BIND_ADDR=127.0.0.1:18081 godmesh-server:dev
godmesh-server: escutando em http://127.0.0.1:18081
```

Caso 1 — grant válido (`open_sos_incident` + `f3_verified_online`, revogação `absent`):

```
$ curl -s -i -X POST http://127.0.0.1:18081/v1/incidents -H 'content-type: application/json' \
  -d '{"incident_id":"inc-final-001","tenant_id":"tenant-1","principal":"principal-42","operation":"open_sos_incident","freshness":"f3_verified_online","revocation":"absent","epoch_comparable":true,"action_enumerated":true,"retention_class_known":true}'
HTTP/1.1 201 Created
content-type: application/json
content-length: 136

date: Wed, 18 Nov 2026 13:20:00 GMT

{"incident_id":"inc-final-001","state":"persisted_local","ack_class":"persisted_local","ui_label":"registrado localmente, não enviado"}
```

Caso 2 — `freshness: none`:

```
HTTP/1.1 422 Unprocessable Entity
content-type: application/json
content-length: 92

{"ack_class":"transport_accepted","detail":"DenyDeniedNoTrustedTime","error":"grant_denied"}
```

Caso 3 — `share_precise_location`:

```
HTTP/1.1 422 Unprocessable Entity
content-type: application/json
content-length: 85

{"ack_class":"transport_accepted","detail":"DenyNeverOffline","error":"grant_denied"}
```

Persistência conferida no Postgres de dev:

```
$ docker exec -i godmesh-pg psql -U godmesh -d godmesh -c "SELECT id,tenant_id,state FROM sos_incident WHERE id LIKE 'inc-final-%';"
      id      | tenant_id |      state
--------------+-----------+-----------------
 inc-final-001 | tenant-1  | persisted_local
(1 row)
```

## 6.5 Runtime verificado na imagem real

```
$ docker exec gm-final id
uid=10001(godmesh) gid=10001(godmesh) groups=10001(godmesh)

$ docker exec gm-final sh -c 'grep godmesh /etc/passwd'
godmesh:x:10001:10001::/nonexistent:/usr/sbin/nologin

$ docker exec gm-final sh -c 'for b in cargo rustc gcc cc make; do command -v $b || echo "sem $b"; done'
sem cargo
sem rustc
sem gcc
sem cc
sem make

$ docker exec gm-final sh -c 'ls -la /migrations'
dr-xr-xr-x 1 root root   36 Nov 18 13:15 .
-r--r--r-- 1 root root 4999 Nov 18 13:14 0001_core.sql
dr-xr-xr-x 1 root root   38 Nov 18 13:15 tests

$ docker exec gm-final sh -c 'touch /migrations/x; echo touch_exit=$?'
touch: cannot touch '/migrations/x': Permission denied
touch_exit=1

$ docker exec gm-final sh -c 'command -v wget; command -v nc; command -v curl'
nada (imagem sem curl, wget ou nc)
```

## 6.6 Healthcheck

O Dockerfile **não tinha** `HEALTHCHECK` até esta rodada (a seção 4 dizia isso).
Foi adicionado sem trazer dependência nova: a imagem não tem `curl`/`wget`/`nc`, então
o check usa `/bin/bash` + `/dev/tcp`, lendo a porta de `BIND_ADDR` (default `8080`):

```
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD /bin/bash -c 'port="${BIND_ADDR:-127.0.0.1:8080}"; port="${port##*:}"; exec 3<>/dev/tcp/127.0.0.1/$port || exit 1; printf "GET /healthz HTTP/1.0\r\n\r\n" >&3; grep -q "transport_accepted" <&3'
```

```
$ curl -s -i http://127.0.0.1:18081/healthz
HTTP/1.1 200 OK
content-type: application/json
content-length: 48

{"ack_class":"transport_accepted","status":"ok"}

$ docker inspect gm-final --format 'Health={{.State.Health.Status}} Failing={{.State.Health.FailingStreak}}'
Health=healthy Failing=0
```

## 6.7 SIGTERM / shutdown

```
$ docker stop -t 10 gm-final
godmesh-server: escutando em http://127.0.0.1:18081
godmesh-server: encerrado
exitcode=0
```

Graceful shutdown em `SIGTERM` com saída `0` — antes não verificado.

## 6.8 O que continua NÃO verificado

1. **Build multi-arch**: só `linux/arm64` foi construído (este Mac é arm64). `amd64`
   nunca foi compilado nem testado.
2. **Aplicação de migrações pela imagem**: `/migrations` está na imagem e é read-only,
   mas nenhuma etapa de migração foi executada *de dentro* do contêiner. O Postgres de
   dev foi inicializado por `docker-entrypoint-initdb.d`.
3. **Diretório de migrações configurável**: `GODMESH_MIGRATIONS_DIR=/migrations` é
   exportado, mas o binário atual **não lê** essa variável (não aplica migração alguma).
4. **Publicação em registry, assinatura/atestação, SBOM**: nada feito. O build gera
   atestação local do BuildKit, que não é assinatura publicada.
5. **Limites de memória/CPU e política de restart**: não configurados.
6. **Postgres gerenciado/produção**: os testes rodaram contra o `postgres:18-alpine`
   local de dev, não contra um banco de produção.
7. **`--help`**: o binário não implementa ajuda/CLI; a expectativa antiga da seção 5
   era incorreta.
8. **`amd64` + binário real**: o `Cargo.lock` não muda, mas não há evidência de build
   `amd64` com o binário real.