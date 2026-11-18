# Evidência — contêiner de produção `godmesh-server` (17/11/2026)

> Nome do arquivo conforme solicitado (ciclo de 17/11/2026). Consultas ao
> registry e builds foram executados em **18/11/2026 UTC** — os horários abaixo
> são literais.

**Ambiente:** macOS arm64, Docker 29.8.2 (build 7fc2dff), buildx v0.37.2, build via BuildKit.
**Arquivo sob teste:** `deploy/prod/Dockerfile` (multi-stage), `deploy/README.md`, `.dockerignore`.

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

## 2. Build real da imagem de produção — FALHA ESPERADA

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

## 3. Sonda descartável — o que já está provado da imagem

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

## 4. O que isso NÃO prova

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

## 5. Próximo passo (quando o binário existir)

```bash
docker build -f deploy/prod/Dockerfile -t godmesh-server:dev .
docker run --rm godmesh-server:dev --help                 # esperado: ajuda sem banco
docker run --rm godmesh-server:dev                        # esperado: erro claro sem DATABASE_URL
docker image ls godmesh-server:dev                        # registrar tamanho
```

Trocar aqui os resultados da sonda pelos resultados reais e remover a seção 3.