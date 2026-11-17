# Requisitos integrais — rastreabilidade

> Estados: **APROVADO** = direção de produto/arquitetura; **ESPECIFICAR** = contrato ou operação ainda incompletos; **VALIDAR** = requer prova técnica ou revisão jurídica. Nenhum desses estados significa funcionalidade implementada.

| ID | Requisito | Estado | Referência |
|---|---|---|---|
| RF-001 | Identidade para usuários, tenants, nós, dispositivos e gateways | APROVADO | `04-seguranca/seguranca.md` |
| RF-002 | Ativos com múltiplos dispositivos e endpoints por transporte | APROVADO | `02-dominio/modelo-dados.md` |
| RF-003 | Perfis declarados, verificados e disponibilidade operacional | APROVADO | `02-dominio/capacidades.md` |
| RF-004 | Roteador que resolve serviço, destino, caminho, autorização e custo | APROVADO | `03-integracoes/gateway-universal.md` |
| RF-005 | Voz nativa com PTT, grupos, prioridade e canais | APROVADO | `01-arquitetura/midia-nativa.md` |
| RF-006 | Protocolo GMP próprio, versionado, sem criptografia própria | APROVADO | `01-arquitetura/protocolo-nativo.md` |
| RF-007 | Mumble como bridge externa opcional, isolada | APROVADO | `03-integracoes/bridges-voz.md` |
| RF-008 | FRN como bridge externa opcional, isolada e autorizada | APROVADO | `03-integracoes/bridges-voz.md` |
| RF-009 | Localização, proximidade, histórico, mapa e geofencing | APROVADO | `02-dominio/modelo-dados.md` |
| RF-010 | ChirpStack/LoRaWAN, LoRa mesh, BLE e GPS/LTE por adaptadores | APROVADO | `03-integracoes/lora-lorawan-ble.md` |
| RF-011 | Mensagens, presença, notificações, SOS, ACKs por etapa | APROVADO | `02-dominio/eventos.md` |
| RF-012 | Vários servidores full-stack regionais com balanceamento | APROVADO | `01-arquitetura/nos-regioes.md` |
| RF-013 | Federação global e sync escopado, sem origem única permanente | APROVADO | `01-arquitetura/replicacao.md` |
| RF-014 | IPv6 preferencial e IPv4 fallback; extensibilidade futura | APROVADO | `01-arquitetura/transporte.md` |
| RF-015 | Instalação/descoberta automáticas com ingresso autorizado | APROVADO | `01-arquitetura/descoberta-ingresso.md` |
| RF-016 | Espelhamento de software e serviços; políticas de cópias de dados | APROVADO | `01-arquitetura/nos-regioes.md` |
| RF-017 | Descoberta autorizada de serviços externos locais, sem varredura indiscriminada | ESPECIFICAR | `03-integracoes/gateway-universal.md` |
| RF-018 | SDK de bridges e adaptadores com isolamento de privilégios | APROVADO | `03-integracoes/bridges-voz.md` |
| RF-019 | API pública, SDK de clientes e gerência de versões | ESPECIFICAR | `07-contratos/api-inicial.md` |
| RF-020 | Sistema completo para Personal, Teams e Node | APROVADO | `00-produto/visao.md` |
| RF-021 | Habilitação de funções sem acesso não autorizado a RF ou localização | APROVADO | `04-seguranca/uso-das-redes.md` |
| RF-022 | Criptografia E2EE onde suportada, sem falsa promessa em bridges | ESPECIFICAR | `04-seguranca/seguranca.md` |
| RF-023 | GodMesh nativo independente de quaisquer servidores externos; bridges devem ser desconectáveis sem impactar os canais e serviços internos | APROVADO | `01-arquitetura/estrategia-tecnologica.md` |
| RF-024 | Transportes e versões evolutivos com negociação de capacidades, fallback seguro, conformidade e atualização gradativa | APROVADO | `01-arquitetura/estrategia-tecnologica.md` |

| RF-025 | Cada canal de voz oferece timeline de chat com transcrição autorizada, GMC ou ID externo preservado, marcadores parcial/final/correção e retenção controlada | APROVADO | `01-arquitetura/transcricao-multimodal.md` |
| RF-026 | SOS multimodal independente de ASR, com autenticação, localização opcional, escalonamento autorizado, ACK de rede/dispositivo e humano separados, modo teste e auditoria | APROVADO | `02-dominio/servico-sos.md` |
## Não funcionais
| ID | Requisito | Teste / critério |
|---|---|---|
| RNF-001 | IPv6-first sem prejudicar acessibilidade | Testes de IPv6, IPv4, QUIC, TCP e relay |
| RNF-002 | Identidade estável e rotação segura de credenciais | Reinício, mudança de IP, revogação, compromisso de chave |
| RNF-003 | Isolamento multi-tenant e localização consentida | Testes de autorização, revogação e auditoria |
| RNF-004 | Sincronização idempotente e recuperável | Partições, duplicação, reorder, snapshots e tombstones |
| RNF-005 | Alta disponibilidade com degradação explícita | Falha de instância, zona, região e partições |
| RNF-006 | Não saturar serviços críticos com tráfego opcional | Admission control, backpressure, reservas e prioridades |
| RNF-007 | RTP/mídia não transportada pelo event bus | Teste de jitter, perda, reconexão e floor control |
| RNF-008 | Evolução de protocolos/firmwares interoperável | Negotiation, compatibilidade N/N-1, rollout canary |
| RNF-009 | Rastreabilidade dos estados de SOS e comandos | ACK persistido, recebido, confirmado e reconhecido |
| RNF-010 | Operação e compliance | Backups restaurados, observabilidade e políticas RF |

