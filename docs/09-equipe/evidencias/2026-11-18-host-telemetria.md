# Evidência — Telemetria de recursos do nó, camada "observar" (18/11/2026)

**Branch:** `room/h1-telemetria-de-recursos-do-no-ekqh` (worktree desta thread)
**Base:** `6a8e3cf docs(adr): ADR-073 agente de recursos do no - observar, decidir, agir`
**Escopo:** ADR-073 **camada 1 (observar, read-only)** apenas. Nenhuma ação de
escrita: nenhum firewall, rota, `ulimit`, dreno, `kill` ou regra de conntrack foi
lida, sugerida como automática ou executada. Camada 2 (decidir: admissão por
classe, readiness por pressão) e camada 3 (agir) continuam não implementadas —
e esta fatia é deliberadamente incapaz de julgar saúde a partir de pressão.

**Ambiente da execução:** macOS 27.0.0 (Darwin), arm64; `rustc 1.99.1
(58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`.
Postgres de desenvolvimento em `postgres://godmesh:godmesh_dev_only@localhost:55432`,
banco **próprio desta suíte: `godmesh_h1`**, criado do zero e migrado pelo
runner oficial (`godmesh_server::migrate::migrate`) — nenhum `DROP`/`TRUNCATE`
no banco compartilhado `godmesh`.

> Este host **não tem `/proc`**. Isso não é um detalhe do relatório: é a
> condição que obrigou a separação entre parse (fixtures) e leitura (raiz
> configurável), e é também o motivo pelo qual a conferência com `top`/`free`/`ss`
> **não foi executada** (§ não provado).

---

## 1. O que foi construído

| Caminho | O que é |
|---|---|
| `crates/godmesh-host/` (crate nova) | coleta read-only, parsers, exposition, config, árvore de `/proc` copiável, binário de evidência |
| `crates/godmesh-host/src/proc.rs` | parsers puros sobre `&str` (nenhum toca disco) |
| `crates/godmesh-host/src/read.rs` | raiz configurável (`/` em produção, diretório de fixture no teste) |
| `crates/godmesh-host/src/collect.rs` | `Collector` (monta o snapshot, decide lido/ausente), `SharedSnapshot` |
| `crates/godmesh-host/src/expose.rs` | texto Prometheus `godmesh_host_*` |
| `crates/godmesh-host/src/snapshot.rs` | modelo; `Capacity` (resumo do `/healthz`) |
| `crates/godmesh-host/src/volumes.rs` | volumes por `statfs` via `sysinfo` + filtro de pseudo-fs |
| `crates/godmesh-host/src/config.rs` | `GODMESH_HOST_METRICS_INTERVAL_MS`, `GODMESH_HOST_PROC_ROOT` |
| `crates/godmesh-host/src/tree.rs` | cópia da árvore legível de `/proc` para disco (evidência reproduzível) |
| `crates/godmesh-host/src/bin/host_evidence.rs` | `dump` / `metrics` / `proc-fs DIR [ARQ]` / `volumes` / `tree-write DIR` |
| `crates/godmesh-server/src/http/mod.rs` | `HostTelemetry` no estado; `/metrics` com a seção; `/healthz` com `capacity` |
| `crates/godmesh-server/src/main.rs` | tarefa de coleta em background (`spawn_blocking`), config, shutdown |
| `crates/godmesh-host/tests/` | `parsers.rs` (26), `collect_fixtures.rs` (18), `exposition.rs` (7), `cost.rs` (4) + fixtures `linux`, `container`, `container-v1`, `container-v1-unlimited` |
| `crates/godmesh-server/tests/host_telemetry.rs` | 7 testes no fio (`/metrics` autenticado, `/healthz`) |

### Por que crate nova, e não módulo em `godmesh-server`

Não é "mais simples" nos eixos que importam aqui:

* **o que se testa não é o que se injeta.** Um módulo dentro de `godmesh-server`
  tem o workspace de testes daquele crate, que exige Postgres. Os parsers de
  `/proc` não têm relação com banco, e a tarefa exige que eles não dependam do
  host do CI. Na crate própria, `cargo test -p godmesh-host` roda os 52 testes
  de formato e coleta em qualquer máquina, sem banco.
* **fronteira de privilégio.** A camada 3 (agir) vai existir e precisa de um
  crivo de segurança próprio (`host:admin`, `dry-run`, auditoria). Com o host
  num crate, a revisão enxerga a fronteira por `Cargo.toml`; dentro de
  `godmesh-server` ela fica enterrada atrás de handlers de incidente.
* **dependência nova isolada.** `sysinfo` entra **só** em `godmesh-host`
  (feature `disk`, `default-features = false`). `godmesh-server` não ganha
  dependência de third-party nesta fatia — ganha um path dep do crate do
  workspace.
* **reuso pelo agente.** A camada 1 existe para ser lida por outros binários
  (`host-evidence`, futuro agente de borda). Um módulo privado de servidor não
  serve ninguém fora do servidor.

---

## 2. Dependências: versões confirmadas no registry e o que foi usado

Registry consultado em 18/11/2026 (crates.io API; a lista de versões foi
pedida junto para não confiar num único campo):

