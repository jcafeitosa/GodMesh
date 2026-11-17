# Testes — funcionamento normal do app e Live Presence

> Matriz a executar. Não constitui evidência de implementação.

## Normal e coexistência
- [ ] Abrir sem internet: dados locais disponíveis, estado Offline, outbox pendente honesto.
- [ ] Inicialização usa identidade local sem divulgar GMI e sincroniza de forma idempotente.
- [ ] Sem permissões BLE/localização: chat funciona e UI explica recursos não habilitados.
- [ ] PTT e Guardian ativos simultaneamente; SOS preserva estado e assume prioridade.
- [ ] Fim de sessão Guardian revoga localização temporária sem apagar histórico com retenção legal permitida.
- [ ] ASR falho: linha de voz continua no chat com origem/identificador e motivo.
- [ ] Bridges FRN/Mumble indisponíveis: canais GodMesh nativos operam normalmente.

## iOS / Android
- [ ] Modelo compatível mostra Live Activity/Live Update só para atividades elegíveis e em andamento.
- [ ] Modelo sem Dynamic Island/Live Updates usa alternativa suportada.
- [ ] Tela bloqueada não apresenta posição precisa do SOS/Guardian, credenciais ou auditoria de Wi-Fi.
- [ ] Atividade de SOS distingue registrado, encaminhado, confirmação técnica e atendimento humano.
- [ ] UI e deep link continuam corretos após reboot, kill do processo, mudança de rede e expiração.
- [ ] Live Update dispensada pelo usuário não é reposta indevidamente.
- [ ] Bateria baixa reduz tarefas opcionais sem cortar acesso a recursos oficiais de emergência.

## Evidência
Registrar sistema operacional, versão, modelo, permissão, modo background, canal, evento redigido, resultado esperado/observado, duração e captura sem dados privados.

[Arquitetura](../01-arquitetura/aplicativo-operacao-normal.md) · [Live Presence](../01-arquitetura/live-presence.md).
