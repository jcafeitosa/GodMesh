# Histórico da documentação

## Cliente único em tecnologia web — 17/11/2026
- ADR-068: React 19 + Astryx + shadcn/ui + Tailwind, empacotado com Tauri 2 (macOS, Windows, Linux, iOS, Android) e núcleo Rust embarcado via comandos/eventos tipados; plugins nativos só onde o SO não expõe API web. **Supera a ADR-055.**
- Emenda à ADR-067: shadcn/ui permanece, com regra de convivência (um componente, um dono; tema único; fronteira `web/ui/`; remoção quando o Astryx cobrir o padrão).
- Versões resolvidas: Tauri CLI/API 2.12.5, Astryx 0.7.16, Tailwind 4.3.5, React 19.3.1, Next 16.4.6, TypeScript 7.0.2, StyleX 0.19.2.
- Pendente: novas versões de SPEC-013, SPEC-014, SPEC-015 e SPEC-016 no modelo web+Tauri; ajuste de SPEC-007/012/024 para plugin nativo. Nada instalado, nada compilado.

## Stack de interface web — 17/11/2026
- ADR-067: Astryx (`@astryxdesign/core` 0.7.16, MIT, React 19+) como base de componentes; Tailwind 4.3.5 para utilitários e layout via `className`; tema por CSS custom properties.
- coss.com e alignui.com rebaixados a referência de padrão visual/composição; coss.com registrado também como candidato de integração de API (e-mail/SMS/forms/webhooks), não como UI.
- shadcn/ui não adotado como base nesta decisão; convivência depende de ratificação do usuário.
- Versões resolvidas em 17/11/2026 por `npm view`: core/CLI Astryx 0.7.16, StyleX 0.19.2, React 19.3.1, Next 16.4.6, TypeScript 7.0.2. Nada instalado, nada compilado.

## Decisões do usuário e fechamento de gaps — 17/11/2026
- ADR-064: SOS offline negado quando não há fonte de frescor confiável (F1/F2) e não há verificação online; UI declara o estado antes do incidente.
- ADR-065: política inicial de retenção por classe (mensagem 24 m, posição 30 d, áudio não retido, SOS 12 m, auditoria 12/24 m, backup 35 d), com revisão jurídica por jurisdição e redução permitida por tenant.
- ADR-066: operação mundial com região primária Brasil (São Paulo) e secundária Colômbia (Bogotá); residência declarada por tenant e contrato por país.
- Emendas: ADR-060 (enum `confidentiality` e herança transitiva), ADR-063 (`generation` + piso de versão contra downgrade).
- SPECs 001–024: casos por decisão e casos por requisito com P/N/B/F; casos positivos dependentes de parâmetro OPEN passam a esperar `param_not_configured`.
- Criados: SPEC-022 (produto/organizações), SPEC-023 (capacidades/conectividade), SPEC-024 (CrowdFind); contratos `autoridade-contratos.md` e `eventos-sincronizacao.md` v0.2.
- Primeiro commit do repositório registra esta base. Nenhuma SPEC aprovada, nenhum código, nenhum teste executado.

## Rodada de gaps e decisões — 17/11/2026
- Varredura completa por 8 frentes (produto/expansão, integrações, defeitos documentais, escala/resiliência, segurança/conformidade) e 3 debates técnicos com fonte primária.
- Registradas ADR-056 a ADR-063: N/N-1 por perfil com rejeição antes do efeito; posição canônica de evento e cursor; taxonomia de ACK por fronteira; escopo do grant offline; E2EE nativo e proibição de alegar confidencialidade indevida; fronteira transacional e recuperação; retenção fail-closed; baseline de cadeia de suprimento.
- Criado `09-equipe/registro-gaps.md` com 113 itens classificados, dono, severidade e ordem de execução; decisões de usuário isoladas com recomendação.
- Corrigido o snapshot de hashes de `SPECS.md` (15 de 21 divergiam), removidos caminhos absolutos de máquina, indexados `aceites-comportamentais.md` e `credenciais-crowdfind.md`.
- Nenhuma SPEC aprovada, nenhum código, nenhum teste executado.

## Organização da equipe — 17/11/2026
- Criado `09-equipe/EQUIPE.md`: orquestração, 8 pods de especialistas por domínio, 4 críticos independentes com veto, workers de integração/matriz/harness e implementadores condicionados ao gate.
- Registrados estado real do projeto, fluxo de trabalho, definição de pronto de SPEC, regras de evidência, escalonamentos abertos e roster de threads.
- Documento de organização: não aprova arquitetura, SPEC, implementação ou teste. Nenhum código foi escrito, nenhuma SPEC foi aprovada e nenhum teste foi executado.

