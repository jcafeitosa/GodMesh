# Histórico da documentação

- **18/11/2026 — ADR-075.** O usuário aprovou o planejamento, a revisão e a liberação da federação por fatia. O aceite de ponta a ponta deve envolver pelo menos três servidores.
- **18/11/2026 — M1B/I.** Integra decisão de ingresso assinada e ingest de eventos assinados. O aceite local verificou três instâncias QUIC/mTLS com queda e reconexão. [Evidência da integração](09-equipe/evidencias/2026-11-18-integracao-m1b.md)
- **18/11/2026 — ADR-076.** Registra a direção de certificado v2 portátil e revogação assinada de provas de ingresso. A M1C detalha a verificação de cadeia entre servidores.

## Porta do SEC1 concorrente: teto do extrator, `effect: unknown` e timeout 503 — 18/11/2026
- Três melhorias de `1799e51` portadas **sem** substituir a implementação integrada: a arquitetura de dois portões, os limites e os defaults testados permanecem.
- **Teto do extrator alinhado** (`DefaultBodyLimit::max(payload_ceiling())`): teto configurado acima de 2 MiB passa a valer de ponta a ponta; sem isto, o axum recusava corpo sozinho, fora do contrato `{error, detail, hint, ack_class}`.
- **Timeout declara `effect: unknown`** + `timeout_ms` + hint de reenvio com a mesma idempotência; todas as recusas do portão ganharam `hint` acionável (ADR-074).
- **Status do timeout redecidido 504 → 503** com citação da RFC 9110 §15.5.9/§15.6.4/§15.6.5: 504 exige papel de gateway/proxy com upstream (não é o caso); 408 pressupõe requisição incompleta (a nossa chegou inteira); 503 é a única definição que descreve incapacidade temporária de tratar a requisição e autoriza `Retry-After`. Emenda registrada na evidência anterior.
- Verificações: **331** testes (329 anteriores + 2 novos), 0 falha; clippy limpo; `run_all.sh` com 44 invariantes. Evidência: `docs/09-equipe/evidencias/2026-11-18-sec-porta-teto-extrator-e-timeout-503.md`.

## SEC1 integrada à malha: cotas, payload e admissão por classe sobre federação — 18/11/2026
- `6b21211` (cotas/admissão) foi reaplicado sobre o main **depois** da federação M1+M2 e do roteador dividido por domínio: `AppState` agora carrega `federation: Option<Arc<PeerLinks>>` **e** `admission: Arc<Admission>`, e `build_router` recebe os dois anexos.
- Novo construtor do binário: `http::router_with_federation_and_admission(pool, grants, federation, admission)`. Os cinco construtores públicos existentes (`router`, `router_with`, `router_with_log`, `router_with_federation`, `router_with_admission`) mantiveram a assinatura que os testes e o `--migrate-only` já usam.
- Ordem dos portões escrita no código e no doc do módulo: `observe → http_gate → require_grant → grant_gate → portão de escopo → handler`. O portão de cotas roda **antes** do de escopo: grant válido sem escopo também paga cota antes do 403.
- Rotas de federação têm classe declarada por rota: `POST /v1/federation/join-requests` (pública, sem credencial) entra em `sync`; leitura de pedidos/pares e **aprovação/rejeição** ficam em `incident` — a decisão administrativa precisa continuar alcançável sob pressão para estancar o abuso. Não existe classe `admin` na taxonomia aprovada.
- Boot valida `GODMESH_*` de cotas junto com identidade de nó e listener QUIC: `GODMESH_MAX_BODY_BYTES=0` → **exit 78** antes de escutar, sem default silencioso.
- Varredura: **329** testes passando (306 do main + 22 da SEC1 + 1 novo), 0 falha em **três rodadas** com listas de nomes idênticas; `clippy --workspace --all-targets` limpo; `migrations/tests/run_all.sh` com 44 invariantes; fumaça do binário com a malha ligada e limiar de pressão em 0: `event`/`sync`/`metrics` 503, SOS 201, `/v1/federation/*` 200, corpo de 70 kB 413. Invariante §4-2 colado na evidência. Evidência: `docs/09-equipe/evidencias/2026-11-18-integracao-sec1.md`.

