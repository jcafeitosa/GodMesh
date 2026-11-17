# Contexto operacional: terra, ar e mar

> **GodMesh 3.6.0 · decisão de produto e especificação proposta.** `operational_context` descreve a **atividade atual**, não uma característica permanente da pessoa. Este módulo não autoriza operações de rádio, aviação ou navegação.

## Objetivo
O GodMesh deve adaptar interface, sensores, conectividade, exibição de rotas e opções de acompanhamento ao ambiente da atividade. A pessoa pode ter atividades em terra, no ar e no mar ao longo do tempo. Um dispositivo aéreo controlado do solo **não** transforma o operador em usuário fisicamente aéreo.

## Modelo conceitual
- `environment`: `land | air | sea | unknown`.
- `activity`: classe declarada, ex.: `hiking`, `walking`, `cycling`, `road_travel`, `flight_passenger`, `aviation_authorized`, `boat_trip`, `navigation`, `custom`.
- `role`: `participant | passenger | operator | observer | unknown`; sua validade pode depender de credenciais externas.
- `source`: `user_selected | device_profile | inferred_suggestion`. Inferências não substituem a confirmação quando exigida.
- `effective_from` e `effective_until`, atualizadas por sessão.
- `mobility`: `moving | stationary | uncertain` e métricas com precisão e timestamp.
- `route_plan_id` e `expected_checkin_at`, quando o usuário informar.

## Regras
1. A seleção manual possui prioridade sobre hipóteses calculadas por sensores; o usuário pode revisar a atividade e a audiência.
2. Velocidade, altitude, tipo de equipamento ou nome de rede **não comprovam** se a pessoa está numa aeronave ou embarcação. A interface não atribuirá status de piloto nem habilitará frequências com base nessa inferência.
3. Localização precisa de atividades fica no escopo `guardian.precise`, com consentimento e destinatários temporários. **Chat/PTT normal mantém somente área consentida com raio de incerteza >= 50 km.**
4. O contexto operacional não implica transmissão automática de posição, acionamento SOS ou acesso a redes de rádio.
5. Contexto ausente permanece `unknown` e não bloqueia funções fundamentais do app.

## Dados auxiliares por ambiente
| Ambiente | Dados úteis, se disponíveis | Limitação |
|---|---|---|
| Terra | GNSS, passos, velocidade, elevação relativa, rota prevista | Sensores inerciais acumulam deriva |
| Ar | GNSS, velocidade, altitude e rota planejada quando legal e tecnicamente disponível | O uso de dispositivos/redes a bordo depende das regras aplicáveis |
| Mar | GNSS, rumo, velocidade e rota declarada | Cobertura celular e Wi-Fi pode ser inexistente |

## Integrações
- [Guardian: acompanhamento temporário](guardian-acompanhamento.md)
- [Sensores e conectividade do cliente](../01-arquitetura/cliente-mobile-sensores-conectividade.md)
- [Trajetória e fusão de sensores](../01-arquitetura/guardian-localizacao-trajetoria.md)
- [Política de localização](../04-seguranca/politica-localizacao.md)

## Pendências
Taxonomia final de atividades, acessibilidade, regras por país e compatibilidade com dispositivos específicos exigem pesquisa e validação.
