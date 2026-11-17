# GodMesh — documentação técnica

> **Versão documental 3.8.0** · **15 de novembro de 2026** · **Escopo: sistema integral e avançado**. Esta documentação registra a visão e especificações propostas; **não** afirma implementação concluída ou validação em produção.

> A publicação inicial inclui documentação de produto, arquitetura, domínio, integrações, segurança, operação, contratos e planejamento (`06-planejamento/`). Configurações locais de agentes, rascunhos de trabalho e contribuições externas ainda não integradas ao repositório não fazem parte deste conjunto.

**GodMesh = tecnologia própria de aplicação, comunicação e federação em Rust, construída sobre padrões modernos e evolutivos + plataformas externas conectadas exclusivamente por bridges/adaptadores opcionais.**

## Comece aqui
1. [Arquitetura de referência — documento normativo](01-arquitetura/arquitetura-referencia.md)
1a. [Estratégia tecnológica — significado oficial de protocolo próprio](01-arquitetura/estrategia-tecnologica.md)
2. [Visão integral do produto](00-produto/visao.md)
3. [Requisitos rastreáveis](00-produto/requisitos.md)
4. [GodMesh Protocol (GMP)](01-arquitetura/protocolo-nativo.md)
5. [GodMesh Voice nativo](01-arquitetura/midia-nativa.md)
6. [Arquitetura normativa](01-arquitetura/arquitetura-referencia.md) e decisões registradas nela

## Plataformas de entrega

Plataformas propostas: servidor em contêiner Linux; **cliente único em tecnologia web (React 19 + Astryx + shadcn/ui + Tailwind) empacotado com Tauri 2** para macOS, Windows, Linux, iOS e Android, com núcleo Rust embarcado (ADR-068, que supera a ADR-055). Recursos que o sistema operacional não expõe à web — BLE, segundo plano, áudio de PTT, push, Live Activities, NFC — entram como plugins nativos do Tauri. Consulte a [arquitetura de referência](01-arquitetura/arquitetura-referencia.md); isso não afirma implementação.

## Produto
- [Operação normal do aplicativo — estados, telas, offline e energia](01-arquitetura/aplicativo-operacao-normal.md)
- [GodMesh Live Presence — Dynamic Island e Android Live Updates](01-arquitetura/live-presence.md)
- [Visão e produtos Personal/Teams/Node](00-produto/visao.md)
- [Requisitos integrais](00-produto/requisitos.md)
- [Glossário](00-produto/glossario.md)

## Arquitetura distribuída
- [Arquitetura de referência](01-arquitetura/arquitetura-referencia.md)
- [Resumo da arquitetura](01-arquitetura/visao-geral.md)
- [Nós, clusters, regiões e espelhamento](01-arquitetura/nos-regioes.md)
- [IPv6-first, IPv4 e transportes evolutivos](01-arquitetura/transporte.md)
- [Estratégia tecnológica: padrões modernos, extensibilidade e bridges externas](01-arquitetura/estrategia-tecnologica.md)
- [Descoberta, ingresso e ativação autorizada](01-arquitetura/descoberta-ingresso.md)
- [Replicação e consistência](01-arquitetura/replicacao.md)
- [Balanceamento e failover](01-arquitetura/balanceamento.md)
- [Serviços e workspace Rust](01-arquitetura/servicos.md)
- [Camada Universal de Normalização de Dados (UDNL)](01-arquitetura/normalizacao-dados.md)
- [Fronteiras entre Gateway, UDNL, adaptadores e bridges](01-arquitetura/fronteiras-modulos.md)
- [GodMesh Protocol — tecnologia própria](01-arquitetura/protocolo-nativo.md)
- [Wi-Fi AutoDiscover / AutoShare / AutoConnect](01-arquitetura/wifi-autodiscover-autoshare.md)
- [GodMesh Voice — mídia nativa](01-arquitetura/midia-nativa.md)

