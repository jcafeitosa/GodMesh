# GodMesh DICE — Device Intelligence & Connectivity Engine

> **Arquitetura proposta para Android e iOS.** Nenhum aplicativo pode assumir acesso irrestrito a todos os rádios ou sensores. Toda disponibilidade é detectada e autorizada em tempo de execução.

## Responsabilidade
O DICE faz inventário dinâmico dos recursos permitidos do aparelho, monitora qualidade dos caminhos, identifica dispositivos GodMesh próximos e escolhe **rotas elegíveis** para mensagens, localização, PTT e SOS. O app nativo usa API Android/iOS para hardware e partilha núcleo Rust para decisões e GMP.

## Submódulos
- `dice-capabilities`: inventário assinado de funções realmente expostas ao aplicativo, incluindo versão de OS e estado de permissão.
- `dice-discovery`: descoberta de peers GodMesh por BLE, Bonjour/mDNS, Wi-Fi Aware e outras APIs presentes.
- `dice-connectivity`: rotas IPv6/IPv4, Wi-Fi, celular, LAN, links peer-to-peer e gateways autorizados; métricas de sinal são meramente indicativas.
- `dice-sensors`: GNSS, movimento, orientação, barômetro e proximidade quando existentes.
- `dice-policy`: escopo de acesso, permissões, privacidade, bateria e regras de execução em segundo plano.
- `dice-route-selector`: escolhe caminho sob restrições de capacidade, latência, consumo e autorização; não inventa acesso a RF satelital ou outras redes.
- `dice-offline`: fila local cifrada, replay idempotente e expiração por tipo de mensagem.

## Matriz preliminar de recursos
| Recurso | Android | iOS | Papel | Observação |
|---|---|---|---|---|
| IPv6/IPv4 Wi-Fi e rede móvel | Sim | Sim | Rede IP GodMesh | Alternância e conectividade dependem do SO |
| Wi-Fi AP scanning genérico | API com permissões e throttling | Sem API pública geral para lista irrestrita | Referências de rede | Não fornece direção dos APs |
| Bonjour/mDNS / descoberta LAN | Sim | Sim, com privacidade LAN | Peers locais | iOS pede permissão de rede local |
| BLE central/scan | Sim, com permissões | Sim, com limitações de background | Detectar tags e gateways | Taxa e payload limitados |
| BLE peripheral/advertising | Sim, segundo hardware e SO | Sim, anúncios restritos em background | Guardian Beacon | Não equivalente a uma AirTag |
| Bluetooth Classic | Perfis e APIs permitidos | Perfis/interfaces autorizados | Rádios e periféricos | Não é acesso genérico universal |
| Wi-Fi Direct | Dispositivos compatíveis | Sem API pública geral equivalente | Conexão peer-to-peer | Não assumir interoperabilidade direta |
| Wi-Fi Aware | Em aparelhos compatíveis | Conforme disponibilização por dispositivo/versão | Descoberta e conexão peer-to-peer | Negociar capabilities |
| Wi-Fi RTT | Em aparelhos/APs compatíveis | Não assumir API equivalente geral | Ranging | Requer infraestrutura compatível |
| UWB | Hardware/entitlements quando presentes | Hardware/entitlements quando presentes | Proximidade precisa | Não é rastreamento mundial |
| NFC | Conforme hardware/permissão | Conforme entitlements/tipo de tag | Associação/cadastro | Geralmente interação próxima |
| GNSS / localização do SO | Com autorização | Com autorização | Guardian, SOS | Precisão variável |
| Acelerômetro/giroscópio | Quando disponíveis | Quando disponíveis | Estimar movimento | Deriva sem referência externa |
| Magnetômetro/barômetro/passos | Dependendo do aparelho | Dependendo do aparelho | Orientação/elevação relativa | Não equipar todos como padrão |
| USB/áudio externo | Classes e permissões compatíveis | Interfaces de acessórios permitidas | Rádio/gateway/PTT | Controle do rádio requer suporte do equipamento |
| Satélite | Somente por APIs/serviços disponibilizados | Somente por APIs/serviços disponibilizados | Transporte opcional | Não há acesso satelital bruto universal |
| LoRa/LoRaWAN | Via acessório externo | Via acessório externo | Telemetria/SOS curtos | Smartphone padrão não tem rádio LoRa integrado |

## Fluxo de avaliação
1. Detectar existência da API e capacidade de hardware.
2. Solicitar **apenas** permissões necessárias para uma função explicitamente ativada.
3. Verificar disponibilidade operacional (Wi-Fi habilitado, BLE permitido, energia, restrições de background).
4. Descobrir apenas serviços e beacons GodMesh/terceiros autorizados; não fazer varredura agressiva/indiscriminada de aparelhos alheios.
5. Validar identidade, propriedade ou autorização do peer/gateway.
6. Oferecer rotas elegíveis com estado `ready | unavailable | permission_required | hardware_unsupported | background_limited`.
7. Registrar transições sem armazenar identificadores de rede desnecessários.

## Conectividade durante Guardian
Guardian pode solicitar sensores e publicidade BLE em contexto explícito; sua ativação não transforma o aplicativo em agente de descoberta sempre ligado. Políticas de SO, modo de economia de energia e consentimento condicionam a coleta. A interface mostra **última detecção**, latência e idade da posição.

## Segurança
A descoberta não é permissão de conexão. Não usar SSID/BSSID/MAC de redes alheias como identidade estável de pessoas. Não armazenar lista de pontos de acesso não necessária à atividade. Nem mesmo o SOS contorna permissões do SO ou habilita rádios bloqueados.

## Fontes técnicas
- Android Wi-Fi Scan: https://developer.android.com/develop/connectivity/wifi/wifi-scan
- Android Wi-Fi Aware: https://developer.android.com/develop/connectivity/wifi/wifi-aware
- Android sensores: https://developer.android.com/develop/sensors-and-location/sensors/sensors_overview
- Apple Core Bluetooth (background): https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Apple Bonjour privacidade LAN: https://developer.apple.com/documentation/Technotes/tn3179-understanding-local-network-privacy

## Contratos e testes
- [Contratos Guardian/CrowdFind](../07-contratos/guardian-crowdfind.md)
- [Testes de dispositivos móveis](../05-operacao/testes-mobile-guardian.md)

## Contingência e orientação manual
O DICE incorpora o serviço [Resilient Connectivity](conectividade-contingencia-satelite.md). Quando possível, o app monitora alterações da rede móvel e caminhos IP, escolhe apenas transportes já disponibilizados pelo sistema e reduz tráfego discricionário **do próprio GodMesh**. Quando troca de operadora, plano/serviço satelital, bateria ou aplicativos terceiros não podem ser controlados pelas APIs oficiais, o app **orienta o usuário a realizar a ação manualmente** e registra o resultado como desconhecido até confirmação. O Guardian/SOS nunca deve afirmar que algo foi entregue apenas por detectar uma rota satelital.