```
$ curl https://crates.io/api/v1/crates/sysinfo               -> max_stable=0.40.4  newest=0.40.4  updated=2026-11-16
$ curl https://crates.io/api/v1/crates/procfs                -> max_stable=0.18.0  newest=0.18.0  updated=2025-08-30
$ curl https://crates.io/api/v1/crates/if-addrs              -> max_stable=0.15.0  newest=0.15.0  updated=2026-02-08
$ curl https://crates.io/api/v1/crates/netlink-packet-utils  -> max_stable=0.6.0   updated=2025-08-27
$ curl https://crates.io/api/v1/crates/if-addrs (versões)    -> 0.15.0 (2026-02-08), 0.14.0 (2025-07-09), 0.13.4, 0.13.3, …
```

**Nenhuma divergência em relação ao ADR-073:** as três versões que a decisão
registra (`sysinfo` 0.40.4, `procfs` 0.18.0, `if-addrs` 0.15.0) são exatamente
as estáveis mais recentes no dia da consulta, e `netlink-packet-utils` 0.6.0
também.

Registro de processo, porque importa para a confiabilidade desta evidência: uma
leitura intermediária desta consulta anotou `if-addrs 0.16.0` e chegou a
produzir uma "correção de ADR" com esse número. A correção estava errada — o
campo lido veio de uma resposta parcial da API. A consulta foi repetida contra
a lista de versões antes de este arquivo ser escrito, o texto falso foi
desfeito, e a linha acima fica como o que foi efetivamente verificado.

O que **não** entrou, com o motivo, seguindo o critério do próprio ADR
("preferir leitura direta de `/proc` onde o crate não agrega valor"):

| Crate | Resolvida | Usada? | Motivo |
|---|---|---|---|
| `sysinfo` | 0.40.4 | **sim**, `default-features = false, features = ["disk"]` | única coisa que `/proc` não tem: espaço livre de filesystem é `statvfs(2)` |
| `procfs` | 0.18.0 | não | os 16 arquivos desta fatia têm formato estável e documentado em `proc(5)`; o parse próprio coube em um módulo com 25 testes de fixture e não adiciona um crate com `unsafe` para ler texto |
| `if-addrs` | 0.15.0 | não | a tarefa pede **contadores** (bytes/pacotes/erros/drops), que só existem em `/proc/net/dev`; `if-addrs` devolve endereços de interface, e endereço de interface é dado de topologia que nada nesta fatia publica |
| `netlink-packet-utils` | 0.6.0 | não | conntrack é lido por arquivo; netlink está explicitamente fora desta fatia |

Motivo de `sysinfo` ser insubstituível aqui, verificado no código da versão
resolvida (não por memória):

```toml
# crates/godmesh-host/Cargo.toml
sysinfo = { version = "0.40.4", default-features = false, features = ["disk"] }
```

```
~/.cargo/registry/src/index.crates.io-*/sysinfo-0.40.4/src/unix/linux/disk.rs
  244:  let frsize = cast!(stat.f_frsize);
  245:  let blocks = cast!(stat.f_blocks);
  246:  let bavail = cast!(stat.f_bavail);
  248:  let total = frsize.saturating_mul(blocks);
  252:  let available = frsize.saturating_mul(bavail);
  253:  let is_read_only = (stat.f_flag & libc::ST_RDONLY) != 0;
```

Ou seja: no Linux, `total`/`available`/`read_only` que publicamos são **os
mesmos campos que `df -k` lê** (`f_frsize·f_blocks`, `f_frsize·f_bavail`). Não é
analogia — é a mesma chamada, e é por isso que a conferência de §5.4 fecha em
`total` com exatidão de byte.

MSRV: `sysinfo` 0.40.4 declara `rust-version = 1.95`; o workspace usa
`rust-version = 1.99` com `rustc 1.99.1` ✔.

**Nada além do necessário entrou:** `sysinfo` puxa `libc` (já no `Cargo.lock`
por `ring`/`sqlx`/`tokio`) e, só no alvo Apple, `objc2-io-kit`. O `Cargo.lock`
ganhou **13** entradas: `godmesh-host`, `sysinfo`, `objc2-io-kit` e 10
`windows-*` (que só compilem em target Windows; inertes no Linux onde o nó roda).

---

## 3. Contrato das métricas expostas

Prefixo `godmesh_host_`. Unidade no nome (`_bytes`, `_seconds`, `_ratio`,
`_total` para o que só cresce). Vocabulário de rótulos **fechado**, exceto
`volume` e `interface`, capados em 16 cada com o corte declarado.

