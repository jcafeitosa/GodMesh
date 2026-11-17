# Catálogo de serviços e workspace Rust

## Estratégia de implantação
Arquitetura lógica completa, com **monólito modular em Rust** para regras de domínio e **processos especializados** separados para mídia em tempo real, gateways de hardware e bridges não confiáveis. Isso é uma decisão de fronteira operacional, não uma redução funcional do produto final. Outros serviços podem ser extraídos conforme isolamento/escala.

| Crate lógico | Papel |
|---|---|
| `godmesh-protocol` | Contratos GMP, negociação, erros e IDs |
| `godmesh-identity` | Usuários, tenants, dispositivos e confiança de nós |
| `godmesh-federation` | Descoberta, libp2p, ingresso e rotas P2P |
| `godmesh-governance` | Políticas, catálogos, quórum e revogação |
| `godmesh-sync` | Outbox/inbox, snapshot, replay, tombstones |
| `godmesh-balancer` | Região, nó, admissão, afinidade e degradação |
| `godmesh-registry` | Ativos, dispositivos e endpoints |
| `godmesh-capabilities` | Perfis, verificações, políticas e rotas possíveis |
| `godmesh-location` | GNSS, BLE, mapas, geofencing e histórico |
| `godmesh-messaging` | Grupos, presença, mensagens e notificações |
| `godmesh-emergency` | SOS, ACKs, escalonamento e auditoria |
| `godmesh-voice-control` | Canais nativos, PTT e sessões |
| `godmesh-media-edge` | Transporte e distribuição de áudio Opus |
| `godmesh-bridge-sdk` | Contratos para redes externas opcionais |
| `godmesh-adapters` | ChirpStack, BLE, GPS, MQTT, LoRa, FRN, Mumble |
| `godmesh-observability` | Métricas, traces, auditabilidade |

## Workspace proposto
```text
GodMesh/
├── Cargo.toml
├── crates/
│   ├── godmesh-protocol/
│   ├── godmesh-identity/
│   ├── godmesh-federation/
│   ├── godmesh-governance/
│   ├── godmesh-sync/
│   ├── godmesh-balancer/
│   ├── godmesh-registry/
│   ├── godmesh-capabilities/
│   ├── godmesh-location/
│   ├── godmesh-messaging/
│   ├── godmesh-emergency/
│   ├── godmesh-voice-control/
│   ├── godmesh-media-edge/
│   ├── godmesh-bridge-sdk/
│   └── godmesh-adapters/
│       ├── mumble/
│       ├── frn/
│       ├── chirpstack/
│       ├── ble/
│       ├── lora/
│       └── gps/
├── apps/
│   ├── godmesh-node/
│   ├── godmesh-media/
│   ├── godmesh-bridge-host/
│   ├── godmesh-cli/
│   └── godmesh-api/
├── clients/       # Web, mobile, desktop
├── contracts/     # Protobuf / OpenAPI / SDK
├── migrations/    # Banco e eventos
├── tests/         # Integração, rede, segurança e carga
├── deploy/        # Linux, containers e HA
└── docs/          # Esta documentação
```

## Reuso de tecnologias (candidatas técnicas)
`tokio`, `axum`, `serde`, `sqlx`, `tracing`, `rust-libp2p`, `quinn` ou transporte QUIC gerenciado, `async-nats`, `rumqttc`, PostgreSQL/PostGIS, WebRTC e codec Opus. Confirmar versões, manutenção, licenças e comportamento antes de fixar dependências.

## Fronteiras rígidas
- Mídia isolada de filas de negócio e gossip.
- Conectores externos isolados em processos sandbox, com credenciais por integração.
- Serviço de autenticação e política não depende de FRN/Mumble.
- Controle de versões e migrações compatível com nós temporariamente offline.
- Política de serviço instalado ≠ serviço pronto; readiness é por módulo e dependência.

## Subsistemas acrescentados para o aplicativo e Wi-Fi Share

| Crate ou módulo lógico | Responsabilidade |
|---|---|
| `godmesh-mobile-state` | Atividades simultâneas, inicialização local/offline e prioridades |
| `godmesh-live-presence` | Projeções redigidas para Dynamic Island/Live Activities/Live Updates |
| `godmesh-wifi-core` | Tipos e invariantes de descoberta, compartilhamento e conexão |
| `godmesh-wifi-registry` | Inventário de APs administrados e registro de localização |
| `godmesh-wifi-access` | Grants, sessões, auditoria e retenção |
| `godmesh-wifi-provisioner` | Integração com controladores e credenciais temporárias |
| `godmesh-wifi-map` | Projeções geográficas com consentimento para precisão exata |
| `godmesh-wifi-sync` | Federação de publicações autorizadas, revogações e versões |
| `godmesh-wifi-router-agent` | Controle opcional de AP/guest SSID autorizado pelo administrador |

O core é Rust; chamadas a hardware e ao SO são wrappers Swift/Kotlin. Serviços podem executar no mesmo processo lógico, exceto agentes/integrações que requerem isolamento ou proximidade física ao AP.

## Plataformas de entrega — ADR-055

Decisão explícita do usuário: servidor Rust distribuído em **contêiner Linux**; desktop **macOS universal Apple Silicon (arm64) e Intel (x86_64), com SwiftUI**, **Windows** e **Linux**; mobile **iOS com SwiftUI** e **Android**. O núcleo Rust e as integrações de plataforma permanecem separados.

A distribuição em contêiner não altera a diretriz de desenvolvimento: dependências como PostgreSQL e NATS no OrbStack, backend, frontend e apps executados localmente. Isso não comprova infraestrutura configurada. A imagem Linux base, arquiteturas do servidor, versões mínimas de SO, ferramentas de build e distribuição ainda precisam ser definidas.

Recomendação técnica pesquisada, ainda não aprovada como escolha específica nem validada no GodMesh: **WinUI 3** para Windows, **Qt 6 / Qt Quick / QML** para Linux e **Kotlin / Jetpack Compose** para Android. Ver fundamentos, alternativas e critérios de fechamento na [ADR-055](../06-planejamento/decisoes.md#adr-055--plataformas-de-entrega-e-seleção-de-interfaces).

