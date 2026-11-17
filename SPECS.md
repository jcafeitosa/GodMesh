# GodMesh — SPECs do sistema integral

Estado: rascunho central em elaboração. Data: 16/11/2026. Responsável: Ada Lovelace; coordenação e crítica recíproca: Ada e Alan Turing.

Este documento organiza o catálogo e os critérios de conclusão das especificações do GodMesh completo. Não constitui aprovação das SPECs, implementação ou validação. A autoria avança em paralelo à resolução de pendências, conforme determinação do usuário; todas as SPECs devem ser aprovadas antes da implementação.

## Fontes e precedência

- [Documentação oficial](docs/README.md), com regras de prevalência.
- [Visão integral](docs/00-produto/visao.md) e [requisitos](docs/00-produto/requisitos.md).
- [Arquitetura normativa](docs/01-arquitetura/arquitetura-referencia.md) e [decisões](docs/06-planejamento/decisoes.md).
- [Pendências](docs/06-planejamento/pendencias.md) e [matriz de rastreabilidade](docs/06-planejamento/matriz-rastreabilidade.md).

Conflitos entre fontes devem ser registrados e conciliados; este catálogo não substitui decisões normativas. Contribuições externas ao repositório são insumos documentais e precisam de integração e revisão antes de serem tratadas como SPECs canônicas.

## Conteúdo obrigatório de cada SPEC

1. Identificador, versão, estado, autor, revisor e fontes com localizadores.
2. Objetivo, requisitos cobertos, escopo, plataformas e exclusões justificadas.
3. Decisões rastreáveis, alternativas, consequências, divergências e dependências.
4. Estados e transições, contratos, dados, erros, permissões e limites.
5. Timeout, cancelamento, repetição limitada, idempotência, offline, revogação e recuperação.
6. Segurança, isolamento, consentimento, retenção, compatibilidade e migração.
7. Critérios de aceite observáveis e casos planejados positivos, negativos, falhas e recuperação.
8. Pendências com responsável e condição de fechamento; evidências de revisão e aprovação.

Metas propostas não são medições. ACK técnico, persistência, recebimento pelo destino e atendimento humano permanecem distintos. Parâmetros desconhecidos ficam abertos, sem valores inventados.

## Catálogo de cobertura

Captura dos 43 requisitos funcionais e 17 não funcionais do índice central existente. “A conciliar” significa que o vínculo à versão canônica da SPEC ainda precisa ser conferido; não significa ausência de responsável ou de contribuição. “Aberto” não comprova cobertura semântica, revisão ou aprovação.