| Família | Tipo | Fonte (arquivo/chamada) | O que **não** prova |
|---|---|---|---|
| `godmesh_host_metrics_available{source="proc"\|"cgroup"\|"statfs"\|"none"}` | gauge | — | nada sobre o serviço; só se o host foi lido |
| `godmesh_host_group_available{group="cpu"\|"memory"\|"pressure"\|"disk"\|"descriptors"\|"sockets"\|"tcp"\|"conntrack"\|"netdev"\|"cgroup"}` | gauge | — | magnitude alguma |
| `godmesh_host_cpu_seconds_total{class=…}`, `class="total"` | counter | `/proc/stat` linha `cpu` ÷ 100 Hz | throttling de cgroup (`cgroup_cpu_throttled_*` prova isso) |
| `godmesh_host_cpu_usage_ratio{class="busy"\|user\|system\|io_wait\|steal\|…}` | gauge | delta de `/proc/stat` entre duas coletas | uso desde o boot; é a **janela** (`collection_window_seconds`) |
| `godmesh_host_cpu_cores` | gauge | linhas `cpuN` de `/proc/stat` | quota do cgroup |
| `godmesh_host_load_average{window="1m"\|"5m"\|"15m"}` | gauge | `/proc/loadavg` | fração de CPU; é contagem de tarefas |
| `godmesh_host_threads{state="runnable"\|"total"}` | gauge | `/proc/loadavg` | threads do processo (`process_threads`) |
| `godmesh_host_process_threads` | gauge | `/proc/self/status` `Threads:` | threads do host |
| `godmesh_host_memory_bytes{state="total"\|"free"\|"available"\|"used"}` | gauge | `/proc/meminfo` | limite do contêiner — **em contêiner é a memória do host físico**, a interface não conhece cgroup |
| `godmesh_host_memory_usage_ratio` | gauge | derivado (total − available)/total | pressão (`pressure_stalled_ratio`) |
| `godmesh_host_swap_bytes{state=…}` | gauge | `/proc/meminfo` | uso de swap por processo |
| `godmesh_host_pressure_stalled_ratio{resource,kind,window,scope}` | gauge | `/proc/pressure/{cpu,memory,io}` (`scope="host"`), `<cgroup>/{cpu,memory,io}.pressure` (`scope="cgroup"`) | existência de PSI (`CONFIG_PSI`); ausente ≠ 0 |
| `godmesh_host_pressure_stalled_seconds_total{resource,kind,scope}` | counter | campo `total=` (µs) ÷ 1e6 | tempo de stall do processo isolado |
| `godmesh_host_cgroup_memory_current_bytes`, `_memory_limit_bytes`, `_memory_usage_ratio` | gauge | v2 `memory.current`/`memory.max`; v1 `memory.usage_in_bytes`/`memory.limit_in_bytes` (sentinela `LONG_MAX` = sem limite) | hipervisor físico — **é o namespace** |
| `godmesh_host_cgroup_cpu_limit_cores` | gauge | `cpu.max` (quota/período); **ausente em v1** | núcleos visíveis (`cpu_cores`), e em v1 não prova quota nenhuma |
| `godmesh_host_cgroup_cpu_throttled_seconds_total`, `_throttled_periods_total` | counter | `cpu.stat` (`throttled_usec`, `nr_throttled`) | throttle por pressão de CPU do host |
| `godmesh_host_cgroup_version` | gauge | `/proc/self/mounts` (cgroup2 = 2) | conteúdo dos arquivos |
| `godmesh_host_disk_bytes{volume,state="total"\|"available"\|"used"}`, `_disk_usage_ratio`, `_disk_free_ratio`, `_disk_read_only` | gauge | `statvfs(2)` via `sysinfo` | **inodes** (lacuna §6), I/O por device, ocupação por tenant |
| `godmesh_host_disk_volumes`, `godmesh_host_cardinality_truncated{type="volume"\|"interface"}` | gauge | — | que volumes ilegíveis existiam |
| `godmesh_host_process_descriptors{state="open"\|"limit_soft"\|"limit_hard"}` | gauge | `/proc/self/fd` (contagem), `/proc/self/limits` | descritores de outros processos; `unlimited` **não sai como número** |
| `godmesh_host_process_descriptors_usage_ratio` | gauge | derivado open/soft | existência do limite |
| `godmesh_host_sockets{protocol="used"\|"tcp"\|"udp"\|"frag"\|…}` | gauge | `/proc/net/sockstat` `inuse` | sockets por conexão/porta |
| `godmesh_host_socket_memory_pages{protocol}` | gauge | `/proc/net/sockstat` `mem` | **bytes**: a unidade do kernel é página, e não multiplicamos por page size chutado |
| `godmesh_host_tcp_connections{state}`, `godmesh_host_tcp_connections_observed` | gauge | `/proc/net/tcp` + `tcp6`, coluna `st` (hex) | quem conecta (sem endereço/porta/peer) |
| `godmesh_host_conntrack_entries`, `_conntrack_limit`, `_usage_ratio` | gauge | `/proc/sys/net/netfilter/nf_conntrack_count`, `_max` | tabela de outro namespace; **netlink não é usado** |
| `godmesh_host_network_interface_{bytes,packets,errors,dropped}_total{interface,direction}` | counter | `/proc/net/dev` | taxa (`rate()` no Prometheus calcula); namespace de rede ≠ cabo físico |
| `godmesh_host_collection_duration_seconds`, `godmesh_host_snapshot_age_seconds`, `godmesh_host_collection_window_seconds`, `godmesh_host_container` | gauge | instrumento interno | capacidade de produção |

Vocabulário de estados TCP indexado **pelo valor**, conferido no enum `SS_*` de
`misc/ss.c` do iproute2 (18/11/2026): 1 `established`, 2 `syn_sent`, 3
`syn_recv`, 4 `fin_wait1`, 5 `fin_wait2`, 6 `time_wait`, 7 `close`, 8
`close_wait`, 9 `last_ack`, 10 `listen` (o `0A` clássico), 11 `closing`, 12
`new_syn_recv`. Valor fora da tabela sai `unknown`, nunca mapeado num estado
velho — teste `tcp_de_estado_desconhecido_vira_unknown_nao_um_estado_velho`.

---

## 4. Coleta eficiente e custo medido

Um `Collector` por processo, tarefa em background no intervalo configurado
(`GODMESH_HOST_METRICS_INTERVAL_MS`, default **5 s**), publicando num
`SharedSnapshot` (`Arc<Mutex<Option<Arc<Snapshot>>>>`). `spawn_blocking` porque
a leitura de `/proc` é I/O síncuo; o `await` garante uma coleta por vez.

**O handler de `/metrics` não abre nenhum arquivo do kernel.** Lê o snapshot
publicado. Medido (teste `uma_coleta_de_snapshot_cabe_no_orcamento_declarado`):