## Atualização documental — plataformas de entrega
- Registrada ADR-055: servidor em contêiner Linux; macOS universal Apple Silicon/Intel com SwiftUI; Windows/Linux; iOS com SwiftUI; Android.
- Separadas decisões explícitas de propostas WinUI 3, Qt Quick e Kotlin/Jetpack Compose, com fontes oficiais e critérios ainda pendentes.
- Alinhados arquitetura, visão, serviços, implantação, roadmap, índice e rastreabilidade; preservado desenvolvimento local com dependências no OrbStack. Não houve implementação, instalação ou execução de testes.

## 3.8.0 — 15/11/2026
- Consolidada a experiência de operação normal do aplicativo: telas, modos simultâneos, inicialização, operação offline, energia adaptativa e privacidade.
- Especificada integração com Dynamic Island/Live Activities (iOS) e Android Live Updates/notificações, sem presença persistente ou dados privados no bloqueio.
- Concluída especificação lógica Wi-Fi AutoDiscover, AutoShare, AutoConnect, incluindo APIs, integração Kotlin/Swift/Router Agent, consentimento e segurança.
- Corrigida a publicação de pontos de acesso: coordenadas exatas no cadastro privado e **exatas no mapa somente mediante opt-in do administrador comprovado**, independente da regra dos 50 km de localização pessoal do chat.
- Criado registro de sessões Wi-Fi, distinguindo pedido de conexão, associação declarada pelo cliente, autenticação atestada pelo AP e consumo efetivamente medido; sem histórico de navegação.
- Acrescentados contratos GMP/REST, cenários de teste, requisitos RF-034 a RF-043, ADR-047 a ADR-054 e rastreabilidade. Não são funcionalidades entregues.


## 3.7.0 — 15/11/2026
- Consolidados DICE, Guardian, fusão de sensores, CrowdFind próprio e Beacon temporário com privacidade/consentimento.
- Formalizados ADR-042..046 e requisitos RF-027..033: contexto terra/ar/mar; acompanhamento voluntário; identidade BLE rotativa; monitoramento de operadora e contingência.
- Criada camada Resilient Connectivity com modo satélite por evidência, diferença entre Direct-to-Cell e internet Starlink por Wi-Fi, fallback offline e priorização de SOS.
- Regra de intervenção humana: só automatizar quando APIs oficiais permitem e usuário autorizou; orientar manualmente em todas as outras situações. Nenhum controle de aplicativos terceiros, de assinatura satelital ou de seleção de operadora.
- Acrescentados documentos de arquitetura, CrowdFind, privacidade, contratos e testes móveis; referências técnicas oficiais; links internos verificados.

## 3.5.2 — 15/11/2026
- ADR-041: localização opcional nos chats e registros PTT sempre aproximada (incerteza mínima de 50 km de raio); posição original nunca na timeline nem em bridges.
- SOS: localização mais precisa disponível, com qualidade e horário reais, apenas a atendentes designados e autenticados; canais, eventos, APIs e auditoria separados.
- Adicionados `04-seguranca/politica-localizacao.md` e `07-contratos/localizacao-privacidade.md`; atualizados SOS, transcrição, UDNL, eventos, contratos, segurança, ADRs e README.


## 3.5.1 — 15/11/2026
- Formalizada ADR-040: cada transmissão PTT gera entrada de timeline independente da transcrição; quando ASR falha, é desativado ou não reconhece a fala, o chat registra transmissor/origem verificada, horário, duração conhecida e motivo de indisponibilidade.
- Para rádios analógicos sem identidade verificável, registrar a atividade da estação/gateway sem atribuir a fala a uma pessoa; dados de atividade submetidos a ACL e retenção própria.
- Contratos de eventos, transcrição e serviço SOS alinhados a essa regra.

## 3.5.0 — 15/11/2026
- Formalizada a **Transcrição Multimodal por Canal** (ADR-038): chat cronológico de voz/texto, ASR separado do plano de mídia, GMC/identidade externa com proveniência, políticas de consentimento, retenção e E2EE.
- Formalizado **GodMesh SOS** (ADR-039): evento estruturado independente da transcrição, prioridade com filas reservadas, localização opcional, reconhecimento humano distinto de ACK de rede/dispositivo, escalonamento e modo teste.
- Criados documentos de transcrição, serviço SOS e contratos GMP/API; atualizados arquitetura, eventos, mídia, segurança, testes e requisitos.