| Requisito | Descrição da fonte | SPEC canônica / versão | Aceite e teste planejado |
|---|---|---|---|
| RF-001 | Identidade para usuários, tenants, nós, dispositivos e gateways | A conciliar | Aberto |
| RF-002 | Ativos com múltiplos dispositivos e endpoints por transporte | A conciliar | Aberto |
| RF-003 | Perfis declarados, verificados e disponibilidade operacional | A conciliar | Aberto |
| RF-004 | Roteador que resolve serviço, destino, caminho, autorização e custo | A conciliar | Aberto |
| RF-005 | Voz nativa com PTT, grupos, prioridade e canais | A conciliar | Aberto |
| RF-006 | Protocolo GMP próprio, versionado, sem criptografia própria | A conciliar | Aberto |
| RF-007 | Mumble como bridge externa opcional, isolada | A conciliar | Aberto |
| RF-008 | FRN como bridge externa opcional, isolada e autorizada | A conciliar | Aberto |
| RF-009 | Localização, proximidade, histórico, mapa e geofencing | A conciliar | Aberto |
| RF-010 | ChirpStack/LoRaWAN, LoRa mesh, BLE e GPS/LTE por adaptadores | A conciliar | Aberto |
| RF-011 | Mensagens, presença, notificações, SOS, ACKs por etapa | A conciliar | Aberto |
| RF-012 | Vários servidores full-stack regionais com balanceamento | A conciliar | Aberto |
| RF-013 | Federação global e sync escopado, sem origem única permanente | A conciliar | Aberto |
| RF-014 | IPv6 preferencial e IPv4 fallback; extensibilidade futura | A conciliar | Aberto |
| RF-015 | Instalação/descoberta automáticas com ingresso autorizado | A conciliar | Aberto |
| RF-016 | Espelhamento de software e serviços; políticas de cópias de dados | A conciliar | Aberto |
| RF-017 | Descoberta autorizada de serviços externos locais, sem varredura indiscriminada | A conciliar | Aberto |
| RF-018 | SDK de bridges e adaptadores com isolamento de privilégios | A conciliar | Aberto |
| RF-019 | API pública, SDK de clientes e gerência de versões | A conciliar | Aberto |
| RF-020 | Sistema completo para Personal, Teams e Node | A conciliar | Aberto |
| RF-021 | Habilitação de funções sem acesso não autorizado a RF ou localização | A conciliar | Aberto |
| RF-022 | Criptografia E2EE onde suportada, sem falsa promessa em bridges | A conciliar | Aberto |
| RF-023 | GodMesh nativo independente de quaisquer servidores externos; bridges devem ser desconectáveis sem impactar os canais e serviços internos | A conciliar | Aberto |
| RF-024 | Transportes e versões evolutivos com negociação de capacidades, fallback seguro, conformidade e atualização gradativa | A conciliar | Aberto |
| RF-025 | Cada canal de voz oferece timeline de chat com transcrição autorizada, GMC ou ID externo preservado, marcadores parcial/final/correção e retenção controlada | A conciliar | Aberto |
| RF-026 | SOS multimodal independente de ASR, com autenticação, localização opcional, escalonamento autorizado, ACK de rede/dispositivo e humano separados, modo teste e auditoria | A conciliar | Aberto |
| RF-027 | Contexto operacional terra/ar/mar por atividade, sem inferência compulsória ou habilitação de RF por tipo presumido | A conciliar | Aberto |
| RF-028 | Guardian: convite, prazo, rastreamento autorizado temporário, revogação, check-in e aviso de atraso sem SOS automático | A conciliar | Aberto |
| RF-029 | Sensor fusion GNSS/IMU/Wi-Fi/BLE/UWB conforme hardware e APIs, com incerteza/frescor; sem falsas direções Wi-Fi 360° | A conciliar | Aberto |
| RF-030 | CrowdFind próprio com Beacon opcional e ID rotativo, sem depender de Find My/Find Hub e sem rastreamento não autorizado | A conciliar | Aberto |
| RF-031 | DICE monitora conectividade celular/IPv6/IPv4/local e classifica satélite somente com evidência permitida | A conciliar | Aberto |
| RF-032 | Sem automação disponível, orientar ação manual, sem mudar configurações de operadora, satélite ou aplicativos terceiros | A conciliar | Aberto |
| RF-033 | Em conexão restrita, priorizar SOS/check-in/mensagens e reduzir cargas não essenciais do próprio app, mantendo comunicação de emergência do SO | A conciliar | Aberto |
| RF-034 | Aplicativo normal com Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS; modos simultâneos e offline | A conciliar | Aberto |
| RF-035 | Wi-Fi AutoDiscover inventaria privadamente redes observáveis pelo SO, sem extração de senhas salvas ou cadastro de terceiros como públicos | A conciliar | Aberto |
| RF-036 | AutoShare apenas AP comprovadamente administrado, com consentimento, políticas, validade e revogação | A conciliar | Aberto |
| RF-037 | AutoConnect solicita associação pelo SO conforme plataforma, distinguindo sugestão, associação, autenticação e internet verificada | A conciliar | Aberto |
| RF-038 | AP físico registra coordenadas exatas disponíveis, qualidade, origem, pessoa que cadastrou/compartilhou, data e histórico de mudanças | A conciliar | Aberto |
| RF-039 | Mapa publica AP exato somente mediante opt-in específico do administrador comprovado; usuário/visitante não é exposto | A conciliar | Aberto |
| RF-040 | Auditoria de acesso Wi-Fi por usuário/GMC, AP, grant, sessão, duração e volume quando medido pelo controlador; sem histórico de navegação | A conciliar | Aberto |
| RF-041 | APIs/GMP de inventário, APs, compartilhamento, grants, provisionamento, sessões e mapa com escopo e idempotência | A conciliar | Aberto |
| RF-042 | Live Presence no iOS/Android com SOS/Guardian/PTT e estados reais, sem exposição de dados precisos na tela bloqueada | A conciliar | Aberto |
| RF-043 | Operação prioritária e energia adaptativa preservam comunicação essencial sem controlar apps de terceiros | A conciliar | Aberto |
| RNF-001 | IPv6-first sem prejudicar acessibilidade | A conciliar | Aberto |
| RNF-002 | Identidade estável e rotação segura de credenciais | A conciliar | Aberto |
| RNF-003 | Isolamento multi-tenant e localização consentida | A conciliar | Aberto |
| RNF-004 | Sincronização idempotente e recuperável | A conciliar | Aberto |
| RNF-005 | Alta disponibilidade com degradação explícita | A conciliar | Aberto |
| RNF-006 | Não saturar serviços críticos com tráfego opcional | A conciliar | Aberto |
| RNF-007 | RTP/mídia não transportada pelo event bus | A conciliar | Aberto |
| RNF-008 | Evolução de protocolos/firmwares interoperável | A conciliar | Aberto |
| RNF-009 | Rastreabilidade dos estados de SOS e comandos | A conciliar | Aberto |
| RNF-010 | Operação e compliance | A conciliar | Aberto |
| RNF-011 | Transcrição não afeta QoS da voz; política de privacidade e ASR/retention verificáveis | A conciliar | Aberto |
| RNF-012 | SOS mostra evidência do avanço por camadas e não promete despacho ou entrega | A conciliar | Aberto |
| RNF-013 | não publicar scans Wi-Fi nem senhas por padrão; provação de titularidade auditável. | A conciliar | Aberto |
| RNF-014 | distinção verificável entre `requested`, `os_associated`, `ap_authenticated` e `internet_verified`. | A conciliar | Aberto |
| RNF-015 | informações administrativas de AP e visitantes protegidas por função/tenant e retenção definida. | A conciliar | Aberto |
| RNF-016 | Live Activities/Updates não devem anunciar como confirmados estados sem evidência. | A conciliar | Aberto |
| RNF-017 | fluxo offline, revogação e disponibilidade de AP testados sob partição regional. | A conciliar | Aberto |

