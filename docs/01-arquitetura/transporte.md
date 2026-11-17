# Redes e transportes — IPv6 preferencial, IPv4 compatível

## Contrato

O GodMesh possui protocolo de **aplicação** próprio, mas **não** inventa protocolos de transporte. A seleção das tecnologias deve privilegiar padrões modernos, maduros, interoperáveis e atualizáveis; recursos experimentais entram por provas de conformidade e implementação. Ver [Estratégia tecnológica](estrategia-tecnologica.md).

**IPv6-first** para enlaces IP entre nós e para clientes capazes, com fallback transparente a IPv4 quando necessário. Identidade criptográfica do nó é independente de endereço. Transporte QUIC/TLS 1.3 preferencial; TCP seguro onde UDP não funciona. Clientes web usam HTTP/2/3, WebSocket e WebRTC, sujeitos a APIs do navegador.

## Seleção de caminho
1. Autenticar peer e obter endpoints de DNS AAAA/A, registro assinado, Kademlia autorizado e cache local.
2. Filtrar endereços expirados, protocolos incompatíveis e destinos sem política.
3. Preferir IPv6 medido como alcançável; tentar IPv4 de maneira escalonada (*Happy Eyeballs*), sem esperar um timeout excessivo.
4. Tentar relay autenticado se conexão direta falhar; impor quotas e evitar loops.
5. Medir RTT, perda, taxa de falha, custo, estabilidade e estado do enlace; aplicar histerese.
6. Negociar esquema GMP, capacidades e versões mínimas seguras; nunca permitir downgrade de autenticação.

## Protocolos e camadas

A versão de transporte e a versão **GMP** são negociadas independentemente. QUIC v1 e QUIC v2 são padrões especificados; o suporte a QUIC v2 **não está implícito** e exige implementação, testes e negociação conforme as RFCs pertinentes. Protocolos futuros serão adicionados por novos providers com rollout controlado, mantendo compatibilidade.

| Função | Padrões | Observação |
|---|---|---|
| P2P servidor-servidor | libp2p sobre QUIC/IPv6, IPv4; TCP seguro | Descoberta/controle/replicação; não cada pacote de voz |
| APIs | HTTPS HTTP/2 ou HTTP/3 | Navegadores e SDKs |
| Eventos em tempo real | WebSocket / streaming versionado | Replay depende de log persistente |
| Voz em navegador | WebRTC + Opus | Mídia e sinalização separados |
| IoT | MQTT e adaptadores de rádio | LoRaWAN não equivale a IPv6 na RF |
| Protocolos futuros | Plug-ins testados + negociação | Suporte só após implementação de adapter |

## Descoberta
mDNS apenas na LAN; seeds/peers já conhecidos e DNS distribuídos; DHT de participantes autorizados; libp2p Identify para inventário de endereços e capacidades verificadas. Gossipsub para anúncios efêmeros, não armazenamento histórico.

## Segurança
Abrir e limitar explicitamente UDP/TCP IPv6 e IPv4; aplicar TLS, ACL e logs de handshake. IP público não equivale a permissão. Não presumir que um cliente web tem acesso a sockets P2P TCP/QUIC irrestritos.

## Referências
- [RFC 8305 — Happy Eyeballs v2](https://www.rfc-editor.org/rfc/rfc8305.html)
- [RFC 9000 — QUIC](https://www.rfc-editor.org/rfc/rfc9000.html)
- [RFC 9369 — QUIC Version 2](https://www.rfc-editor.org/rfc/rfc9369.html)
- [RFC 9368 — Compatible Version Negotiation](https://www.rfc-editor.org/rfc/rfc9368.html)
- [rust-libp2p](https://github.com/libp2p/rust-libp2p)