## Telemetria de recursos do nó — camada 1 (observar) do ADR-073 — 18/11/2026
- Crate nova `godmesh-host`: coleta **read-only** de CPU (classes user/nice/system/idle/io_wait/irq/softirq/steal, load 1/5/15, threads do processo), memória e swap, pressão PSI de `/proc/pressure` **e** do cgroup próprio, discos por `statfs`, descritores e `Max open files`, sockets, conexões TCP agregadas por estado, conntrack por arquivo e contadores por interface. Nenhuma ação de escrita existe nesta camada: firewall, rota, `ulimit`, dreno e `kill` continuam fora (ADR-073 camadas 2 e 3).
- `/metrics` ganha a seção `godmesh_host_*` com unidade no nome, cardinalidade capada (≤16 volumes, ≤16 interfaces, vocabulário fechado nos demais rótulos) e o par `godmesh_host_metrics_available{source="proc"|"cgroup"|"statfs"|"none"}` + `godmesh_host_group_available{group=…}` — **série ausente não é zero**, e a família ausente nem sequer é escrita.
- `/healthz` passa a publicar `capacity` (`cpu_pressure`, `memory_pressure`, `disk_free_ratio`, `source`, `container`, `snapshot_age_seconds`) com `null` para o que não foi medido, e `pressure_is_not_a_health_signal: true`: pressão **não** derruba o nó nesta fatia.
- Coleta em background (`GODMESH_HOST_METRICS_INTERVAL_MS`, default 5 s; `GODMESH_HOST_PROC_ROOT`), handler serve o último snapshot. Custo medido: ~1 ms por snapshot com volumes injetados, 63 ns por leitura no scrape; orçamento declarado de 5 % do intervalo.
- `sysinfo` 0.40.4 (feature `disk`) é a única dependência nova — espaço livre de filesystem é `statvfs(2)`, `/proc` não o expõe e a chamada direta é `unsafe`. `procfs` 0.18.0 e `if-addrs` 0.15.0 foram **confirmados** como latest estável e **não usados** (parse próprio sobre `proc(5)`, contadores de `/proc/net/dev`). Inodes por volume não são publicados: a dependência não expõe `f_files`/`f_ffree`; lacuna declarada.
- 71 testes novos: `godmesh-host` 9 unitários + 26 de parsers + 18 de coleta sobre fixture + 7 de exposição + 4 de custo, e 7 no fio com Postgres real em `godmesh-server`. Varredura do workspace: 377 testes passando, 0 falha, 1 ignorado (teste de política já marcado); `clippy --workspace --all-targets -- -D warnings` limpo. O que **não** foi provado (conferência com `top`/`free`/`ss` em Linux e contêiner real, execução com netlink, cgroup v1 com quota de CPU) está listado em `docs/09-equipe/evidencias/2026-11-18-host-telemetria.md` §6.
- Documentação: `docs/05-operacao/observabilidade.md` § "Painel de host `godmesh_host_*`", `docs/01-arquitetura/servicos.md` (crate no catálogo), `docs/06-planejamento/decisoes.md` ADR-073 (registro das versões confirmadas e do que entrou de fato). Integração no main: `AppState` ganhou `host: HostTelemetry` mantendo `admission`/`federation`/registro de contrato; nova porta `http::router_runtime_with_host` (o binário a usa); `/metrics` e `/healthz` continuam registrados no `Registry` e o bloco `capacity` entrou no schema `HealthResponse` do contrato.

## Federação reconciliada: M1 + M2 em um crate — 18/11/2026
- `godmesh-federation` passa a ter **uma** definição de `NodeIdentity`, `SignedManifest` e `PeerRecord` — a de M1 (`gmn_` + `sha256(pubkey)`, forma assinada TLV, registro em `federation_peer`), já usada pela migração `0008` e pelas rotas de ingresso. O transporte de M2 (QUIC/mTLS, handshake, heartbeat, diagnóstico de enlace) foi adaptado a esses tipos; `registry.rs` de M2 foi absorvido por `peer.rs`.
- `SignedManifest` ganhou o campo assinado `alpn` (extensão de M1, `serde(default)`) e o envelope de fio `to_frame`/`from_frame` com teto de 8 KiB. `trust_state` **saiu** do manifesto: nó não declara a própria autorização; quem decide é o registro (ADR-071 emenda §1/§5, ADR-072 §2 invariantes 6/8).
- Servidor carrega a `NodeIdentity` no boot (`GODMESH_NODE_KEY`, `0600`, falha alta), abre o listener QUIC quando `GODMESH_FEDERATION_BIND` está definida (desligado por default) e mantém a allowlist e a discagem dos pares autorizados recarregadas de `federation_peer`.
- Varredura: 306 testes passando, 0 falha; `clippy --workspace --all-targets` limpo; `migrations/tests/run_all.sh` com 44 invariantes (inclui `0008`); fumaça com dois processos e `ready` → `unknown`. Evidência em `docs/09-equipe/evidencias/2026-11-18-integracao-federacao.md`.
- `deploy/`: `GODMESH_NODE_KEY` passa a ser obrigatória para servir e a imagem cria `/var/lib/godmesh` (`0700`); `GODMESH_FEDERATION_*` documentadas.

