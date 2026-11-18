# Evidência — infraestrutura de desenvolvimento em OrbStack (17/11/2026)

**Ambiente:** macOS arm64, OrbStack 2.2.5, Docker 29.8.2, imagem `postgres:18-alpine` (multi-arch).
**Decisão do usuário:** dependências de infraestrutura no OrbStack; backend/frontend/apps locais; produção em contêiner.

## O que foi executado

1. `docker compose -f deploy/dev/compose.yaml up -d` → container `godmesh-pg` **Up (healthy)**.
2. Migração `migrations/0001_core.sql` aplicada na inicialização do volume.
3. Verificação de invariantes no banco:

```
NOTICE:  OK 3: posição duplicada recusada
NOTICE:  OK 4: ACK append-only (reescrita bloqueada)
NOTICE:  OK 5: estado inventado recusado
       resultado       
 verificação concluída
```

## Invariantes provados **no banco** (não só em Rust)

| # | Invariante | Resultado |
|---|---|---|
| 1 | Evento sem classe de retenção é recusado (ADR-062) | FK violation → recusado |
| 2 | Evento com classe válida é aceito | aceito |
| 3 | Posição duplicada é recusada (ADR-057) | PK violation → recusada |
| 4 | Trilha de ACK é append-only (ADR-058) | trigger bloqueou reescrita |
| 5 | Estado de incidente é conjunto fechado (ADR-058/064) | CHECK recusou "enviado" |

## Achados de ambiente

- **PostGIS é amd64-only** nas imagens 17 e 18 do `postgis/postgis` (verificado no registry). Em arm64 o dev roda `postgres:18-alpine` **sem** PostGIS; geoespacial (SPEC-011) fica bloqueado até imagem multi-arch homologada.
- **Postgres 18 mudou o mount**: exige `/var/lib/postgresql`, não `/var/lib/postgresql/data`. Corrigido.

## O que isso NÃO prova

- Nenhum app Rust conecta no banco: não há driver (`sqlx`/tokio-postgres) instalado.
- A imagem de produção **não foi construída**: o binário `godmesh-server` não existe.
- Sem teste de carga, backup, restore ou failover.
