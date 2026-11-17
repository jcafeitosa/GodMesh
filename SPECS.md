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

## Distribuição canônica das 21 SPECs

Captura de 16/11/2026 do mapa CONSOLIDACAO-CANONICA.md, mantido por Alan. Atribuição não comprova entrega, cobertura, revisão ou aprovação. Arquivo ausente é exibido sem link.

| Arquivo / ID | Autor exclusivo | Recorte / interface | Estado observado |
|---|---|---|---|
| [SPEC-001-arquitetura-federacao.md](docs/08-specs/SPEC-001-arquitetura-federacao.md) | Martin Fowler | núcleo, arquitetura, federação e compatibilidade; Erich |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-002-identidade-autorizacao.md](docs/08-specs/SPEC-002-identidade-autorizacao.md) | Kim Cameron | identidade, PKI, grants e recuperação; Bruce |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-003-gmp-api.md](docs/08-specs/SPEC-003-gmp-api.md) | Graydon Hoare | GMP, API, erros, versionamento; Erich |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-004-eventos-sincronizacao.md](docs/08-specs/SPEC-004-eventos-sincronizacao.md) | Martin Kleppmann | outbox/inbox, replay, fence, B06/B12; Leslie |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-005-dados-persistencia.md](docs/08-specs/SPEC-005-dados-persistencia.md) | Michael Stonebraker | dados, isolamento, cópias, retenção; Momjian |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-006-sos-assistencia.md](docs/08-specs/SPEC-006-sos-assistencia.md) | Nancy Leveson | SOS, estados, ACK, segurança operacional; Edsger |  DRAFT-0.9; revisão independente desta versão pendente |
| [SPEC-007-voz-ptt-midia.md](docs/08-specs/SPEC-007-voz-ptt-midia.md) | Radia Perlman | floor/PTT, mídia, RNF-007; Vint |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-008-transcricao-asr.md](docs/08-specs/SPEC-008-transcricao-asr.md) | Daniel Povey | ASR consentido, revogação e retenção; Bruce |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-009-wifi.md](docs/08-specs/SPEC-009-wifi.md) | Jouni Malinen | Wi-Fi, estados, auditabilidade; Richard |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-010-integracoes-bridges.md](docs/08-specs/SPEC-010-integracoes-bridges.md) | Gregor Hohpe | UDNL, adaptadores, bridges; Erich |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-011-localizacao-geoespacial.md](docs/08-specs/SPEC-011-localizacao-geoespacial.md) | Paul Ramsey | geoespacial, frescor e privacidade; Bruce |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-012-produto-guardian.md](docs/08-specs/SPEC-012-produto-guardian.md) | Margaret Hamilton | RF-020/027/028 parciais; jornadas, contexto e Guardian; Ada |  DRAFT-0.10; revisão independente desta versão pendente |
| [SPEC-013-experiencia-plataformas.md](docs/08-specs/SPEC-013-experiencia-plataformas.md) | Lea Verou | UX, acessibilidade, desktop Windows/Linux/macOS e web; James |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-014-cliente-offline-ffi.md](docs/08-specs/SPEC-014-cliente-offline-ffi.md) | Alex Crichton | núcleo offline e fronteiras FFI; Donny/Chet |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-015-android-kotlin.md](docs/08-specs/SPEC-015-android-kotlin.md) | Chet Haase | contrato Android/Kotlin, lifecycle, permissões, bindings e OPEN; Alex |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-016-ios-swift.md](docs/08-specs/SPEC-016-ios-swift.md) | Donny Wals | iOS, Swift, lifecycle e permissões; Alex |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-017-dispositivos-firmware.md](docs/08-specs/SPEC-017-dispositivos-firmware.md) | Jack Ganssle | dispositivo, firmware, watchdog e limites; Vint |  DRAFT-0.4; revisão independente desta versão pendente |
| [SPEC-018-operacao-observabilidade.md](docs/08-specs/SPEC-018-operacao-observabilidade.md) | Brendan Gregg | SLO, sinais, INT-ADA-001, falhas e restauração; James |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-019-implantacao-capacidade.md](docs/08-specs/SPEC-019-implantacao-capacidade.md) | Grady Booch | modalidades, carga, custo e failover; Leslie |  DRAFT-0.7; revisão independente desta versão pendente |
| [SPEC-020-seguranca-retencao.md](docs/08-specs/SPEC-020-seguranca-retencao.md) | Bruce Schneier | políticas, cópias e ameaças; Dan/Raphael |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-021-postgresql-recuperacao.md](docs/08-specs/SPEC-021-postgresql-recuperacao.md) | Bruce Momjian | backup/restore, rollback e migração PostgreSQL; Michael |  DRAFT-0.6; revisão independente desta versão pendente |
| [SPEC-022-produto-organizacoes.md](docs/08-specs/SPEC-022-produto-organizacoes.md) | Margaret Hamilton | conta, organizações, convites, papéis, encerramento; Ada |  DRAFT-0.5; revisão independente desta versão pendente |
| [SPEC-023-capacidades-conectividade.md](docs/08-specs/SPEC-023-capacidades-conectividade.md) | Gregor Hohpe | capacidades, evidência de conectividade, satélite, prioridade; Vint |  DRAFT-0.3; revisão independente desta versão pendente |
| [SPEC-024-crowdfind-beacon.md](docs/08-specs/SPEC-024-crowdfind-beacon.md) | Jack Ganssle | CrowdFind, Beacon rotativo, resolução e antirrastreamento; Bruce |  DRAFT-0.5; revisão independente desta versão pendente |