| RNF-011 | Transcrição não afeta QoS da voz; política de privacidade e ASR/retention verificáveis | Testes de falha ASR, consentimento, E2EE e isolamento entre tenants |
| RNF-012 | SOS mostra evidência do avanço por camadas e não promete despacho ou entrega | Simular falha de gateway, no-ACK, falha regional e confirmação humana |
## Qualidade documental
Todo requisito deverá possuir dono técnico, contratos concretos, casos de teste, limites conhecidos e critérios quantitativos de aceite. [Plano de verificações](../05-operacao/testes.md).

## Requisitos do GodMesh Guardian, CrowdFind e contingência (aprovação de arquitetura)
| ID | Requisito | Estado | Referência |
|---|---|---|---|
| RF-027 | Contexto operacional terra/ar/mar por atividade, sem inferência compulsória ou habilitação de RF por tipo presumido | APROVADO | `02-dominio/contexto-operacional.md` |
| RF-028 | Guardian: convite, prazo, rastreamento autorizado temporário, revogação, check-in e aviso de atraso sem SOS automático | APROVADO | `02-dominio/guardian-acompanhamento.md` |
| RF-029 | Sensor fusion GNSS/IMU/Wi-Fi/BLE/UWB conforme hardware e APIs, com incerteza/frescor; sem falsas direções Wi-Fi 360° | APROVADO | `01-arquitetura/guardian-localizacao-trajetoria.md` |
| RF-030 | CrowdFind próprio com Beacon opcional e ID rotativo, sem depender de Find My/Find Hub e sem rastreamento não autorizado | APROVADO | `03-integracoes/crowdfind-beacon.md` |
| RF-031 | DICE monitora conectividade celular/IPv6/IPv4/local e classifica satélite somente com evidência permitida | APROVADO | `01-arquitetura/conectividade-contingencia-satelite.md` |
| RF-032 | Sem automação disponível, orientar ação manual, sem mudar configurações de operadora, satélite ou aplicativos terceiros | APROVADO | `07-contratos/conectividade-contingencia.md` |
| RF-033 | Em conexão restrita, priorizar SOS/check-in/mensagens e reduzir cargas não essenciais do próprio app, mantendo comunicação de emergência do SO | APROVADO | `01-arquitetura/conectividade-contingencia-satelite.md` |

## Requisitos — aplicativo, Wi-Fi comunitário, mapa e presença dinâmica

| ID | Requisito | Estado | Referência |
|---|---|---|---|
| RF-034 | Aplicativo normal com Início, Comunicações, Mapa, Redes, Dispositivos, Guardian e SOS; modos simultâneos e offline | APROVADO | `01-arquitetura/aplicativo-operacao-normal.md` |
| RF-035 | Wi-Fi AutoDiscover inventaria privadamente redes observáveis pelo SO, sem extração de senhas salvas ou cadastro de terceiros como públicos | APROVADO | `01-arquitetura/wifi-autodiscover-autoshare.md` |
| RF-036 | AutoShare apenas AP comprovadamente administrado, com consentimento, políticas, validade e revogação | APROVADO | `04-seguranca/wifi-compartilhamento-privacidade.md` |
| RF-037 | AutoConnect solicita associação pelo SO conforme plataforma, distinguindo sugestão, associação, autenticação e internet verificada | APROVADO | `03-integracoes/wifi-android-ios-roteadores.md` |
| RF-038 | AP físico registra coordenadas exatas disponíveis, qualidade, origem, pessoa que cadastrou/compartilhou, data e histórico de mudanças | APROVADO | `02-dominio/wifi-modelo-dados.md` |
| RF-039 | Mapa publica AP exato somente mediante opt-in específico do administrador comprovado; usuário/visitante não é exposto | APROVADO | `04-seguranca/wifi-compartilhamento-privacidade.md` |
| RF-040 | Auditoria de acesso Wi-Fi por usuário/GMC, AP, grant, sessão, duração e volume quando medido pelo controlador; sem histórico de navegação | APROVADO | `02-dominio/wifi-registro-acessos.md` |
| RF-041 | APIs/GMP de inventário, APs, compartilhamento, grants, provisionamento, sessões e mapa com escopo e idempotência | ESPECIFICAR | `07-contratos/wifi-autodiscover-autoshare-api.md` |
| RF-042 | Live Presence no iOS/Android com SOS/Guardian/PTT e estados reais, sem exposição de dados precisos na tela bloqueada | APROVADO | `01-arquitetura/live-presence.md` |
| RF-043 | Operação prioritária e energia adaptativa preservam comunicação essencial sem controlar apps de terceiros | APROVADO | `01-arquitetura/aplicativo-operacao-normal.md` |

## Critérios adicionais não funcionais

- RNF-013: não publicar scans Wi-Fi nem senhas por padrão; comprovação de titularidade auditável.
- RNF-014: distinção verificável entre `requested`, `os_associated`, `ap_authenticated` e `internet_verified`.
- RNF-015: informações administrativas de AP e visitantes protegidas por função/tenant e retenção definida.
- RNF-016: Live Activities/Updates não devem anunciar como confirmados estados sem evidência.
- RNF-017: fluxo offline, revogação e disponibilidade de AP testados sob partição regional.