## Estados e liberação

Fluxo documental: rascunho → em revisão → revisada → aprovada. Pendências materiais impedem apresentar uma SPEC como pronta para implementar. Aprovação exige evidência identificável da versão e de quem a aprovou; autoria, parecer ou atividade não equivalem a aprovação.

A implementação autorizada segue SPEC aprovada e TDD: teste falhando pelo comportamento esperado → implementação mínima → teste passando → refatoração. Código e testes executados terão vínculos próprios e resultados observados; neste catálogo permanecem não verificados.

## Pendências desta consolidação

- Ada: conciliar cada requisito com arquivo e versão de SPEC, aceite, caso planejado e parecer específico.
- Alan: integrar contratos e decisões transversais com crítica de Ada; preservar o quadro único e os responsáveis existentes.
- Autores/revisores: resolver lacunas e vincular evidência ao conteúdo revisado antes da aprovação.
- RF-004 e RNF-007: Erich e Radia relataram complementos explícitos; o recebimento não fecha revisão, SPEC ou casos. Conferir os trechos antes de atualizar cobertura.
- Implementação e execução de testes: não verificadas nesta entrega documental.

## Origem e limites da captura

Índice de origem: `SPECS-EM-ANDAMENTO/INDICE-REQUISITOS-SPECS.md` (fonte externa não versionada, não incluída no repositório). Quadro de autoria e revisão: `MOBILIZACAO.md` (fonte externa não versionada). Esses caminhos locais exigem acesso ao ambiente compartilhado; ainda não foram incorporados ao repositório.

Conferência realizada: enumeração dos 60 IDs e existência dos links locais. Não houve validação integral dos contratos nem aprovação de SPECs. Apoio Jev não executado nesta transcrição determinística do catálogo.

## Consolidação integral solicitada em 16/11/2026

O usuário determinou fechar as pendências e criar todas as SPECs. `MANIFESTO-FONTES-SPECS.md` (fonte externa não versionada) registra os 33 arquivos de contribuição por hash e títulos observados; é captura verificável, não incorporação canônica nem aprovação. Alan integra os contratos e arquivos de SPEC; Ada mantém catálogo, cobertura e crítica. O fechamento exige vínculos por versão, aceite, teste planejado e parecer específico para os 60 requisitos, incluindo todas as plataformas. Ausências e decisões materiais permanecem abertas até evidência; a implementação depende do planejamento concluído e da aprovação integral.

## Distribuição canônica das 24 SPECs

Captura de 16/11/2026 do mapa CONSOLIDACAO-CANONICA.md, mantido por Alan. Atribuição não comprova entrega, cobertura, revisão ou aprovação. Arquivo ausente é exibido sem link.

