# LoRa, LoRaWAN e BLE

## Diferenças incontornáveis
| Tecnologia | Topologia/uso | Suporte GodMesh |
|---|---|---|
| LoRa ponto a ponto/mesh | Pode criar malha por firmware especializado | Mensagens e eventos via gateway compatível |
| LoRaWAN | Estrela-de-estrelas, servidor de rede e gateways | ChirpStack; telemetria, mensagens curtas e SOS |
| Bluetooth LE | Proximidade e links locais | Tags autorizadas via smartphone/gateway |

**LoRaWAN não equivale a mesh de rádio** e não oferece voz PTT convencional de baixa latência. A mesh Global Mesh é **a rede de informação entre IP nodes**, não a rede LoRaWAN física.

## ChirpStack
Integração preferencial por MQTT:
- Uplink evento: `application/<APPLICATION_ID>/device/<DEV_EUI>/event/up`
- Fila de downlink: `application/<APPLICATION_ID>/device/<DEV_EUI>/command/down`
- Identificador em MQTT deve ser validado, associado a tenant/dispositivo e normalizado.
- Confirmar que fila aceita o comando não prova entrega pelo dispositivo.

Documentação: [ChirpStack MQTT](https://www.chirpstack.io/docs/chirpstack/integrations/mqtt.html).

## Classes LoRaWAN
- **Classe A:** recepção após uplink; menos consumo, resposta com latência e disponibilidade restritas.
- **Classe C:** escuta quase contínua onde suportado; mais consumo e ainda sujeito a disponibilidade de rede e regras do rádio.

## BLE
Conector de gateway ou smartphone autorizado detecta tags, coleta identificadores permitidos e vincula a observação ao gateway/posição estimada. Não assumir acesso às redes proprietárias Apple/Google/Samsung por APIs públicas. É obrigatório evitar rastreamento de terceiros sem autorização.

## Firmware de terminal
Será um projeto separado: pairing seguro, assinatura/criptografia apropriada ao protocolo, mensagens compactas e versão de payload; evitar afirmar suporte a GPS/SOS sem hardware/firmware. Verificar legalidade local de frequências, potência, duty cycle e uso do espectro antes de instalação.

## Integração em modos offline
Se perder a internet externa, um gateway LoRaWAN pode continuar atendendo se houver ChirpStack/MQTT e conectividade IP local disponíveis. Um dispositivo Classe A não recebe “push imediato” fora de sua janela de recepção. Mesh LoRa pode continuar no ar dentro do alcance e topologia real.
