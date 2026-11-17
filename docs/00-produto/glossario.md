# Glossário

> **Proposta documental C05:** as definições de Universal Gateway e estados de capacidade abaixo esclarecem o vocabulário usado nos contratos existentes; não aprovam implantação, alteração de ADR ou formato de wire. Ver [Fronteiras de módulos](../01-arquitetura/fronteiras-modulos.md).

- **GodMesh Native:** tecnologia própria de aplicação e voz da plataforma; independente de Mumble ou FRN.
- **GMP:** GodMesh Protocol, conjunto versionado de contratos de aplicação e sinalização.
- **GodMesh Voice:** motor nativo de sessões, canais, PTT, encaminhamento e mídia.
- **GodMesh Bridge SDK:** interface versionada para implementar conectores externos opcionais e isolados; não é, por si só, um runtime, uma autoridade de identidade ou uma decisão de autorização/rota.
- **Universal Gateway (proposta de vocabulário):** subsistema arquitetural que coordena descoberta/perfis, avaliação de capacidades, políticas e seleção de rotas usando módulos especializados. É um papel/fluxo composto, não uma crate ou processo único obrigatório; a composição pode permanecer no monólito modular salvo necessidade já coberta por decisão arquitetural. Não substitui a UDNL nem o plano de mídia. Ver [Fronteiras de módulos](../01-arquitetura/fronteiras-modulos.md).
- **UDNL:** camada semântica que valida e normaliza dados externos, preserva escopo e proveniência e prepara projeções explícitas; não concede autorização nem escolhe, sozinha, a rota. Contrato de dados e perda declarado em [Normalização de dados](../01-arquitetura/normalizacao-dados.md).
- **Ativo:** bem ou objeto lógico (veículo, equipamento etc.), distinto do dispositivo.
- **Dispositivo:** hardware com uma ou mais interfaces identificáveis.
- **Endpoint:** forma de alcançar um dispositivo em um protocolo específico.
- **Capacidade:** função identificável como `voice.tx`, `location.report` ou `message.rx`, avaliada para sujeito, perfil, direção e contexto. Fontes documentais propõem os rótulos `supported` (declarada), `verified` (com evidência atribuível) e `operational` (disponibilidade observada no contexto); isso não é uma aprovação de contrato único. A seção 7.3 da API GMP v1 declara explicitamente esses conjuntos cumulativos e define o mapeamento como proposta documental; `02-dominio/capacidades.md` agora separa evidência de capacidade, autorização, elegibilidade de rota e readiness, refletindo essa projeção sem aprová-la. Capacidade operacional não é decisão de autorização nem readiness do serviço. Semântica e correspondência propostas em [Fronteiras de módulos](../01-arquitetura/fronteiras-modulos.md), para revisão de integração por Alan com os contratos proprietários.
- **Transporte:** meio de rede (IPv6, IPv4, WebRTC, LoRaWAN, BLE, LTE etc.).
- **Gateway:** agente, hardware ou software que conecta redes diferentes; qualificar como **Universal Gateway** quando se referir ao subsistema GodMesh, para não confundir com um dispositivo gateway externo.
- **Adapter (adaptador):** implementação concreta, específica de protocolo/dispositivo, que atende ao contrato de uma interface na borda. A tradução semântica externa passa pela UDNL.
- **Bridge:** adaptador que integra serviços de dados ou mídia entre domínios de protocolo externos; opcional, autorizado e isolado, e pode terminar criptografia. Não é caminho obrigatório entre participantes GodMesh nativos.
- **Tenant:** unidade de propriedade e autorização, conta pessoal ou organização.
- **PoP:** ponto de presença regional para atendimento de clientes.
- **Célula regional:** conjunto de nós GodMesh ativos com balanceamento e armazenamento regional.
- **Full node:** servidor com pacote completo de serviços; disponibilidade real depende de recursos e dependências.
- **Control plane:** identidade de nós, versões, políticas e decisões fortes com quórum.
- **Data plane:** dispositivos, mensagens, localização, comandos e eventos persistentes.
- **Media plane:** áudio e sessões PTT de baixa latência; separado de gossip/log de eventos.
- **Active-active:** múltiplos nós atendendo em paralelo; não implica multi-writer arbitrário.
- **Snapshot:** cópia íntegra de estado permitido para sincronização inicial.
- **Outbox/Inbox:** registro transacional de emissão/recebimento e deduplicação.
- **Checkpoint:** posição persistida de replay por origem e fluxo.
- **Lease / fencing token:** concessão temporal e geração que limitam operação exclusiva.
- **Gossip:** propagação de metadados recentes, sem garantia de armazenamento histórico.
- **PTT / floor:** push-to-talk e direito temporário de transmitir em um canal.
- **FRN:** Free Radio Network, rede/protocolo externo com regras próprias.
- **Mumble:** sistema externo de voz/canais, integrado somente via bridge.
- **LoRa:** modulação de rádio; uma mesh LoRa exige protocolo/firmware adequado.
- **LoRaWAN:** rede de rádio estrela-de-estrelas, não mesh direta de terminais.
- **Classe A/C:** classes LoRaWAN com diferentes características de recepção e consumo.
- **Entrega confirmada:** estado por etapa, não sinônimo de mensagem enfileirada.
- **Arquitetura-alvo:** sistema integral especificado; não significa código já entregue.