| Arquivo / ID | Autor exclusivo | Recorte / interface | Estado observado |
|---|---|---|---|
| [SPEC-001-arquitetura-federacao.md](docs/08-specs/SPEC-001-arquitetura-federacao.md) | Martin Fowler | núcleo, arquitetura, federação e compatibilidade; Erich |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-002-identidade-autorizacao.md](docs/08-specs/SPEC-002-identidade-autorizacao.md) | Kim Cameron | identidade, PKI, grants e recuperação; Bruce |  DRAFT-0.9; revisão independente desta versão pendente |
| [SPEC-003-gmp-api.md](docs/08-specs/SPEC-003-gmp-api.md) | Graydon Hoare | GMP, API, erros, versionamento; Erich |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-004-eventos-sincronizacao.md](docs/08-specs/SPEC-004-eventos-sincronizacao.md) | Martin Kleppmann | outbox/inbox, replay, fence, B06/B12; Leslie |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-005-dados-persistencia.md](docs/08-specs/SPEC-005-dados-persistencia.md) | Michael Stonebraker | dados, isolamento, cópias, retenção; Momjian |  DRAFT-0.8; revisão independente desta versão pendente |
| [SPEC-006-sos-assistencia.md](docs/08-specs/SPEC-006-sos-assistencia.md) | Nancy Leveson | SOS, estados, ACK, segurança operacional; Edsger |  DRAFT-0.10; revisão independente desta versão pendente |
| [SPEC-007-voz-ptt-midia.md](docs/08-specs/SPEC-007-voz-ptt-midia.md) | Radia Perlman | floor/PTT, mídia, RNF-007; Vint |  DRAFT-0.8; revisão independente desta versão pendente |
| [SPEC-008-transcricao-asr.md](docs/08-specs/SPEC-008-transcricao-asr.md) | Daniel Povey | ASR consentido, revogação e retenção; Bruce |  DRAFT-0.8; revisão independente desta versão pendente |
| [SPEC-009-wifi.md](docs/08-specs/SPEC-009-wifi.md) | Jouni Malinen | Wi-Fi, estados, auditabilidade; Richard |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-010-integracoes-bridges.md](docs/08-specs/SPEC-010-integracoes-bridges.md) | Gregor Hohpe | UDNL, adaptadores, bridges; Erich |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-011-localizacao-geoespacial.md](docs/08-specs/SPEC-011-localizacao-geoespacial.md) | Paul Ramsey | geoespacial, frescor e privacidade; Bruce |  DRAFT-0.8; revisão independente desta versão pendente |
| [SPEC-012-produto-guardian.md](docs/08-specs/SPEC-012-produto-guardian.md) | Margaret Hamilton | RF-020/027/028 parciais; jornadas, contexto e Guardian; Ada |  DRAFT-0.12; revisão independente desta versão pendente |
| [SPEC-013-experiencia-plataformas.md](docs/08-specs/SPEC-013-experiencia-plataformas.md) | Lea Verou | UX, acessibilidade, desktop Windows/Linux/macOS e web; James |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-014-cliente-offline-ffi.md](docs/08-specs/SPEC-014-cliente-offline-ffi.md) | Alex Crichton | núcleo offline e fronteiras FFI; Donny/Chet |  DRAFT-0.8; revisão independente desta versão pendente |
| [SPEC-015-android-kotlin.md](docs/08-specs/SPEC-015-android-kotlin.md) | Chet Haase | contrato Android/Kotlin, lifecycle, permissões, bindings e OPEN; Alex |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-016-ios-swift.md](docs/08-specs/SPEC-016-ios-swift.md) | Donny Wals | iOS, Swift, lifecycle e permissões; Alex |  DRAFT-0.9; revisão independente desta versão pendente |
| [SPEC-017-dispositivos-firmware.md](docs/08-specs/SPEC-017-dispositivos-firmware.md) | Jack Ganssle | dispositivo, firmware, watchdog e limites; Vint |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-018-operacao-observabilidade.md](docs/08-specs/SPEC-018-operacao-observabilidade.md) | Brendan Gregg | SLO, sinais, INT-ADA-001, falhas e restauração; James |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-019-implantacao-capacidade.md](docs/08-specs/SPEC-019-implantacao-capacidade.md) | Grady Booch | modalidades, carga, custo e failover; Leslie |  DRAFT-0.10; revisão independente desta versão pendente |
| [SPEC-020-seguranca-retencao.md](docs/08-specs/SPEC-020-seguranca-retencao.md) | Bruce Schneier | políticas, cópias e ameaças; Dan/Raphael |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-021-postgresql-recuperacao.md](docs/08-specs/SPEC-021-postgresql-recuperacao.md) | Bruce Momjian | backup/restore, rollback e migração PostgreSQL; Michael |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-022-produto-organizacoes.md](docs/08-specs/SPEC-022-produto-organizacoes.md) | Margaret Hamilton | conta, organizações, convites, papéis, encerramento; Ada |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-023-capacidades-conectividade.md](docs/08-specs/SPEC-023-capacidades-conectividade.md) | Gregor Hohpe | capacidades, evidência de conectividade, satélite, prioridade; Vint |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-024-crowdfind-beacon.md](docs/08-specs/SPEC-024-crowdfind-beacon.md) | Jack Ganssle | CrowdFind, Beacon rotativo, resolução e antirrastreamento; Bruce |  DRAFT-0.6; revisão independente desta versão pendente |