Cobertura semântica dos 60 requisitos permanece pendente de leitura das peças entregues. Não somar IDs como percentual de conclusão.

### Snapshot das versões entregues

Captura **regenerada em 17/11/2026** a partir dos 24 arquivos presentes. O hash identifica o conteúdo atual; não comprova revisão, aprovação nem cobertura. Parecer que cite hash antigo vale apenas para a versão citada. Registro detalhado: `09-equipe/registro-versao-e-revisao.md`.

| SPEC | SHA-256 (17/11/2026) |
|---|---|
| SPEC-001-arquitetura-federacao.md | `4753787b8b7c9118240545faa4ded6e424505f3b892b59a615ab5ce9f660a92b` |
| SPEC-002-identidade-autorizacao.md | `6376def4308bb98169dc257b189bb82afb6c8cee0e6f7efe8d72f09453e0fddc` |
| SPEC-003-gmp-api.md | `a5e85551ff71f0da38ccfaa6e20a4f8ab30952a7aec18d30703501ffd9133c15` |
| SPEC-004-eventos-sincronizacao.md | `ce350188aaf38da269804782b617eefc9b6398912fdcd8f1c14a1eb62d128b6f` |
| SPEC-005-dados-persistencia.md | `29a456dd7f48e381f7a7fa894466aac7894064def45e6d993841a9cbcc64cfc1` |
| SPEC-006-sos-assistencia.md | `2ebf945a397bce085b39f2e766fcfe4fe510ee15794136749566f04c29990ac3` |
| SPEC-007-voz-ptt-midia.md | `ef06ca98b10e1676883750e057a79bd33ad9961e361fe73986b3bcbbd1db029b` |
| SPEC-008-transcricao-asr.md | `6e195424984e8a076f20b0184daaa6ddb9d40958e30ade46aabe6adc1c8b4a70` |
| SPEC-009-wifi.md | `4347fa14443e9eb7c7434df10d9e30d58f290cd3897d4eb1c0c6575949e5acbe` |
| SPEC-010-integracoes-bridges.md | `ca5eb0bed0d2274c89c93f6bd4208dc414329b43decdbb8b0b1c2ebede211a0f` |
| SPEC-011-localizacao-geoespacial.md | `4c0fd8cbc21a34198ce94cfc267e249b2e57474c1bcac48a076020f84d1ead4d` |
| SPEC-012-produto-guardian.md | `271f2af78664de3f0f6fe65e8fed2e6974b9387530739b6eacb076bc43daa81d` |
| SPEC-013-experiencia-plataformas.md | `3e3735d23d3799fce928eaed6c284dd380ace7077caadc6e086b5983ad366db7` |
| SPEC-014-cliente-offline-ffi.md | `af4751bad0b71069217ef75f3651df264a66a7c7969342c56fa6787961459f3d` |
| SPEC-015-android-kotlin.md | `bbb04ec21cb4e7db95769ef8d689bca4364cccfb67c77382df3a780b3ef84b8e` |
| SPEC-016-ios-swift.md | `89153a9f1198740b97084d93ced1b7181187d6576815087c9a8e2872b9051d9c` |
| SPEC-017-dispositivos-firmware.md | `c92d5ab117b2a1993ba65675afdfa21cc676af1154a6bec33849bd41deb46d3d` |
| SPEC-018-operacao-observabilidade.md | `510276c5cc698f56fc899e7e3b6cb4701ae8f944faa57de7f8b8b018d16640a3` |
| SPEC-019-implantacao-capacidade.md | `45964aa15526c5d474ed9331ab8659b7a243e2321fc10696f7456ac14e6322dc` |
| SPEC-020-seguranca-retencao.md | `ed5f0943907e3908d56414f1a7ccc29ab0d951efe40aff8ff6ebf77d860f97b6` |
| SPEC-021-postgresql-recuperacao.md | `9b4624065d2969d73ecc937f31681235947bcfb7b47af91e55f3c550bcbd2332` |
| SPEC-022-produto-organizacoes.md | `e5c039bb7fabf0ac045c4e8f3294f22f915112ca55da141942eb5cd41d39e549` |
| SPEC-023-capacidades-conectividade.md | `3a36c63860f4e75eb5ca707aaec20024beb907926ec8c4b5baebcc2ed67ba207` |
| SPEC-024-crowdfind-beacon.md | `32b8c5285886e4daf6a3a8f4e6c48ba36fa24fc64837d149cdec2ab3378100e8` |