## 3.4.0 — 15/11/2026
- Criada **Universal Data Normalization Layer (UDNL)**, padrão bidirecional para identidade externa, eventos, proveniência e projeção com perdas explícitas.
- A normalização preserva ID original, contexto da rede/servidor, evidência de confiança e não atribui GMC a perfis externos por semelhança.
- Reafirmada separação do plano de mídia PTT, evitando converter todos os frames em eventos genéricos.
- Atualizados índice, arquitetura de referência, gateway universal, eventos e ADR-037.

## 3.3.0 — 15/11/2026
- Definido **indicativo digital pseudônimo GMC** para identificar usuários sem divulgar identidade civil na comunicação nativa; GMI permanece como ID interno.
- Definida transmissão de identidade com `TalkStart` autenticado e referência efêmera nos frames, em vez de repetir identificador estável em cada pacote de áudio.
- Endereços `acct:`/WebFinger passam a ser aliases de descoberta **opt-in**; reforçadas proteção contra impersonação, correlação, replay e limitações de bridges externas.
- Criados capítulos [GMC](02-dominio/indicativos-digitais.md) e [transporte da identificação](01-arquitetura/identificacao-midia-mensagens.md), atualizados contratos, índice, GMP, mídia nativa e ADR-035/036.

## 3.2.0 — 15/11/2026
- Formalizado **GodMesh Global Identity (GMI)**: UUIDv7/URN permanente; nome público no padrão `acct:apelido@domínio`, WebFinger, OpenID Connect (`iss` + `sub`) e passkeys/WebAuthn.
- Estabelecida distinção entre identidade, alias, autenticação e marca GodMesh verificada por federação, sem depender de e-mail, telefone, rádio ou endereço IP.
- Atualizados índice, modelo de dados, contratos, GMP, segurança, rastreabilidade, ADR-034 e referências.

## 3.1.0 — 15/11/2026
- Esclarecimento oficial de **tecnologia própria**: GMP, Voice, roteamento e federação são semântica e motores nativos do GodMesh **sobre padrões modernos e evolutivos**, sem reinventar transportes/criptografia/codecs.
- Mumble, FRN, ChirpStack e quaisquer sistemas externos são **apenas bridges/adaptadores opcionais**, isolados do caminho GodMesh↔GodMesh.
- Criada [Estratégia tecnológica](01-arquitetura/estrategia-tecnologica.md); revisados arquitetura de referência, protocolo, transporte, visão, requisitos, bridges, ADRs e rastreabilidade.
- Adoção de protocolos futuros condicionada a implementação, negociação, conformidade, testes, rollout e segurança.

## 3.0.0 — 15/11/2026
- Revisão integral do brainstorming; estabelecida arquitetura de referência normativa e sistema completo (sem definição por escopo reduzido).
- GodMesh Native e GMP são o núcleo; Mumble/FRN definitivamente opcionais por bridges.
- Requisitos, visão, glossário, roadmap, contratos, mídia e modelos alinhados à decisão.
- Especificada separação entre software espelhado, dados escopados, plano de mídia, autoridade de escrita e controle.
- Adicionadas matriz de rastreabilidade, gates de verificação e regras de prevalência documental.
- Mantidos como propostas os componentes ainda não implementados/validados.

## 0.2.0 — 15/11/2026
- Decidido desenvolver o **protocolo de aplicação GodMesh (GMP)** e um **motor PTT nativo em Rust**.
- Mumble e FRN passam de infraestrutura de voz principal a **bridges externas opcionais**.
- Criados documentos de arquitetura nativa e Bridge SDK; revisados índice e ADRs para preservar histórico sem ambiguidade.
- Registrados limites de E2EE em bridges, políticas anti-loop, PTT com leases/fencing e isolamento regulatório de rádio.


## 0.1.0 — 15/11/2026
- Registro inicial do brainstorming GodMesh.
- Consolidado núcleo Rust em Linux, plataforma consumer + enterprise, modelo de ativos/dispositivos/transportes e universal gateway.
- Documentadas malha mundial de servidores full-stack e operação active-active por região, descoberta, IPv6 preferencial, IPv4 fallback, replicação escopada e balanceamento.
- Selecionados Mumble e FRN como primeiras integrações de voz PTT; ChirpStack/LoRaWAN previsto para dados.
- Adicionados modelo de dados, contratos de eventos/API, requisitos, ADRs, roadmap, segurança, riscos e testes.
- Todos os elementos foram documentados como propostas/decisões de arquitetura, sem alegar implementação produtiva.