Cobertura semântica dos 60 requisitos permanece pendente de leitura das peças entregues. Não somar IDs como percentual de conclusão.

### Snapshot das versões entregues

Captura **regenerada em 17/11/2026** a partir dos 24 arquivos presentes. O hash identifica o conteúdo atual; não comprova revisão, aprovação nem cobertura. Parecer que cite hash antigo vale apenas para a versão citada. Registro detalhado: `09-equipe/registro-versao-e-revisao.md`.

| SPEC | SHA-256 (17/11/2026) |
|---|---|
| SPEC-001-arquitetura-federacao.md | `78ce1a8b1bb44ac3498ea8c53880b0083a9ba004fc10e09126ffa2ea9b287110` |
| SPEC-002-identidade-autorizacao.md | `98ce3a947b44b2a103e8498105c752ffc1b41373e68cd078591c4a562b5e369d` |
| SPEC-003-gmp-api.md | `a5e85551ff71f0da38ccfaa6e20a4f8ab30952a7aec18d30703501ffd9133c15` |
| SPEC-004-eventos-sincronizacao.md | `ce350188aaf38da269804782b617eefc9b6398912fdcd8f1c14a1eb62d128b6f` |
| SPEC-005-dados-persistencia.md | `5f2972ff66f7bd510f1b0a85eb48a326929fc1891f109e3301223893bbb3a543` |
| SPEC-006-sos-assistencia.md | `8a6703aae83cad826e8a175dd88349ec1e30aacd0d296ac8f01656013a1073b5` |
| SPEC-007-voz-ptt-midia.md | `c713fa4ad8a9580c0224658da0490706b3dcad0e103ba02442920ae490ad614e` |
| SPEC-008-transcricao-asr.md | `2279bbbc7bd9843b5206203ee36cb312f9667f50ca9d430bd0708fa3847868a2` |
| SPEC-009-wifi.md | `3b0ca6f674b4b8105231e39e21e67cb65736f3fe3effc59312b94a3e275616be` |
| SPEC-010-integracoes-bridges.md | `2e38e9aa61adb01cd3e9d7d755359cce70832acb98653d9df3072674fbf5d873` |
| SPEC-011-localizacao-geoespacial.md | `6e68c172aa86b62fb79657d475bf1bdb32a339487c9cc351bde1a78b3599bbb4` |
| SPEC-012-produto-guardian.md | `6d4b4584fcd17f40432b5a1ec9898930c1972d767a40bef4392d1058ae5bf1ea` |
| SPEC-013-experiencia-plataformas.md | `2a0af707d04f40950be96e694da72f79efa23b71a7ac8f3d26e75685b6418478` |
| SPEC-014-cliente-offline-ffi.md | `a2e75a90583e623ec42bde132e6851f93fed24302cbaa4fe3705b2ed78c6dba7` |
| SPEC-015-android-kotlin.md | `18c1ab4a8be8a8669129e04663c73039be66118bb54ff7bb046d518ec7619cea` |
| SPEC-016-ios-swift.md | `0378d4b790c465676875055221b75cea01c6d8f21b4b9dc31a8da4eac4b341b1` |
| SPEC-017-dispositivos-firmware.md | `982d5096af36ee2e73c0bf87871fbc1d17724666b9e746f3ba33b6efe9336658` |
| SPEC-018-operacao-observabilidade.md | `be889e2a5405d270a28aa5b19123141b974b76511a80f87528c6e0f783e92b33` |
| SPEC-019-implantacao-capacidade.md | `8e64fd2037bfd5383b555ab32e6c0d9bbc5faf364ae61599404b683d286d3a77` |
| SPEC-020-seguranca-retencao.md | `6e90fe613044c61fdc35a910148117d5542446f74b90cb41163a9289ce900180` |
| SPEC-021-postgresql-recuperacao.md | `8d4b173bdb473acb98198570031160f4f96625ba8bef25ac9258e24b59c4d7ce` |
| SPEC-022-produto-organizacoes.md | `510961f2978f3ab7c563260f18da114affb305ce130e0665e2b88eb7f00b2713` |
| SPEC-023-capacidades-conectividade.md | `909370dc04bb1085c4b4a44eeb356fa345889fab93616653a27b8a7f345f8d2a` |
| SPEC-024-crowdfind-beacon.md | `c49f6a830211898f562228113417cb6f670b5b34acc0b672b2aa38f67be73cfa` |