## Distribuição de configuração aprovada — 18/11/2026
- Emenda à ADR-072: quatro namespaces com autoridades distintas, pull assinado com quórum e anti-rollback, fronteira do que nunca cruza, ordem M1→M5 vinculante.

## Rede-alvo aprovada — 18/11/2026
- ADR-072 adota o desenho de cinco planos (entrada, controle, dados, mídia, borda) como alvo de arquitetura de rede, com dez invariantes testáveis e ordem de implementação: identidade → ingresso assinado → transporte → sync escopado → mídia → borda.
- Documento: `docs/09-equipe/rede-alvo-cto.md` promovido de proposta a direção aprovada; itens normativos derivados continuam exigindo ADR própria.
- Emenda à ADR-071: quem autoriza um nó é nó já autorizado, com aprovação assinada, cadeia verificável, revogação com epoch e bootstrap por genesis declarado.

## Outbox publicador durável — 18/11/2026
- Migração `0003_outbox_publisher.sql`: `outbox_entry.attempts` (contador de falhas do efeito), `CHECK outbox_entry_confirmed_requires_published` (confirmação sem publicação é impossível no banco), índice parcial de pendências e a tabela de **teste** `outbox_sink` (não é integração externa).
- `godmesh-sync::pg_outbox` implementa o contrato da ADR-061 sobre a tabela: append transacional com lock de grupo, reivindicação de publicação atômica (`FOR UPDATE SKIP LOCKED` + `UPDATE … RETURNING`), confirmação só após publicação e só na ordem do prefixo, contagem de tentativas e leitura de pendências que para na primeira sequência ausente.
- `godmesh-server::publisher` liga o laço tokio (`GODMESH_OUTBOX`, `GODMESH_OUTBOX_INTERVAL_MS`, default 1000 ms) e loga publicados/confirmados/falhas. Evidência em `docs/09-equipe/evidencias/2026-11-18-servidor-outbox.md`.
- `migrations/tests/run_all.sh` passa a aplicar a 0003 e a exigir 6 blocos `OK` novos: total 20 invariantes.


## Versões latest e documentação oficial — 17/11/2026
- ADR-069: versões latest estáveis somente, com versão resolvida, fonte, data e digest de imagem registrados; documentação oficial da versão é pré-requisito de implementação; atualização de dependência exige nova resolução e nova evidência.
- `AGENTS.md`: nova seção "Versões e documentação" com a mesma regra vinculante.
- `09-equipe/EQUIPE.md` §8: nenhum pod ou worker implementa por memória de API; o CTO orquestra, e decisão e gate são dele.
- Risco de breaking change em `latest` mitigado por lockfile e testes de contrato. Nada instalado, nada compilado.

## Cliente único em tecnologia web — 17/11/2026
- ADR-068: React 19 + Astryx + shadcn/ui + Tailwind, empacotado com Tauri 2 (macOS, Windows, Linux, iOS, Android) e núcleo Rust embarcado via comandos/eventos tipados; plugins nativos só onde o SO não expõe API web. **Supera a ADR-055.**
- Emenda à ADR-067: shadcn/ui permanece, com regra de convivência (um componente, um dono; tema único; fronteira `web/ui/`; remoção quando o Astryx cobrir o padrão).
- Versões resolvidas: Tauri CLI/API 2.12.5, Astryx 0.7.16, Tailwind 4.3.5, React 19.3.1, Next 16.4.6, TypeScript 7.0.2, StyleX 0.19.2.
- Pendente: novas versões de SPEC-013, SPEC-014, SPEC-015 e SPEC-016 no modelo web+Tauri; ajuste de SPEC-007/012/024 para plugin nativo. Nada instalado, nada compilado.

