# Visão estratégica e escopo integral do GodMesh

> **Normativo (produto):** esta página descreve o produto completo planejado, não um escopo reduzido nem funcionalidades já implementadas.

## Missão
**GodMesh é uma infraestrutura global de conectividade e comunicação para pessoas, equipes, ativos e dispositivos, com semântica de aplicação, motor de voz e federação próprios em Rust, apoiados em padrões modernos e abertos. Sistemas externos são acessados exclusivamente por bridges opcionais; a operação nativa não depende deles.**

A experiência lembra um *Find My* universal, ampliado com voz PTT nativa, mensagens, alertas, redes de rádio, automações e gateways IoT. **Não** implica acesso às redes proprietárias Find My/Find Hub/SmartThings Find sem uma integração oficialmente autorizada.

## Produtos sobre o mesmo núcleo

> **Definição de tecnologia própria:** não precisamos inventar um substituto para QUIC, TLS, WebRTC ou Opus; precisamos ser os autores do protocolo GMP, da experiência PTT e das regras de interoperabilidade, independentes de servidores terceiros. Ver [Estratégia tecnológica](../01-arquitetura/estrategia-tecnologica.md).
- **GodMesh Personal:** dispositivos próprios, localização consentida, compartilhamento com pessoas autorizadas, mensagens, alertas e canais de voz.
- **GodMesh Teams:** organizações, equipes, rádios/gateways, ativos e veículos, ACL, geofencing, despacho, auditoria, retenção e integração corporativa.
- **GodMesh Node:** distribuição Linux em Rust, nó regional/edge/gateway, capacidades de comunicação, federação segura e sincronização.
- **GodMesh Platform & SDK:** API pública, SDKs de dispositivos, bridges, perfis e automações.

## Plataformas aprovadas

Servidor em contêiner Linux; desktop macOS universal (Apple Silicon e Intel) com SwiftUI, Windows e Linux; mobile iOS com SwiftUI e Android. Escolhas específicas de interface em Windows, Linux e Android seguem a [ADR-055](../06-planejamento/decisoes.md#adr-055--plataformas-de-entrega-e-seleção-de-interfaces), sem afirmar implementação.

## Capacidades integrais do produto
1. Identidade federada de usuário/organização/nó/dispositivo, com permissões contextualizadas.
2. Ativos independentes de dispositivos e dispositivos independentes de transportes; múltiplas interfaces por dispositivo.
3. Motor de capacidades verificadas e roteador universal de serviços.
4. Voz PTT **GodMesh Native** (canal, prioridade, arbitragem, sessões e distribuição), sem dependência de Mumble/FRN.
5. Mensagens, notificações, presença, grupos, comandos e SOS com confirmação por etapa.
6. Posição GNSS, BLE/proximidade, histórico, geofencing, mapa e identificação da qualidade/fonte.
7. Gateways e bridges opcionais: Mumble, FRN, ChirpStack/LoRaWAN, LoRa mesh, GPS/LTE, MQTT, BLE e protocolos adicionados futuramente.
8. Vários servidores completos por região; distribuição de carga e federação entre regiões, com IPv6 prioritário e IPv4 alternativo.
9. Descoberta, instalação e autoativação após autorização, atualização assinada e recuperação offline.
10. Observabilidade, backup, recuperação de desastre, isolamentos por tenant e políticas de residência dos dados.

## Experiências desejadas
- Usuário registra rádio, rastreador, tag, smartphone ou veículo; o app habilita só o que é compatível e permitido.
- Clientes GodMesh falam por PTT nativo. Uma bridge autorizada pode unir canais específicos Mumble/FRN, sem misturar automaticamente redes privadas, profissionais e de radioamador.
- Um dispositivo LoRaWAN envia localização/SOS como dados curtos, não áudio PTT de baixa latência.
- O cliente é direcionado a um servidor regional elegível; em falha, reconecta a outro, com limites de continuidade explícitos.
- Uma região isolada continua executando operações locais autorizadas e sincroniza após restabelecer conectividade.

## Limites técnicos e de segurança
- Nenhum nó recebe por padrão todas as localizações de todos os usuários; a replicação depende de finalidade, autorização e legislação aplicável.
- Sem licença/autorização de rádio não se permite transmissão RF; redes de radioamador não podem transportar indiscriminadamente tráfego corporativo.
- Emergências requerem confirmação, energia, cobertura e canais redundantes; não se promete entrega garantida em toda circunstância.
- Navegadores e smartphones não se tornam roteadores P2P permanentes em segundo plano.
- Sem criação de cifra ou codec próprios: adotar padrões modernos e auditados.

## Posicionamento
**Uma rede própria com conectividade interoperável**, não uma interface que simplesmente embala Mumble/FRN ou um sistema que depende de um servidor principal permanente. Consulte [Arquitetura de referência](../01-arquitetura/arquitetura-referencia.md) e [Decisões](../06-planejamento/decisoes.md).