```
custo da coleta (fixture linux, 7 coletas): média 1009 µs, máximo 1394 µs, orçamento 250000 µs (5% do intervalo de 5 s)
custo da coleta (fixture container, 7 coletas): máximo 1180 µs
custo de servir o snapshot: 63 ns por leitura
```

O orçamento declarado é **5 % do intervalo** (`godmesh_host::budget`: 250 ms com
intervalo de 5 s) — escolha desta fatia, não parâmetro aprovado, e por isso está
na assinatura da função, não escondido num teste. 7 amostras, sem laço apertado:
laço apertado mediria o cache de página do kernel, não a coleta.

O que **não** está nos números acima: a enumeração de volumes. No macOS deste
host ela é dominada por IOKit e aparece tanto em `host-evidence dump`
(`custo desta coleta: 0.082124 s`) quanto no binário real servindo
`godmesh_host_collection_duration_seconds 0.080176` (§5.9) — 32 % do orçamento
de 250 ms com o intervalo de default de 5 s. Com volumes
injetados (sem syscall), a mesma árvore custa ~1 ms. Em Linux o backend é
`statvfs` por montagem (`src/unix/linux/disk.rs`, §2), e não medimos esse
custo em Linux (§6.1). O gauge existe justamente para isso não ficar como
crença: o custo da última coleta é público no `/metrics`.

Percentil **não** é publicado: 7 amostras não sustentam p95, e a regra deste
repositório exige população, método e janela declarados.

Coleta no instante do boot (antes do primeiro tick): sem isso o nó nasceria com
`/metrics` dizendo "nenhuma leitura" durante um intervalo inteiro.

---

## 5. O que foi executado, com resultado literal

### 5.1 Suítes desta fatia

Executado com `DATABASE_URL=postgres://…/godmesh_h1` (banco próprio da suíte,
migrado pelo runner oficial). Saídas literais, uma por target desta fatia:

```
$ cargo test -p godmesh-host --lib                     -> ok.  9 passed; 0 failed; 0 ignored
$ cargo test -p godmesh-host --test parsers            -> ok. 26 passed; 0 failed; 0 ignored
$ cargo test -p godmesh-host --test collect_fixtures   -> ok. 18 passed; 0 failed; 0 ignored
$ cargo test -p godmesh-host --test exposition         -> ok.  7 passed; 0 failed; 0 ignored
$ cargo test -p godmesh-host --test cost               -> ok.  4 passed; 0 failed; 0 ignored
$ cargo test -p godmesh-server --test host_telemetry   -> ok.  7 passed; 0 failed; 0 ignored
                                                     (71 testes novos no total)

$ cargo test --workspace
passando=377 falhando=0 ignorados=1
```

O único ignorado é pré-existente e não é desta fatia:
`colombian_tenant_without_policy_is_refused_with_retention_policy_missing`
(`ignored, altera o registro de política de CO; roda isolado com --ignored`).

Comando reproduzível:

```
createdb godmesh_h1
DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_h1 \
  cargo test --workspace
DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_h1 \
  cargo test -p godmesh-host -p godmesh-server -- --nocapture
```

### 5.2 Lint

```
$ cargo clippy --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)   # sem saída de diagnóstico
```

Zero `unsafe` no workspace: `#![forbid(unsafe_code)]` em
`crates/godmesh-host/src/lib.rs`, `Cargo.toml` com `[lints] workspace = true` e
o workspace já tem `unsafe_code = "forbid"`.

### 5.3 Exposition gerado de uma árvore de `/proc` real de fixture

```
$ ./target/debug/host_evidence proc-fs crates/godmesh-host/tests/fixtures/linux /tmp/host-metrics-fixture.txt
exposition escrito em /tmp/host-metrics-fixture.txt: 215 linhas

godmesh_host_metrics_available{source="proc"} 1
godmesh_host_group_available{group="cpu"} 1
godmesh_host_group_available{group="cgroup"} 0
godmesh_host_cpu_cores 4
godmesh_host_cpu_seconds_total{class="user"} 10      # 1000 jiffies / 100 Hz
godmesh_host_cpu_seconds_total{class="io_wait"} 4    # 400 jiffies / 100 Hz
godmesh_host_cpu_seconds_total{class="steal"} 0.5    # 50 jiffies / 100 Hz
godmesh_host_cpu_seconds_total{class="total"} 69.8   # soma das oito classes
godmesh_host_memory_bytes{state="total"} 16777216000 # 16384000 kB × 1024
godmesh_host_memory_bytes{state="available"} 10485760000
godmesh_host_memory_bytes{state="used"} 6291456000   # derivado: total − available
godmesh_host_memory_usage_ratio 0.375                # 6291456000 / 16777216000
godmesh_host_pressure_stalled_ratio{resource="memory",kind="some",window="10",scope="host"} 6
godmesh_host_tcp_connections{state="listen"} 3       # 2 no tcp + 1 no tcp6 (0x0A)
godmesh_host_tcp_connections{state="time_wait"} 1    # 0x06
godmesh_host_conntrack_entries 1234
godmesh_host_conntrack_limit 65536
godmesh_host_conntrack_usage_ratio 0.018828          # 1234/65536
godmesh_host_network_interface_bytes_total{interface="eth0",direction="rx"} 5000000
godmesh_host_sockets{protocol="tcp"} 17              # coluna inuse
godmesh_host_socket_memory_pages{protocol="tcp"} 3   # páginas, não bytes
```