## Stack de interface web — 17/11/2026
- ADR-067: Astryx (`@astryxdesign/core` 0.7.16, MIT, React 19+) como base de componentes; Tailwind 4.3.5 para utilitários e layout via `className`; tema por CSS custom properties.
- coss.com e alignui.com rebaixados a referência de padrão visual/composição; coss.com registrado também como candidato de integração de API (e-mail/SMS/forms/webhooks), não como UI.
- shadcn/ui não adotado como base nesta decisão; convivência depende de ratificação do usuário.
- Versões resolvidas em 17/11/2026 por `npm view`: core/CLI Astryx 0.7.16, StyleX 0.19.2, React 19.3.1, Next 16.4.6, TypeScript 7.0.2. Nada instalado, nada compilado.

## Decisões do usuário e fechamento de gaps — 17/11/2026
- ADR-064: SOS offline negado quando não há fonte de frescor confiável (F1/F2) e não há verificação online; UI declara o estado antes do incidente.
- ADR-065: política inicial de retenção por classe (mensagem 24 m, posição 30 d, áudio não retido, SOS 12 m, auditoria 12/24 m, backup 35 d), com revisão jurídica por jurisdição e redução permitida por tenant.
- ADR-066: operação mundial com região primária Brasil (São Paulo) e secundária Colômbia (Bogotá); residência declarada por tenant e contrato por país.
- Emendas: ADR-060 (enum `confidentiality` e herança transitiva), ADR-063 (`generation` + piso de versão contra downgrade).
- SPECs 001–024: casos por decisão e casos por requisito com P/N/B/F; casos positivos dependentes de parâmetro OPEN passam a esperar `param_not_configured`.
- Criados: SPEC-022 (produto/organizações), SPEC-023 (capacidades/conectividade), SPEC-024 (CrowdFind); contratos `autoridade-contratos.md` e `eventos-sincronizacao.md` v0.2.
- Primeiro commit do repositório registra esta base. Nenhuma SPEC aprovada, nenhum código, nenhum teste executado.

## Rodada de gaps e decisões — 17/11/2026
- Varredura completa por 8 frentes (produto/expansão, integrações, defeitos documentais, escala/resiliência, segurança/conformidade) e 3 debates técnicos com fonte primária.
- Registradas ADR-056 a ADR-063: N/N-1 por perfil com rejeição antes do efeito; posição canônica de evento e cursor; taxonomia de ACK por fronteira; escopo do grant offline; E2EE nativo e proibição de alegar confidencialidade indevida; fronteira transacional e recuperação; retenção fail-closed; baseline de cadeia de suprimento.
- Criado `09-equipe/registro-gaps.md` com 113 itens classificados, dono, severidade e ordem de execução; decisões de usuário isoladas com recomendação.
- Corrigido o snapshot de hashes de `SPECS.md` (15 de 21 divergiam), removidos caminhos absolutos de máquina, indexados `aceites-comportamentais.md` e `credenciais-crowdfind.md`.
- Nenhuma SPEC aprovada, nenhum código, nenhum teste executado.

## Organização da equipe — 17/11/2026
- Criado `09-equipe/EQUIPE.md`: orquestração, 8 pods de especialistas por domínio, 4 críticos independentes com veto, workers de integração/matriz/harness e implementadores condicionados ao gate.
- Registrados estado real do projeto, fluxo de trabalho, definição de pronto de SPEC, regras de evidência, escalonamentos abertos e roster de threads.
- Documento de organização: não aprova arquitetura, SPEC, implementação ou teste. Nenhum código foi escrito, nenhuma SPEC foi aprovada e nenhum teste foi executado.

## Atualização documental — plataformas de entrega
- Registrada ADR-055: servidor em contêiner Linux; macOS universal Apple Silicon/Intel com SwiftUI; Windows/Linux; iOS com SwiftUI; Android.
- Separadas decisões explícitas de propostas WinUI 3, Qt Quick e Kotlin/Jetpack Compose, com fontes oficiais e critérios ainda pendentes.
- Alinhados arquitetura, visão, serviços, implantação, roadmap, índice e rastreabilidade; preservado desenvolvimento local com dependências no OrbStack. Não houve implementação, instalação ou execução de testes.

