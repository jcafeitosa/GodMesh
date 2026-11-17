# GodMesh Live Presence — Dynamic Island, Live Activities e Android Live Updates

> **LIVE-ARC-001 · Versão 1.0 · 2026-11-15 · Estado: especificação de UX e integração de plataforma; não implementada.**

## 1. Objetivo

Exibir o estado **de atividades em andamento**, sem exigir abrir o aplicativo. No iOS usar ActivityKit/WidgetKit para Live Activities (Dynamic Island quando houver hardware e visualização compatíveis; tela bloqueada nos demais). No Android utilizar Live Updates em aparelhos e versões elegíveis, com notificações/serviços adequados nas versões que não suportarem o recurso.

**Não** ativar uma atividade persistente somente para mostrar “GodMesh online”; mensagens comuns usam notificações normais.

## 2. Prioridade visual

| Prioridade | Atividade | Informação permitida |
|---|---|---|
| Crítica | SOS explícito em andamento | Número curto/estado local vs entregue vs assumido; tempo; ação segura para abrir detalhes |
| Alta | Guardian em andamento | Atividade, duração/check-in, status de sincronização, sem coordenadas precisas |
| Alta | PTT/chamada em andamento | Nome do canal, indicação de transmissão e GMC, respeitando privacidade |
| Transitória | Provisionamento Wi-Fi solicitado | Conectando, aprovação do SO exigida, erro e encerramento |
| Normal | Mensagem ou AP detectado | Notificação padrão; **não** Live Activity permanente |

Se atividades coexistirem, mostrar SOS em primeiro plano e permitir retornar aos detalhes das demais sem perder o estado. Exposição em tela bloqueada pode ser reduzida ainda mais pela preferência do usuário e pelas políticas do SO.

## 3. Ciclo de vida e contrato

Estados: `not_supported`, `permission_required`, `eligible`, `starting`, `active`, `stale`, `ending`, `ended`, `failed`.

Um objeto `LivePresenceProjection` contém: `activity_id`, `kind`, `started_at`, `expires_at?`, `state`, `summary_redacted`, `last_update_at`, `deep_link`, `priority` e `source_event_id`; **não** transporta segredos nem coordenadas. O servidor emite eventos GMP; a camada nativa decide se a atividade é elegível pelo SO.

A UI deve manter `stale` quando falta atualização e nunca exibir entrega de SOS que não esteja tecnicamente confirmada. Se o SO descartar a Live Activity, o estado principal permanece acessível dentro do aplicativo.

## 4. Implementação por plataforma

### iOS

- Swift/ActivityKit/WidgetKit + SwiftUI para iniciar, atualizar e terminar atividades elegíveis.
- Não assumir que todas as versões ou modelos suportam Dynamic Island.
- Não tentar manipular a Dynamic Island diretamente fora das APIs de Live Activities.
- Conteúdo protegido, deep links autenticados e `staleDate` usado quando apropriado.

### Android

- APIs oficiais de Live Updates para atividades **em andamento, iniciadas pelo usuário e sensíveis ao tempo**; elegibilidade depende do Android/dispositivo.
- Para PTT/Guardian/SOS, selecionar notificação apropriada conforme política do SO, sem fingir que a tela de todos os aparelhos é igual.
- Evitar Live Updates para mensagens avulsas, notícias de rede ambiente ou presença contínua; o Android as desaconselha para essas finalidades.
- Respeitar dispensa de notificação e preferências, com fallback dentro do app.

## 5. Segurança, acessibilidade, testes

- Não exibir localização exata, identidade civil, SSID privado, BSSID, tokens ou usuários do histórico Wi-Fi no lock screen.
- Textos curtos, suporte a leitor de tela, contraste, idiomas e ações confirmadas quando sensíveis.
- Testar atualizações atrasadas, expiração, bloqueio, falta de rede, revogação de permissões, múltiplas atividades e fabricantes Android diferentes.

## Referências oficiais (consultadas em 2026-11-15)

- [Apple ActivityKit — Live Activities](https://developer.apple.com/documentation/activitykit/displaying-live-data-with-live-activities)
- [Android — Create Live Update notifications](https://developer.android.com/develop/ui/views/notifications/live-update)

[Aplicativo normal](aplicativo-operacao-normal.md) · [Contratos Live Presence](../07-contratos/live-presence.md) · [Testes](../05-operacao/testes-app-live-presence.md).