Conferência aritmética (feita à mão no fixture e nos testes):

* CPU: `cpu  1000 200 300 5000 400 10 20 50 0 0` ÷ 100 Hz → 10/2/3/50/4/0.1/0.2/0.5 s,
  total 69,8 s. As duas últimas colunas (`guest`, `guest_nice`) **não** entram:
  já estão contadas em `user`, e somá-las daria busy acima de 1.
* Janela de taxa: duas amostras com deltas user +100, system +50, idle +400,
  io_wait +20, steal +10 → total 580 jiffies, busy 150 → `150/580 = 0.258621`;
  e a soma das oito frações é exatamente 1,0 (teste que prova que nenhuma
  coluna foi perdida nem duplicada).
* Memória: kB × 1024; usado **derivado** de `MemAvailable`, com a derivação
  escrita na `HELP`.
* PSI: `some avg10=6.00 … total=8000000` → ratio 6,0 e 8,0 s acumulados.
* TCP: coluna `st` em hex, `0A` = listen.

### 5.4 Conferência com ferramenta de sistema **neste host** (mesma chamada)

`statvfs` não é `/proc`, e a máquina de desenvolvimento tem `df -k` falando a
mesma chamada de sistema. No instante da execução:

```
$ ./target/debug/host_evidence volumes
  /            apfs  total 494332366848 bytes  available 66982555939 bytes  free 0.1355  read-only
  /Volumes/Claude hfs total  1355767808 bytes  available   371716096 bytes  free 0.2742  read-only

$ df -k /
Filesystem     1024-blocks      Used  Available Capacity iused      ifree %iused  Mounted on
/dev/disk3s1s1   482746452  13335156   26987796       34%  484019  269877960     0%   /
```

* **`total` bate exatamente:** 482 746 452 KiB × 1024 = 494 332 366 848 bytes =
  o valor de `statfs` que publicamos. Mesma grandeza, mesma origem.
* **`available` NÃO bate neste host:** 66 982 555 939 (publicado) contra
  27 635 491 584 bytes (26 987 796 KiB × 1024, coluna Available de `df -k`). A diferença é explicada, não
  escondida: no backend Apple do `sysinfo` 0.40.4 (`src/unix/apple/disk.rs:130`,
  `get_available_volume_space`) o "available" vem de `IOKit` (inclui espaço
  purgável do APFS), enquanto `df -k` usa `f_bavail·f_frsize`. **No Linux, que é
  onde o nó roda, o backend é o `statvfs` puro** (código em §2), i.e. igual ao
  `df -k`. Consequência honesta: `disk_free_ratio` tem semântica de
  "espaço purgável" em macOS e semântica de `f_bavail` em Linux. O rótulo
  `source` do grupo é `statfs` nos dois casos; a diferença é do backend.
* Filtro de pseudo-fs funcionando: `map auto_home`, `devfs`, `tmpfs`,
  `OrbStack:` não aparecem; `apfs`/`hfs`/`ext4`/`xfs`/`overlay` aparecem
  (`crates/godmesh-host/src/volumes.rs`, lista `PSEUDO_FILE_SYSTEMS`).
* Em Linux, reproduzir a mesma comparação:

  ```
  host-evidence volumes > /tmp/volumes.txt
  df -k -P        # comparar total_bytes/available_bytes por mount point
  free -m         # comparar godmesh_host_memory_bytes com /proc/meminfo
  cat /proc/pressure/memory   # comparar godmesh_host_pressure_stalled_ratio
  ss -s                       # comparar godmesh_host_sockets / tcp_connections
  cat /proc/self/limits       # comparar process_descriptors (no processo do nó)
  ```

### 5.5 Degradação real, executada num host sem `/proc`

```
$ ./target/debug/host_evidence
host-evidence — relatório da coleta
  sistema operacional: macos
  fonte dominante: Statfs
  contêiner (cgroup próprio lido): false
  custo desta coleta: 0.082124 s
  grupos (fonte ou "none" = não medido):
    cgroup none    conntrack none    cpu none    descriptors none
    disk statfs    memory none    netdev none    pressure none
    sockets none    tcp none
  resumo de capacidade (/healthz): cpu_pressao=None memoria_pressao=None disco_livre=Some(0.000611)
```

O `/metrics` correspondente contém **apenas** o painel de disponibilidade mais
os volumes; nenhuma série de CPU/memória/rede é publicada (teste
`smoke_no_proc_real_quando_o_host_for_linux`, que roda este caminho). É o
contrário de zerar: o painel diria "CPU 0 %", e isso leria "host saudável".

### 5.6 Contêiner: número do namespace ≠ número do host

Executado sobre a árvore de fixture `container` (mesmo `Collector`, raiz
diferente) — é o par de números que a fatia exige registrar:

| Grandeza | `/proc` (o que este namespace vê) | cgroup próprio (o limite do namespace) | Diferença |
|---|---|---|---|
| CPU | `cpu_cores = 4` | `cgroup_cpu_limit_cores = 2` | o nó tem 2× a quota de `/proc` |
| Memória | `memory_bytes{state="used"} = 8 388 608 000` | `cgroup_memory_current_bytes = 734 003 200` | 11,4× |
| Limite de memória | `memory_bytes{state="total"} = 33 554 432 000` | `cgroup_memory_limit_bytes = 1 073 741 824` | 31,3× |
| Pressão `cpu some avg10` | 12,5 | 30,0 | o resumo do `/healthz` usa **30,0** em contêiner |
| Throttling | (não existe em `/proc/stat`) | `throttled_usec = 5 000 000` → 5 s | só o cgroup prova quota estourada |

