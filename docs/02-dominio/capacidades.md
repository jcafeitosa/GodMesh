# Perfis de dispositivo e motor de capacidades

## Perguntas separadas da avaliação
O motor responde separadamente: (1) que capability o sujeito declara ou demonstra; (2) se uma operação é autorizada; (3) se existe rota elegível para o destino e contexto; e (4) se o serviço está pronto para atender. Uma resposta positiva em uma dimensão não implica resposta positiva nas demais.

## Níveis
1. **`supported` (declarada):** perfil, fabricante ou usuário informa que o sujeito oferece `voice.tx` ou `location.report`. É uma declaração, não evidência de funcionamento.
2. **`verified` (verificada):** evidência atribuível sustenta a capability para o sujeito e contexto avaliados. Não significa autorização nem disponibilidade atual.
3. **`operational` (disponibilidade contextual observada):** evidência atual conforme uma política de frescor configurada sustenta que a capability e suas dependências estão disponíveis no contexto avaliado. Não inclui decisão de autorização, elegibilidade de rota global nem prontidão geral do serviço.

### Dimensões independentes e composição

| Dimensão | Pergunta | Resultado/consequência |
|---|---|---|
| Evidência da capability | O sujeito declara, demonstra ou tem disponibilidade observada para esta função e contexto? | Nível máximo sustentado: `supported`, `verified` ou `operational`. Mudança de sujeito, perfil, dependência, contexto ou evidência pode reduzir ou invalidar a avaliação. |
| Autorização | O principal pode realizar esta operação, sob a policy e o escopo efetivos? | `AccessDecision` independente, derivado de identidade/provas e policy/ACL. Uma capability não concede permissão. |
| Elegibilidade da rota | Há caminho permitido para origem, destino, serviço e restrições desta operação? | Decisão contextual de roteamento, com rota/evidências/validade quando disponíveis. Capability operacional não garante rota elegível; descoberta de rota não concede autorização. |
| Readiness do serviço | O serviço responsável consegue atender sua função operacional? | `HealthReady` ou estado equivalente do serviço. Readiness não prova a capability de um dispositivo e não concede autorização. |

**Projeção cumulativa proposta, alinhada ao contrato proprietário:** quando a resposta usar as coleções GMP, `operational[] ⊆ verified[] ⊆ supported[]`; o nível maior implica presença nos níveis anteriores. A projeção é proposta documental descrita em `07-contratos/gmp-api-v1.md`, §7.3, e não é declarada aprovada ou implementada aqui. A avaliação não é uma máquina persistente de transições. Sem método de evidência ou janela de frescor configurados, não classificar como `operational` nem anunciar prontidão com base nessa capability; nenhum TTL ou prazo foi escolhido.

Para executar uma operação, o consumidor avalia cada dimensão exigida para aquele serviço e contexto. Se autorização, rota, readiness ou evidência necessária for negativa, desconhecida ou não atual, a operação não é elegível; a causa deve permanecer identificável na dimensão correspondente, sem reclassificá-la como falta de capability.

## Taxonomia inicial
`voice.tx`, `voice.rx`, `ptt.control`, `message.tx`, `message.rx`, `location.report`, `location.proximity`, `telemetry.read`, `sos.trigger`, `command.receive`, `store.forward`.

## Exemplos de dispositivos
| Tipo | Serviços típicos | Condição |
|---|---|---|
| Smartphone IP | voz PTT, mensagem, GPS, SOS no app | Permissões e internet/LAN |
| Rádio analógico | voz PTT | Interface de áudio + controle + gateway |
| Rádio DMR/M17 | voz digital; dados conforme modelo | Hotspot/repetidora e rede autorizada |
| Tag BLE simples | detecção/proximidade | Gateway/app BLE com observação autorizada |
| Rastreador GPS/LTE | posição/telemetria | Protocolo/modelo específico |
| LoRaWAN Classe A | dados curtos, telemetria, SOS se hardware permitir | Downlink em janelas pós-uplink |
| LoRa Mesh | mensagens store-and-forward | Firmware/rede mesh compatíveis |

## Exemplo de perfil (proposta)
```json
{
  "profile_id": "generic-analog-fm-gateway",
  "schema_version": "1.0",
  "interfaces": [{"type": "audio_ptt_gateway", "requires_gateway": true}],
  "capabilities": {"voice.tx": true, "voice.rx": true, "ptt.control": true},
  "conditions": ["authorized_owner", "audio_route_ready", "rf_policy_passed"]
}
```

Neste exemplo ilustrativo, `conditions` são restrições/precondições do perfil e do uso, não provas de `supported`, `verified` ou `operational`. `authorized_owner` pertence à avaliação de autorização; `audio_route_ready`, à elegibilidade da rota; `rf_policy_passed`, à policy aplicável. A inclusão de um código nessa lista não prova que a condição foi satisfeita. A avaliação de readiness do serviço também é independente. Os campos do exemplo não definem por si só um schema normativo.

## Máquina de decisão
Não há uma cadeia única de estados `SUPPORTED → AUTHORIZED → ROUTE_AVAILABLE → RESOURCE_HEALTHY → READY`: ela mistura evidência de capability, autorização, rota e prontidão. Avalie essas dimensões separadamente. `UNSUPPORTED` pode descrever ausência de suporte declarado; `NOT_AUTHORIZED` é resultado da decisão de acesso; `WAITING_FOR_GATEWAY` e `UNAVAILABLE` descrevem dependência/rota ou disponibilidade contextual conforme o escopo observado; `REQUIRES_LICENSE` indica restrição de policy/licença pendente. `READY` deve ser reservado à prontidão do serviço no escopo próprio. Nenhum desses resultados deve promover ou substituir uma dimensão diferente.

## Interfaces do app
Exibir as funções habilitadas e explicações de indisponibilidade. **Nunca** apresentar botão de falar para uma tag BLE ou LoRaWAN sem suporte de áudio. Capacidade transmitida por nó não substitui prova de hardware nem autorização de rádio.

## Motor de roteamento como grafo
Nós são dispositivos, gateways, salas **GodMesh nativas**, servidores e, opcionalmente, endpoints de bridges Mumble/FRN. Arestas indicam serviço (voz, mensagem, localização), direção, codec, latência, capacidade, autorização e validade. Um caminho de voz só é válido se **todas** as arestas suportam áudio e são permitidas.

## Contrato com GodMesh Native
O roteador decide por capacidade do **serviço nativo GodMesh**, e bridges aparecem como *arestas externas opcionais*, com `authorization_scope`, `tx_policy`, `bridge_id`, origem, codec e validade. Descoberta de Mumble/FRN não modifica diretamente o estado de autorização do usuário. Para voz, validar também permissões de canal, contexto RF e concessão PTT.