- [Transcrição multimodal por canal — chat, GMC, ASR e políticas](01-arquitetura/transcricao-multimodal.md)
- [DICE — sensores e interfaces móveis](01-arquitetura/cliente-mobile-sensores-conectividade.md)
- [GodMesh Resilient Connectivity — satélite e orientação manual](01-arquitetura/conectividade-contingencia-satelite.md)
- [Guardian — fusão de sensores e trajetória](01-arquitetura/guardian-localizacao-trajetoria.md)
## Domínio e contratos
- [Modelo de dados Wi-Fi e posição exata de AP](02-dominio/wifi-modelo-dados.md)
- [Registro de acesso e auditoria de Wi-Fi](02-dominio/wifi-registro-acessos.md)
- [API/GMP Wi-Fi Share](07-contratos/wifi-autodiscover-autoshare-api.md)
- [Contrato Live Presence](07-contratos/live-presence.md)
- [Identidade interna GodMesh (GMI) — UUID, OIDC e descoberta opt-in](02-dominio/identidade-global.md)
- [Indicativo digital pseudônimo (GMC) para usuários](02-dominio/indicativos-digitais.md)
- [Identificação em mensagens e áudio PTT — GMP](01-arquitetura/identificacao-midia-mensagens.md)
- [Contexto operacional: terra, ar, mar](02-dominio/contexto-operacional.md)
- [Acompanhamento temporário — GodMesh Guardian](02-dominio/guardian-acompanhamento.md)
- [Modelo de dados](02-dominio/modelo-dados.md)
- [Eventos e comandos](02-dominio/eventos.md)
- [Capacidades e roteamento](02-dominio/capacidades.md)
- [API pública e GMP](07-contratos/api-inicial.md)
- [Contratos Guardian e CrowdFind](07-contratos/guardian-crowdfind.md)
- [Credenciais de CrowdFind — contrato ativo condicionado à SPEC de CrowdFind](07-contratos/credenciais-crowdfind.md)
- [Contratos de contingência de rede e satélite](07-contratos/conectividade-contingencia.md)

- [SOS federado — acionamento, escalonamento e ACK humano](02-dominio/servico-sos.md)
- [Contratos de transcrição e SOS](07-contratos/transcricao-sos.md)
## Gateways e redes externas
- [Wi-Fi Android, iOS e controladores](03-integracoes/wifi-android-ios-roteadores.md)
- [Universal Gateway](03-integracoes/gateway-universal.md)
- [LoRa, LoRaWAN, BLE](03-integracoes/lora-lorawan-ble.md)
- [CrowdFind e Beacon GodMesh](03-integracoes/crowdfind-beacon.md)
- [Mumble e FRN — papel oficial de bridges](03-integracoes/mumble-frn.md)
- [Voice Bridge SDK](03-integracoes/bridges-voz.md)
- [GPS, APRS e outros adaptadores](03-integracoes/outros-adaptadores.md)

## Segurança e operações
- [Privacidade e consentimento Wi-Fi Share](04-seguranca/wifi-compartilhamento-privacidade.md)
- [Testes Wi-Fi Share, mapa e auditoria](05-operacao/testes-wifi.md)
- [Testes do aplicativo e Live Presence](05-operacao/testes-app-live-presence.md)
- [Política de localização — chat 50 km e SOS preciso](04-seguranca/politica-localizacao.md)
- [Contratos GMP de localização aproximada e precisa](07-contratos/localizacao-privacidade.md)
- [Segurança, privacidade e governança](04-seguranca/seguranca.md)
- [Privacidade CrowdFind e antirrastreamento](04-seguranca/privacidade-crowdfind.md)
- [Rádio, licenciamento e integração externa](04-seguranca/uso-das-redes.md)
- [Implantação Linux](05-operacao/implantacao.md)
- [Observabilidade e SLOs](05-operacao/observabilidade.md)
- [Plano de validação integral](05-operacao/testes.md)
- [Testes móveis, Guardian, CrowdFind e satélite](05-operacao/testes-mobile-guardian.md)

## Planejamento e histórico
Documentos de decisões, rastreabilidade, roadmap, pendências, aceites comportamentais e referências estão publicados nesta árvore, em `06-planejamento/`. Eles registram propostas, decisões e estados observados; não comprovam implementação ou execução de testes. O histórico de versões está em [Changelog](CHANGELOG.md).

## Organização da equipe
- [Equipe, papéis, críticos, workers e gates](09-equipe/EQUIPE.md)
- [Registro consolidado de gaps, ação e ordem de execução](09-equipe/registro-gaps.md)
- [Cobertura requisito → SPEC → aceite](09-equipe/cobertura-requisitos.md)
- [Versão, autoria, revisão e hash por SPEC e ADR](09-equipe/registro-versao-e-revisao.md)
- [Aceites comportamentais (registro de cenários planejados)](06-planejamento/aceites-comportamentais.md)