Asserções: `container_reporta_cgroup_e_deixa_claro_que_o_numero_do_namespace`,
`resumo_de_capacidade_usa_a_pressao_do_namespace_em_container` (crate
`godmesh-host`) e `healthz_da_capacidade_sem_julgar_pressao` (crate
`godmesh-server`, `cpu_pressure = 30`, não 12,5).

**Em que sentido isso é evidência:** prova que o coletor lê o namespace quando o
namespace existe, e que as duas leituras não são confundidas. **Não** prova o
comportamento num contêiner Linux real (§6).

### 5.7 Cardinalidade contada, antes e depois

```
$ cargo test -p godmesh-server --test host_telemetry -- --nocapture
scrape: 228 linhas totais | antes (aplicação): 19 séries | agora (host): 127 séries
```

* Painel de aplicação (esta fatia não o alterou): 19 séries — o texto de
  aplicação é o mesmo `Metrics::render` anterior, só passa a ser seguido da
  seção de host.
* Painel de host: **127 séries** no scrape acima (fixture `linux`, 3
  interfaces, 2 volumes, 4 estados TCP).
* Teste de teto (`cardinalidade_medida_antes_e_depois`): 64 interfaces
  publicadas na fixture → 16 servidas + `cardinality_truncated{type="interface"} 1`,
  e o total de séries fica em **227**, com asserção `<= 240`. Sem teto, 64
  interfaces custariam 512 séries só de contadores de interface.
* Teste de formato (`exposition_valido_para_host_e_container`,
  `assert_sintaxe_prometheus`/`assert_types`): toda linha de série é
  `nome{rótulos} valor` válido, toda família tem `# TYPE` antes da primeira
  linha, nenhum `TYPE` duplicado, nenhum nome fora do prefixo, nenhuma família
  escrita sem série.
* Teste de rótulo proibido
  (`nenhum_rotulo_parece_identificador_de_conexao_ou_tenant`): nenhum `peer=`,
  `tenant=`, `connection=`, `remote=`, `port=`, `inode=`, `pid=`, `addr=`, `uid=`.

### 5.8 Rodada completa (round trip) da árvore

`tree::write` copia para disco exatamente os caminhos que o coletor lê;
`proc-fs DIR` roda o **mesmo** `Collector` sobre a cópia. O teste
`arvore_copiada_renderiza_identica` asserta **igualdade byte a byte** do
exposition (removidas apenas as duas séries de tempo relativo), para as
fixtures `linux` e `container`. É a peça que permite gerar a árvore num nó Linux
(`tree-write`), conferir com `free`/`df`/`ss`/`top` no mesmo nó, e reproduzir o
número no painel sem privilégio nenhum.

---

### 5.9 Binário real servindo o painel (execução manual, 18/11/2026)

`godmesh-server` de verdade, com Postgres real (`godmesh_h1`, já migrado pelas 8
migrações), `GODMESH_GRANTS` com um token de escopo `admin:metrics`, e
**`GODMESH_HOST_PROC_ROOT=/tmp/godmesh-proc-fs`** — uma cópia `cp -a` da árvore
de `/proc` de fixture. Linha de subida literal:

```
godmesh-server: escutando em http://127.0.0.1:18099 (1 grant(s) configurado(s))
godmesh-server: federação desligada (GODMESH_FEDERATION_BIND ausente); diagnóstico de enlace responde unknown
godmesh-server: outbox publicador ativo a cada 1000 ms (sink de teste outbox_sink)
godmesh-server: telemetria de host ativa (camada 1, read-only): snapshot a cada 1000 ms, raiz /tmp/godmesh-proc-fs, /metrics com prefixo godmesh_host_
```

`GET /healthz` (público, como já era):

```json
{"ack_class":"transport_accepted","capacity":{"container":false,"cpu_pressure":1.25,
 "disk_free_ratio":0.0006113356920702119,"memory_pressure":6.0,
 "pressure_is_not_a_health_signal":true,"snapshot_age_seconds":0.0,"source":"proc"},
 "status":"ok"}
```

`GET /metrics`: **401** sem credencial; **200** com `admin:metrics`; 257 linhas
no scrape, das quais 32 séries de aplicação e **147 séries de host**, em 34
famílias. Amostra literal:

```
godmesh_host_metrics_available{source="proc"} 1      # /proc lido
godmesh_host_metrics_available{source="cgroup"} 0
godmesh_host_group_available{group="cgroup"} 0       # processo fora de namespace
godmesh_host_cpu_cores 4
godmesh_host_cpu_seconds_total{class="io_wait"} 4
godmesh_host_memory_bytes{state="used"} 6291456000
godmesh_host_pressure_stalled_ratio{resource="memory",kind="some",window="10",scope="host"} 6
godmesh_host_tcp_connections{state="listen"} 3
godmesh_host_conntrack_entries 1234
godmesh_host_sockets{protocol="tcp"} 17
godmesh_host_process_descriptors_usage_ratio 0.000008
godmesh_host_collection_duration_seconds 0.080176
godmesh_host_snapshot_age_seconds 0.128000
```

Dois resultados que valem mais que a lista acima:

* **`godmesh_host_cpu_usage_ratio` não apareceu.** A árvore copiada é estática:
  o delta de jiffies entre duas coletas é 0, e taxa derivada de 0 é ausência,
  não 0 %. O painel perdeu a série por um ciclo e continuou dizendo a verdade.
