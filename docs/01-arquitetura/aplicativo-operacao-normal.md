# GodMesh App — operação normal, modos simultâneos e experiência de usuário

> **APP-ARC-001 · Versão 1.0 · 2026-11-15 · Estado: especificação aprovada, implementação pendente.** Vinculado à [arquitetura de referência](arquitetura-referencia.md), [GMP](protocolo-nativo.md) e [política de localização](../04-seguranca/politica-localizacao.md).

## 1. Princípio

O aplicativo é uma interface cotidiana para comunicação, redes, localização e dispositivos, **não** apenas uma ferramenta de emergência. Um usuário não precisa escolher servidores, IPs, codec ou bridge. GodMesh resolve conectividade e capacidade sob políticas de segurança e apresenta estados reais, inclusive quando está offline.

## 2. Navegação e superfícies

| Área | Conteúdo nativo | Condições de exibição |
|---|---|---|
| Início | Estado da conectividade, GMC público, mensagens recentes, sessões, avisos | Respeitar preferências e autorização |
| Comunicações | Chat por canal, PTT próprio, transcrição e registros mesmo sem ASR | Bridge Mumble/FRN isolada e opcional |
| Mapa | Dispositivos próprios, Guardian autorizado, pontos Wi-Fi publicados, gateways autorizados | Camadas e precisão conforme política por tipo de dado |
| Redes | Wi-Fi AutoDiscover/AutoShare/AutoConnect, conectividade IP, redes externas | Sem fingir varredura Wi-Fi irrestrita no iOS |
| Dispositivos | Inventário, perfis, status e capacidades verificadas | Possível operação offline parcial |
| Guardian | Sessão temporária, contatos escolhidos, ETA, check-in e Beacon | Consentimento, prazo, revogação e baterias |
| SOS | Acionamento protegido, status de entrega e acompanhamento do incidente | Prioritário, sem promessa de socorro garantido |

## 3. Modos: não usar enum exclusivo

O estado real é composto por **atividades simultâneas**, não por um único `AppMode`. Exemplo: PTT ativo + Guardian ativo + conectividade degradada. O SOS adiciona uma política de prioridade sem destruir a sessão Guardian.

```rust
pub struct ClientActivities {
    pub ptt_sessions: Vec<String>,
    pub guardian_session: Option<String>,
    pub sos_incident: Option<String>,
    pub wifi_provisioning: Option<String>,
    pub offline_outbox_count: u64,
}

pub enum NetworkQuality { Online, Degraded, LocalOnly, Offline }

pub struct ClientSnapshot {
    pub activities: ClientActivities,
    pub network: NetworkQuality,
    pub battery_saver_recommended: bool,
}
```

Essas estruturas são ilustrativas e não substituem os contratos de domínio ou a implementação móvel.

## 4. Inicialização

1. Restaurar identidade criptográfica local e indicativo GMC, sem expor GMI internamente na interface pública.
2. Ler permissões previamente concedidas e declarar `supported`, `permission_required` ou `unavailable` por capacidade.
3. Carregar dados criptografados locais: chats autorizados, dispositivos e fila de eventos ainda não sincronizados.
4. Identificar rede atual e rotas disponíveis (IPv6 preferencial, IPv4 compatível), sem bloquear a tela inicial.
5. Conectar-se a um nó elegível; autenticar e sincronizar de maneira incremental e idempotente.
6. Ativar tarefas por necessidade, política energética e limite do SO; não executar varredura contínua.
7. Mostrar prontidão e falhas **por serviço**, não um selo genérico de “tudo conectado”.

## 5. Primeiro plano / segundo plano

| Serviço | Primeiro plano | Segundo plano |
|---|---|---|
| Chat | Sincronização imediata | Push e trabalhos permitidos pelo SO |
| PTT | Áudio + controles | Áudio de sessão ativa se o SO/permissões permitirem |
| Wi-Fi | Inventário observável e conexão assistida | Eventos e observações limitadas; sem varredura constante |
| Guardian | Posição e acompanhamento autorizado | Atualização permitida por políticas de localização e energia |
| CrowdFind | Anúncio/observação consentidos | Restrições do aparelho, sobretudo em iOS |
| SOS | Interface de incidente | Persistência local e tentativas autorizadas; não garantir transmissão |

## 6. Priorização e energia

`P0`: incidente SOS ativo, estado e fila mínima de encaminhamento. `P1`: comunicação em andamento e check-ins Guardian. `P2`: mensagens e sincronização autorizadas. `P3`: descoberta de Wi-Fi, telemetria voluntária e tarefas de manutenção.

Com bateria ou conexão limitada, o cliente reduz trabalho **seu**, não controla aplicativos de terceiros: suspender ASR remoto não essencial, uploads pesados, varreduras frequentes e anexos; continuar oferecendo serviços de emergência e navegação do sistema. Quando exigir intervenção manual, explicar sem afirmar que foi executada.

## 7. Regras de interface e privacidade

- **Chat comum**: posição opcional aproximada com incerteza radial nominal >= 50 km; não adicionar GNSS exato ao evento de chat.
- **Guardian**: posição precisa somente para destinatários escolhidos, durante o prazo e enquanto válida a autorização.
- **SOS**: melhor posição disponível em **vault** restrito, não no chat público.
- **Wi-Fi Share**: pontos de acesso com posição interna precisa; mapa pode mostrar ponto preciso **apenas** com consentimento explícito do controlador/proprietário.
- **Registro de quem acessou Wi-Fi**: administradores veem auditoria operacional protegida, não histórico de navegação.
- **Tela bloqueada**: nunca expor localização precisa, senhas, listas de acessos ou identidade civil.

## 8. Critérios de aceite

- A UI abre offline mostrando dados locais e estado `Offline`; não marca mensagens pendentes como entregues.
- PTT + Guardian coexistem; SOS recebe prioridade sem apagar os estados anteriores.
- Usuários sem Bluetooth/localização conseguem usar funcionalidades não dependentes dessas permissões.
- Revogação de acompanhamento impede novas leituras autorizadas; histórico existente segue retenção estabelecida.
- Registro de áudio PTT aparece no chat mesmo sem transcrição.
- Mumble/FRN indisponíveis não tornam o GodMesh Native indisponível.
- Inexistência de Wi-Fi ou UWB suportado é apresentada como limitação da plataforma, não erro genérico.

## Referências cruzadas

[Live Presence](live-presence.md) · [Wi-Fi Share](wifi-autodiscover-autoshare.md) · [Guardian](../02-dominio/guardian-acompanhamento.md) · [SOS](../02-dominio/servico-sos.md) · [Testes do aplicativo](../05-operacao/testes-app-live-presence.md).
