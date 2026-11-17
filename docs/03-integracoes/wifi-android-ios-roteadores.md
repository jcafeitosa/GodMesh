# Integração Wi-Fi Android, iOS e controladores de pontos de acesso

> **WIFI-INT-001 · Versão 1.0 · 2026-11-15 · Integração proposta, requer validação por fabricante e versão.**

## Android — adaptador Kotlin

- `WifiManager` e callbacks para resultados de scans e rede atual, respeitando permissões, throttling, serviços de localização e regras de background.
- `WifiNetworkSuggestion` para sugerir redes autorizadas com credenciais provisionadas pelo GodMesh; a decisão de conexão **é do Android**, não do aplicativo.
- Fluxos de conclusão/aprovação de rede via APIs oficiais quando aplicáveis; reportar o estado exato e não antecipar conexão.
- `ConnectivityManager`/`NetworkCapabilities` para diferenciar associação local, internet verificada, Wi-Fi e conexão celular; capturar somente dados necessários.
- Não ler todas as redes salvas e senhas do sistema; não solicitar privilégios de sistema nem contornar sandbox.

## iOS — adaptador Swift

- `NEHotspotConfigurationManager`: configurar SSID ou Passpoint conhecido, **sujeito à aprovação do usuário**; remover configurações próprias quando permitido.
- `NEHotspotNetwork.fetchCurrent()` somente nas condições de entitlement/privacidade adequadas.
- Sem API pública de varredura de todos os APs próximos nem de lista universal de redes/senhas previamente salvas.
- `NWPathMonitor` pode observar conectividade; não identifica administradores de AP nem oferece controle geral de escolha da rede.
- Qualquer conexão “com um toque” deve oferecer `os_approval_required`, `not_in_range` ou `unknown` quando o SO não confirmar o estado.

## GodMesh Router Agent

Um agente Linux/Rust opcional instalado **pelo administrador legítimo do AP** expõe funções homologadas: `list_managed_ssids`, `read_ap_health`, `issue_guest_credentials`, `revoke_grant`, `report_usage`, `report_station_count` e `attest_ownership`.

- Autenticação mútua com chave do agente, pareamento aprovado pelo administrador e permissão por rede.
- Separar rede de convidados da LAN privada; controles de acesso, quota, expiração e isolamento de clientes no roteador.
- Preferir EAP-TLS/Passpoint/PPSK e credenciais por usuário ou dispositivo, quando o equipamento suportar; WPA2/WPA3-PSK compartilhada tem revogação limitada.
- Segurança e capacidade devem ser **capabilities** anunciadas pelo agente; não assumir RADIUS ou PPSK em roteadores domésticos simples.
- Sem acesso a roteador: inventário e mapa poderão funcionar conforme autorização, mas provisionamento automático auditável pode ser indisponível.

## Matriz resumida

| Recurso | Android | iOS | Roteador pareado |
|---|---|---|---|
| Scan de APs visíveis | Parcial, restrito | Não geral | APs sob sua administração |
| Rede atual | Conforme permissão | Conforme entitlement/condição | Clientes associados, quando suportado |
| Listar senhas salvas | Não | Não | Não exportar senha privada como serviço padrão |
| Conexão automática | Sugestão sujeita ao Android | Configuração com aprovação | Gestão da autenticação no AP |
| Confirmar usuário autenticado no AP | Apenas evidência de cliente | Apenas evidência de cliente | Sim, se telemetria do controlador disponível |
| Medir uso por sessão | Parcial e não necessariamente por AP | Parcial | Quando o roteador mede e atribui a sessão |

## Referências oficiais

- [Android — Wi-Fi scanning](https://developer.android.com/develop/connectivity/wifi/wifi-scan)
- [Android — Wi-Fi network suggestions](https://developer.android.com/develop/connectivity/wifi/wifi-suggest)
- [Apple — NEHotspotConfigurationManager](https://developer.apple.com/documentation/networkextension/nehotspotconfigurationmanager)

[Arquitetura Wi-Fi](../01-arquitetura/wifi-autodiscover-autoshare.md) · [Privacidade](../04-seguranca/wifi-compartilhamento-privacidade.md).