* **`disk_bytes` saiu com números reais deste host** (494 332 366 848 bytes em
  `/`), enquanto o resto veio da árvore copiada. Volumes **não** vêm de
  `/proc`: são `statfs` do processo, e a config de raiz não os afeta. É o
  exemplo mais claro da regra "cada série declara sua fonte": o mesmo snapshot
  mistura origem copiada e origem viva, e os rótulos `source`/`group_available`
  dizem qual é o quê.

Custo real observado com volumes vivos: **80 ms por coleta**
(`collection_duration_seconds 0.080176`) — dominado pela enumeração de volumes
via IOKit no macOS, não por `/proc`. Com volumes injetados, a mesma árvore custa
~1 ms (§5.1). No orçamento de 250 ms (5 % de um intervalo de 5 s) isso é 32 %;
com o intervalo de 1 s usado nesta execução, seria 8 % — razão para **não**
baixar o intervalo em macOS de desenvolvimento sem olhar
`collection_duration_seconds`.

Degradação com raiz inexistente (`GODMESH_HOST_PROC_ROOT=/tmp/nao-existe-proc`,
mesmo binário):

```
"capacity":{"cpu_pressure":null,"memory_pressure":null,"source":"statfs", ...},"status":"ok"
godmesh_host_metrics_available{source="proc"} 0
godmesh_host_metrics_available{source="statfs"} 1
godmesh_host_group_available{group="cpu"} 0     # e memory/pressure/tcp/conntrack/netdev/descriptors: 0
godmesh_host_disk_bytes{volume="/",state="total"} 494332366848
```

Nenhuma série de CPU/memória/rede foi publicada; o painel de disponibilidade
disse 0 em cada grupo; e o `/healthz` continuou 200 porque a dependência
(banco) respondia — exatamente o contrato da fatia.

Configuração inválida é recusada no boot, sem default silencioso (todos com o
binário real):

```
GODMESH_HOST_METRICS_INTERVAL_MS=0     -> "…0 significaria ler /proc em laço apertado, o que é proibido"
GODMESH_HOST_METRICS_INTERVAL_MS=abc  -> mesma recusa
GODMESH_HOST_METRICS_INTERVAL_MS=-3   -> mesma recusa
GODMESH_HOST_METRICS_INTERVAL_MS=1.5  -> mesma recusa
GODMESH_HOST_PROC_ROOT=caminho-relativo -> "…precisa ser um caminho absoluto"
```


## 6. O que **não** foi provado

1. **Conferência com `top`/`free`/`ss`/`vmstat` num Linux real: NÃO EXECUTADA.**
   O ambiente desta thread é macOS/arm64, sem `/proc`, sem máquina Linux
   disponível, e a instrução da fatia proíbe Docker. As fórmulas de parse foram
   conferidas **à mão** contra o formato documentado de `proc(5)` (jiffies ÷
   100 Hz, kB × 1024, colunas de `net/dev`, `st` em hex, `inuse`/`mem` de
   `sockstat`) e **por teste de fixture com o valor esperado escrito**; a única
   conferência contra ferramenta de sistema executada foi `df -k` ×
   `statfs`/`sysinfo` neste host (§5.4). O `host-evidence proc-fs <dir>` existe
   para fechar essa lacuna num nó Linux com um comando (§8).
2. **Contêiner Linux real (cgroup v2): NÃO EXECUTADO.** A diferença
   namespace × host está provada por fixture com a árvore completa de cgroup
   (§5.6); o formato dos arquivos de cgroup é o de `cgroups(7)`
   (`memory.current`, `memory.max`, `cpu.max`, `cpu.stat`, `*.pressure`), e o
   caminho é resolvido de `proc/self/cgroup` + `proc/self/mounts`. Num
   Kubernetes/OrbStack, o operador deve rodar:
   `host-evidence dump` dentro do contêiner e comparar com
   `cat /sys/fs/cgroup/memory.current` e `cat /sys/fs/cgroup/cpu.max`.
   **cgroup v1**: coberto por fixture própria (`container-v1`) e por execução
   nos testes — `container_v1_le_memoria_e_nao_confunde_sentinela_com_limite`
   lê `memory.usage_in_bytes`/`memory.limit_in_bytes` pela subtree
   `/sys/fs/cgroup/memory` (caminho escolhido por `cgroup_memory_path`, nunca
   "a primeira linha que aparecer") e traduz a sentinela de "sem limite"
   (`9223372036854771712`) em **ausência**, não em limite de 8 EiB. Em v1
   **não** é lida a quota de CPU: ela mora em `/sys/fs/cgroup/cpu*`, outra
   subtree, com outro caminho por controller — a série
   `godmesh_host_cgroup_cpu_limit_cores` fica ausente (declarado na `HELP`), o
   que não significa "sem quota". Os arquivos `*.pressure` por cgroup também
   não existem em v1, então o resumo do `/healthz` cai no número de
   `/proc/pressure`. Nada disso foi validado num host v1 real.
3. **Inodes por volume: NÃO PUBLICADO, por decisão declarada.** `statfs` devolve
   `f_files`/`f_ffree` (e `df -k` mostra `iused`/`ifree`), mas `sysinfo` 0.40.4
   não expõe esse par, `/proc` não tem substituto, e chamá-lo direto é `unsafe`
   (proibido no workspace). Adicionar um crate novo de FFI para dois números não
   passou no critério de "mínimo necessário" da tarefa. A série está **ausente**
   e a `HELP` de `disk_usage_ratio` diz isso. Consequência: um volume com inodes
   esgotados aparece como saudável nesta fatia.
