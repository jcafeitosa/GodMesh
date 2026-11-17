# Fontes de referência e bibliotecas candidatas

> Links públicos para consulta técnica, sujeitos a mudanças de versão/licença. Conferir documentação atual antes de usar em produção.

## Identidade global
- RFC 9562 (UUIDv7): https://www.rfc-editor.org/rfc/rfc9562.html
- RFC 7565 (`acct:`): https://www.rfc-editor.org/rfc/rfc7565.html
- RFC 7033 (WebFinger): https://www.rfc-editor.org/rfc/rfc7033.html
- OpenID Connect (`iss`+`sub`): https://openid.net/specs/openid-connect-core-1_0.html
- W3C WebAuthn Level 3: https://www.w3.org/TR/webauthn-3/
- W3C DID Core: https://www.w3.org/TR/did-core/

## Rede e protocolos
- rust-libp2p: https://github.com/libp2p/rust-libp2p
- Gossipsub: https://libp2p.github.io/rust-libp2p/libp2p_gossipsub/index.html
- RFC 8305 (Happy Eyeballs v2): https://www.rfc-editor.org/rfc/rfc8305.html
- RFC 9000 (QUIC): https://www.rfc-editor.org/rfc/rfc9000.html

## Banco e eventos
- PostgreSQL: https://www.postgresql.org/docs/
- PostGIS: https://postgis.net/documentation/
- NATS JetStream: https://docs.nats.io/nats-concepts/jetstream
- Replicação NATS Source/Mirror: https://docs.nats.io/nats-concepts/jetstream/source_and_mirror

## Voz e rádio
- Mumble: https://github.com/mumble-voip/mumble
- Mumble ACL: https://www.mumble.info/documentation/administration/acl/
- Mumble Ice (segurança): https://www.mumble.info/documentation/mumble-server/scripting/ice/server-setup/
- Free Radio Network: https://freeradionetwork.eu/
- Protocolo FRN: https://freeradionetwork.eu/frnprotocol.htm
- FRNServer/AlterFRN Linux: https://github.com/PP5PK/FRNServer
- AllStarLink (referência posterior): https://www.allstarlink.org/
- M17: https://m17project.org/
- APRS-IS: https://www.aprs-is.net/Connecting.aspx

## IoT / hardware
- ChirpStack: https://www.chirpstack.io/
- Integração MQTT ChirpStack: https://www.chirpstack.io/docs/chirpstack/integrations/mqtt.html
- Traccar: https://www.traccar.org/
- Meshtastic: https://meshtastic.org/
- OpenMQTTGateway: https://docs.openmqttgateway.com/

## Infraestrutura e engenharia
- Envoy Load Balancing: https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/upstream/load_balancing/overview
- OpenRaft: https://github.com/databendlabs/openraft
- Tokio: https://tokio.rs/
- Axum: https://github.com/tokio-rs/axum

## Caveats explícitos
- FRN é um protocolo com documentação histórica: validar servidores e binários atuais.
- Mumble Ice precisa ficar protegido e restrito à administração interna.
- A integração ChirpStack MQTT confirma eventos/filas, não necessariamente entrega de downlink a dispositivo.
- NATS source/mirror é assíncrono e não espelha exclusões automaticamente.
- Integrações com redes públicas de radioamador dependem de autorização e normas do país.

## APIs móveis e conectividade satelital
- Android `ConnectivityManager`/`NetworkCapabilities`, incluindo `TRANSPORT_SATELLITE`: https://developer.android.com/reference/android/net/NetworkCapabilities
- Android `TelephonyCallback`: https://developer.android.com/reference/android/telephony/TelephonyCallback
- Android `SatelliteManager`: https://developer.android.com/reference/android/telephony/satellite/SatelliteManager
- Android tela oficial de economia de bateria: https://developer.android.com/reference/android/provider/Settings
- Apple `CTTelephonyNetworkInfo`: https://developer.apple.com/documentation/coretelephony/cttelephonynetworkinfo
- Apple recursos de satélite de operadoras: https://support.apple.com/en-us/122339
- Apple restrição de rede cara/limitada: https://developer.apple.com/documentation/foundation/urlsessionconfiguration/allowsexpensivenetworkaccess
- Apple modo de economia de bateria: https://support.apple.com/guide/iphone/iphcab9aecd1/ios
- Apple BLE em segundo plano: https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Exemplo de serviço de operadora Direct-to-Cell: https://www.t-mobile.com/coverage/satellite-phone-service