## 3.8.0 — 15/11/2026
- Consolidada a experiência de operação normal do aplicativo: telas, modos simultâneos, inicialização, operação offline, energia adaptativa e privacidade.
- Especificada integração com Dynamic Island/Live Activities (iOS) e Android Live Updates/notificações, sem presença persistente ou dados privados no bloqueio.
- Concluída especificação lógica Wi-Fi AutoDiscover, AutoShare, AutoConnect, incluindo APIs, integração Kotlin/Swift/Router Agent, consentimento e segurança.
- Corrigida a publicação de pontos de acesso: coordenadas exatas no cadastro privado e **exatas no mapa somente mediante opt-in do administrador comprovado**, independente da regra dos 50 km de localização pessoal do chat.
- Criado registro de sessões Wi-Fi, distinguindo pedido de conexão, associação declarada pelo cliente, autenticação atestada pelo AP e consumo efetivamente medido; sem histórico de navegação.
- Acrescentados contratos GMP/REST, cenários de teste, requisitos RF-034 a RF-043, ADR-047 a ADR-054 e rastreabilidade. Não são funcionalidades entregues.


## 3.7.0 — 15/11/2026
- Consolidados DICE, Guardian, fusão de sensores, CrowdFind próprio e Beacon temporário com privacidade/consentimento.
- Formalizados ADR-042..046 e requisitos RF-027..033: contexto terra/ar/mar; acompanhamento voluntário; identidade BLE rotativa; monitoramento de operadora e contingência.
- Criada camada Resilient Connectivity com modo satélite por evidência, diferença entre Direct-to-Cell e internet Starlink por Wi-Fi, fallback offline e priorização de SOS.
- Regra de intervenção humana: só automatizar quando APIs oficiais permitem e usuário autorizou; orientar manualmente em todas as outras situações. Nenhum controle de aplicativos terceiros, de assinatura satelital ou de seleção de operadora.
- Acrescentados documentos de arquitetura, CrowdFind, privacidade, contratos e testes móveis; referências técnicas oficiais; links internos verificados.

## 3.5.2 — 15/11/2026
- ADR-041: localização opcional nos chats e registros PTT sempre aproximada (incerteza mínima de 50 km de raio); posição original nunca na timeline nem em bridges.
- SOS: localização mais precisa disponível, com qualidade e horário reais, apenas a atendentes designados e autenticados; canais, eventos, APIs e auditoria separados.
- Adicionados `04-seguranca/politica-localizacao.md` e `07-contratos/localizacao-privacidade.md`; atualizados SOS, transcrição, UDNL, eventos, contratos, segurança, ADRs e README.


## 3.5.1 — 15/11/2026
- Formalizada ADR-040: cada transmissão PTT gera entrada de timeline independente da transcrição; quando ASR falha, é desativado ou não reconhece a fala, o chat registra transmissor/origem verificada, horário, duração conhecida e motivo de indisponibilidade.
- Para rádios analógicos sem identidade verificável, registrar a atividade da estação/gateway sem atribuir a fala a uma pessoa; dados de atividade submetidos a ACL e retenção própria.
- Contratos de eventos, transcrição e serviço SOS alinhados a essa regra.

## 3.5.0 — 15/11/2026
- Formalizada a **Transcrição Multimodal por Canal** (ADR-038): chat cronológico de voz/texto, ASR separado do plano de mídia, GMC/identidade externa com proveniência, políticas de consentimento, retenção e E2EE.
- Formalizado **GodMesh SOS** (ADR-039): evento estruturado independente da transcrição, prioridade com filas reservadas, localização opcional, reconhecimento humano distinto de ACK de rede/dispositivo, escalonamento e modo teste.
- Criados documentos de transcrição, serviço SOS e contratos GMP/API; atualizados arquitetura, eventos, mídia, segurança, testes e requisitos.