4. **Netlink (conntrack via `sock_diag`/`NF_NETLINK_CONNTRACK`): NÃO USADO, por
   instrução da fatia.** Leitura por arquivo cobre count/limit; não cobre
   contagem por protocol/porta nem a tabela real — nada disso é publicado.
5. **Leituras que exigem privilégio** (declaradas, degradadas, não simuladas):
   * `/proc/sys/net/netfilter/nf_conntrack_max` — em kernels com `0440
     root:conntrack` um processo sem esse grupo **não lê**; o painel então
     publica `conntrack_entries` e **omite** `_limit` e `_usage_ratio`, com
     `group_available{group="conntrack"} 1` (a contagem existe). Teste coberto:
     `conntrack_ilegivel_degrada_o_grupo_e_nao_publica_zero`.
   * `/proc/pressure/*` — precisa do kernel com `CONFIG_PSI=y` (habilitado por
     `psi=1` na cmdline em alguns kernels). Sem o arquivo: grupo ausente, não 0.
   * `/proc/net/{tcp,tcp6,sockstat,dev}` são mundiais; `/proc/self/*` são do
     processo. Nada nesta fatia exige root.
   * `statfs` de um volume montado por outro usuário pode falhar em filesystems
     com permissões restritivas — o volume simplesmente não sai.
6. **Pressão como critério de readiness: NÃO implementado, de propósito.** Um
   snapshot com CPU a 100 % de stall responde **200** no `/healthz`, e o corpo
   afirma `"pressure_is_not_a_health_signal": true`. Teste:
   `healthz_da_capacidade_sem_julgar_pressao` (`cpu_pressure = 30` com 200). A
   camada 2 de ADR-073 (limiar + histerese + classes + reserva SOS) precisa de
   parâmetros aprovados antes de existir.
7. **Custo sob carga de produção, latência de scrape em incidentes, e impacto
   do `statfs` num host com muitas montagens remotas: não medido.** O custo
   medido é o de um snapshot nesta máquina, com fixture local e montagens locais
   (§4). O teto declarado (5 % do intervalo) é uma hipótese verificável, não uma
   medição de produção.
8. **Estabilidade do formato entre versões de kernel:** o parse tolera colunas
   novas (ignora o que não conhece) e recusa o que contradiz o formato
   (`Malformed` → grupo indisponível). Não foi executado contra múltiplos
   kernels.

---

## 7. Segurança e privacidade

* `/metrics` continua atrás de credencial **e** escopo `admin:metrics` (401 sem
  token, 403 sem escopo). Teste no fio: `metrics_publica_host_pela_rota_autenticada`.
* Nenhuma série carrega endereço, porta, peer, tenant, `token_id`, PID alheio,
  inode ou nome de socket — teste `nenhum_rotulo_parece_identificador_de_conexao_ou_tenant`.
* Nomes de `volume` (ponto de montagem) e de `interface` vêm do kernel e são
  topologia interna; por isso ficam atrás do mesmo escopo que já protege volume
  de tráfego e backlog de outbox, e por isso o teto de 16+16 séries existe.
* Nenhum segredo em código, log, fixture ou doc: as fixtures são números
  sintéticos; a URL de banco deste relatório é a de desenvolvimento já pública
  em `deploy/README.md`.
* A camada 1 não abre nenhuma superfície de escrita: não há syscall de
  configuração, não há `setrlimit`, não há `iptables`/`nft`, não há netlink de
  alteração.

---

## 8. Configuração

| Variável | Default | Recusa |
|---|---|---|
| `GODMESH_HOST_METRICS_INTERVAL_MS` | `5000` | não inteiro, `0`, negativo (laço apertado lendo o kernel = DoS autoinfligido) |
| `GODMESH_HOST_PROC_ROOT` | `/` | caminho relativo |

Idade máxima tolerada de um snapshot: `3 × intervalo` (15 s com default). Acima
disso o `/metrics` serve `metrics_available{source="none"} 0` e **nenhuma
série de host** — número velho sem aviso é pior que nenhum número. Testes:
`snapshot_velho_servido_como_indisponivel`,
`metrics_section_de_snapshot_velho_e_declarado_indisponivel`.

Não há chave para desligar a coleta: ADR-073 define a camada 1 como *sempre
ligada*, e uma leitura que um operador pode desligar não é um sinal de
capacidade confiável.

## 9. Rastreabilidade

* Requisito: ADR-073 camada 1 (observar), catálogo C13-SIG-019/C13-SIG-020 de
  `docs/05-operacao/observabilidade.md` § "Perfil de recurso e capacidade".
* Decisão normative: ADR-073 (fontes, unidades, janela, "declarar namespace no
  rótulo", cardinalidade agregada).
* Implementação: `crates/godmesh-host` + `crates/godmesh-server/src/{http/mod.rs,main.rs}`.
* Testes: `crates/godmesh-host/tests/{parsers,collect_fixtures,exposition,cost}.rs`
  (26 + 18 + 7 + 4) + 9 unitários da lib = **64** na crate nova, e
  `crates/godmesh-server/tests/host_telemetry.rs` (7) no servidor:
  **71 testes novos** nesta fatia.
* Evidência: este arquivo. **Estado: implementação + testes executados em
  ambiente de desenvolvimento; conferência com `top`/`free`/`ss` em Linux e
  validação em contêiner não executadas (§6).**