## Regras de prevalência
1. A [Arquitetura de referência](01-arquitetura/arquitetura-referencia.md) prevalece para decisões arquiteturais; os registros documentais descrevem seu estado e suas limitações.
2. **Voz nativa GodMesh** é obrigatória; Mumble e FRN são bridges opcionais.
3. Os nós full-stack são espelhos de **serviços e software**, mas dados privados só são copiados segundo escopo e política.
4. IPv6 é preferencial, IPv4 é compatível; protocolos futuros exigem implementação e verificação.
5. A instalação é automatizada, mas o ingresso na federação exige autorização.
6. O GodMesh é o sistema completo planejado; fases de construção não reduzem o escopo.
7. Nenhuma especificação significa funcionalidade testada ou pronta para emergências.
8. **Tecnologia própria significa domínio/protocolo de aplicação nativos sobre padrões auditados; não criar novos protocolos de transporte ou criptografia sem necessidade.**
9. **Sistemas terceiros nunca fazem parte do caminho obrigatório GodMesh↔GodMesh.** São opcionais e acessados apenas via bridge/adaptador autorizado e isolado.
10. **Usuários possuem ID interno `gmi_id` (UUIDv7), mas são apresentados pelo indicativo digital pseudônimo GMC** nas mensagens e no PTT. `acct:usuário@domínio` é descoberta opcional. Indicativos e permissões são verificados criptograficamente; não são credenciais por si só. Veja [GMI](02-dominio/identidade-global.md) e [GMC](02-dominio/indicativos-digitais.md).

11. **Toda integração externa passa pela UDNL:** preserva o identificador original, proveniência e nível de confiança; entrada e saída são traduções explícitas, e identidade/atributos não suportados não são inventados. Áudio em tempo real usa plano de mídia separado.

12. **Toda transmissão PTT observável deve ser representada na timeline**, conforme os contratos e a arquitetura publicados, com transcrição quando permitida e possível; na indisponibilidade ASR, exibir o registro e o motivo sem inventar conteúdo ou identidade. **SOS** é evento independente com ACK técnico e humano distintos; não substitui serviços públicos de emergência.
13. **Localização no chat/PTT:** somente área generalizada de pelo menos 50 km de raio (quando compartilhada). **SOS:** posição mais precisa disponível apenas a responsáveis autorizados, separada da timeline comum, com auditoria e sem promessa de anonimato garantido.
14. **Guardian:** acompanhamento temporário e revogável, contatos autorizados, atraso de check-in não é SOS.
15. **CrowdFind:** beacons rotativos opt-in e rede própria; não usar redes proprietárias Apple/Google sem integração autorizada.
16. **Conectividade e satélite:** usar somente rotas realmente expostas, com estado de evidência e limitações de aplicativos; não confundir satélite de operadora com Wi-Fi de terminal.
17. **Intervenção manual:** se o sistema operacional não autorizar automação, orientar o usuário; nunca afirmar que ações externas foram executadas; preservar recursos essenciais de navegação, Guardian e SOS.

18. **Wi-Fi Share:** descoberta privada automática não publica rede de terceiros; AP físico tem posição interna precisa quando disponível, publicável exatamente somente com opt-in explícito do administrador; auditoria de visitantes é restrita, sem histórico de navegação.
19. **Live Presence:** ActivityKit e Android Live Updates se aplicam a atividades elegíveis em andamento; notificações e fallback dependem do SO; telas bloqueadas não recebem coordenadas precisas nem credenciais.
20. **Cliente único:** todas as interfaces de usuário são web (ADR-068); plugins nativos existem apenas onde a API do SO não é acessível à web, com contrato tipado e teste próprio. **App normal:** modos coexistem, início offline e estados honestos, com prioridade de SOS sem presumir entrega ou disponibilidade universal.

## Manutenção desta documentação

A árvore `docs/` deste projeto é a fonte documental ativa. O histórico de versões está no [changelog](CHANGELOG.md). Mudanças de intenção arquitetural devem registrar contexto, alternativas, decisão e consequências, conforme as orientações de `AGENTS.md`.

Antes de importar um lote externo, registrar origem, versão ou hash, destino, diferenças e responsável; preservar uma cópia anterior e conferir o resultado. Caminhos temporários de conversas não são uma origem permanente nem instrução automática de sincronização. A nota histórica de importação foi preservada no snapshot anterior à revisão documental de 15/11/2026.

Uma correção documental não comprova implementação ou execução de testes. Propostas, decisões aprovadas e resultados observados devem continuar identificados separadamente.