## 3.4.0 — 15/11/2026
- Criada **Universal Data Normalization Layer (UDNL)**, padrão bidirecional para identidade externa, eventos, proveniência e projeção com perdas explícitas.
- A normalização preserva ID original, contexto da rede/servidor, evidência de confiança e não atribui GMC a perfis externos por semelhança.
- Reafirmada separação do plano de mídia PTT, evitando converter todos os frames em eventos genéricos.
- Atualizados índice, arquitetura de referência, gateway universal, eventos e ADR-037.

## 3.3.0 — 15/11/2026
- Definido **indicativo digital pseudônimo GMC** para identificar usuários sem divulgar identidade civil na comunicação nativa; GMI permanece como ID interno.
- Definida transmissão de identidade com `TalkStart` autenticado e referência efêmera nos frames, em vez de repetir identificador estável em cada pacote de áudio.
- Endereços `acct:`/WebFinger passam a ser aliases de descoberta **opt-in**; reforçadas proteção contra impersonação, correlação, replay e limitações de bridges externas.
- Criados capítulos [GMC](02-dominio/indicativos-digitais.md) e [transporte da identificação](01-arquitetura/identificacao-midia-mensagens.md), atualizados contratos, índice, GMP, mídia nativa e ADR-035/036.

## 3.2.0 — 15/11/2026
- Formalizado **GodMesh Global Identity (GMI)**: UUIDv7/URN permanente; nome público no padrão `acct:apelido@domínio`, WebFinger, OpenID Connect (`iss` + `sub`) e passkeys/WebAuthn.
- Estabelecida distinção entre identidade, alias, autenticação e marca GodMesh verificada por federação, sem depender de e-mail, telefone, rádio ou endereço IP.
- Atualizados índice, modelo de dados, contratos, GMP, segurança, rastreabilidade, ADR-034 e referências.

## 3.1.0 — 15/11/2026
- Esclarecimento oficial de **tecnologia própria**: GMP, Voice, roteamento e federação são semântica e motores nativos do GodMesh **sobre padrões modernos e evolutivos**, sem reinventar transportes/criptografia/codecs.
- Mumble, FRN, ChirpStack e quaisquer sistemas externos são **apenas bridges/adaptadores opcionais**, isolados do caminho GodMesh↔GodMesh.
- Criada [Estratégia tecnológica](01-arquitetura/estrategia-tecnologica.md); revisados arquitetura de referência, protocolo, transporte, visão, requisitos, bridges, ADRs e rastreabilidade.
- Adoção de protocolos futuros condicionada a implementação, negociação, conformidade, testes, rollout e segurança.

## 3.0.0 — 15/11/2026
- Revisão integral do brainstorming; estabelecida arquitetura de referência normativa e sistema completo (sem definição por escopo reduzido).
- GodMesh Native e GMP são o núcleo; Mumble/FRN definitivamente opcionais por bridges.
- Requisitos, visão, glossário, roadmap, contratos, mídia e modelos alinhados à decisão.
- Especificada separação entre software espelhado, dados escopados, plano de mídia, autoridade de escrita e controle.
- Adicionadas matriz de rastreabilidade, gates de verificação e regras de prevalência documental.
- Mantidos como propostas os componentes ainda não implementados/validados.

## 0.2.0 — 15/11/2026
- Decidido desenvolver o **protocolo de aplicação GodMesh (GMP)** e um **motor PTT nativo em Rust**.
- Mumble e FRN passam de infraestrutura de voz principal a **bridges externas opcionais**.
- Criados documentos de arquitetura nativa e Bridge SDK; revisados índice e ADRs para preservar histórico sem ambiguidade.
- Registrados limites de E2EE em bridges, políticas anti-loop, PTT com leases/fencing e isolamento regulatório de rádio.


## 0.1.0 — 15/11/2026
- Registro inicial do brainstorming GodMesh.
- Consolidado núcleo Rust em Linux, plataforma consumer + enterprise, modelo de ativos/dispositivos/transportes e universal gateway.
- Documentadas malha mundial de servidores full-stack e operação active-active por região, descoberta, IPv6 preferencial, IPv4 fallback, replicação escopada e balanceamento.
- Selecionados Mumble e FRN como primeiras integrações de voz PTT; ChirpStack/LoRaWAN previsto para dados.
- Adicionados modelo de dados, contratos de eventos/API, requisitos, ADRs, roadmap, segurança, riscos e testes.
- Todos os elementos foram documentados como propostas/decisões de arquitetura, sem alegar implementação produtiva.
