# GodMesh Guardian — sessão temporária de acompanhamento

> **GodMesh 3.6.0 · especificação de produto, não sistema de emergência em operação.** O compartilhamento é voluntário, limitado por finalidade, audiência e duração.

## Propósito
Uma pessoa que inicia uma atividade — trilha, caminhada, deslocamento, viagem ou outra atividade informada — pode autorizar contatos escolhidos a acompanhar sua posição durante um período definido, sem expor coordenadas precisas a canais públicos. A mesma plataforma atende contas pessoais e equipes, mas a administração empresarial não elimina requisitos de autorização e transparência.

## Estados de sessão
`draft -> consent_pending -> scheduled -> active -> paused -> active -> completed | expired | revoked`.
Estados de conectividade são ortogonais: `online`, `offline_buffering`, `stale`, `unknown`. `overdue` é um alerta de check-in, **não um SOS**.

## Configuração obrigatória
- Proprietário da sessão (GMI, não nome civil público), `guardian_session_id` único e pseudônimo GMC exibido quando adequado.
- Ambiente operacional (`land/air/sea`) e atividade selecionada.
- Inicio, expiração, prazo estimado de retorno e tolerância configurável.
- Lista de destinatários convidados e aceitos, com prazo e escopo individual.
- Precisão compartilhada: `precise` ou `coarse_50km`; escolher o menor privilégio suficiente.
- Permissões independentes: localização, histórico, notificações de ausência/check-in e detecção Beacon.
- Frequência de atualização adaptada a bateria/cobertura; avisos sobre consumo e incerteza.
- Revogação e encerramento acessíveis com um comando visível ao usuário.

## Fluxo normal
1. Criar sessão local `draft` e selecionar voluntariamente contatos que poderão acompanhar.
2. Convidados recebem convite autenticado, com finalidade, período e escopo; não ganham rastreamento permanente.
3. Com autorização e horário válido, o app coleta GNSS e sensores permitidos. Processamento de fusão prioriza execução local.
4. O servidor valida audiência e TTL antes de permitir a leitura da posição precisa. Público e bridges recebem, no máximo, localização aproximada autorizada ou nenhuma.
5. Offline: pontos/observações permanecem na fila local criptografada e sincronizam quando existir caminho legítimo. A tela mostra a idade e a qualidade da última posição.
6. Em `expired`, `revoked` ou `completed`, encerrar imediatamente futuras consultas e desativar o Beacon da sessão; aplicar retenção e purga conforme política.

## Check-ins e atraso
- O usuário pode confirmar que chegou ou que está bem. Um contato autorizado pode receber aviso de check-in pendente.
- Quando o prazo é ultrapassado, o estado torna-se `overdue` e os contatos previamente escolhidos recebem **notificação de ausência de confirmação**; o sistema **não infere emergência** nem aciona polícia/resgate automaticamente.
- Falta de internet/sinal pode explicar uma posição desatualizada; mostrar essa possibilidade explicitamente.
- Um SOS somente inicia pelo [serviço SOS](servico-sos.md) segundo suas próprias condições e controles.

## Beacon opcional
Ativar `guardian_beacon` é um consentimento separado de `guardian.precise`. Seu papel é permitir observações de proximidade por outros dispositivos participantes. Nenhum anúncio BLE deve expor GMC, identidade civil, longitude/latitude, contato ou ID permanente. Consultar [GodMesh CrowdFind](../03-integracoes/crowdfind-beacon.md).

## Modelo de dados complementar
- `guardian_sessions(id, owner_gmi, environment, activity, started_at, expires_at, expected_checkin_at, state, privacy_mode, policy_version)`.
- `guardian_recipients(session_id, recipient_gmi, grant_id, scope, accepted_at, expires_at, revoked_at)`.
- `guardian_checkins(id, session_id, reported_at, kind, origin, event_id)`.
- `guardian_observations(id, session_id, device_id, source, position_encrypted, uncertainty_m, measured_at, received_at)`.
- `guardian_alerts(id, session_id, alert_type, status, created_at, target_scope)`.
- `guardian_beacon_sessions(session_id, beacon_key_ref, enabled_at, stopped_at, rotation_policy)`.

## Contratos e aceite
Ver [contratos GMP](../07-contratos/guardian-crowdfind.md) e [testes do Guardian](../05-operacao/testes-mobile-guardian.md). Aceitação exige fim automático, revogação imediata de novas leituras, identidades de detectores protegidas, verificação de autorização e distinção entre `overdue` e SOS.

## Contingência de rede durante Guardian
Quando a operadora perder cobertura ou houver conexão limitada, o [Resilient Connectivity](../01-arquitetura/conectividade-contingencia-satelite.md) ajustará o tráfego **interno** conforme capacidades e prioridade. O usuário poderá receber instruções para ativar economia de energia, verificar modo satélite oferecido pela operadora, reduzir streaming e downloads e manter ativos os recursos de navegação, contato e SOS. A orientação não altera configurações do SO sem consentimento, não substitui assistência oficial e não garante que o modo satélite esteja disponível ou transporte dados do GodMesh.
