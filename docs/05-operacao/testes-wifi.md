# Plano de testes — Wi-Fi AutoDiscover, AutoShare, AutoConnect, mapa e auditoria

> Matriz a executar em Android, iOS, roteadores e clusters regionais. Nenhum resultado foi certificado até execução em dispositivos reais.

## Descoberta / cadastro
- [ ] Permissão recusada: sem inventário, dados brutos ou erro silencioso; app continua funcional.
- [ ] Android expõe apenas scans permitidos e marca resultados desatualizados; limites de throttling respeitados.
- [ ] iOS não anuncia lista de redes salvas ou próximas que não está disponível pelas APIs.
- [ ] Nova rede detectada entra apenas no inventário privado, sem publicação nem transmissão de senha.
- [ ] BSSID/SSID idênticos em múltiplos locais não são confundidos com mesma autoridade de AP.

## Titularidade / compartilhamento
- [ ] Login em Wi-Fi de terceiros não basta para publicar acesso.
- [ ] Pareamento do Router Agent exige aprovação administrativa e prova de posse/autorização.
- [ ] Novos SSIDs herdam apenas políticas explicitamente aprovadas do controlador pareado.
- [ ] Revogação encerra emissão de grants e tenta desativar acesso no controlador; status pendente visível.
- [ ] PSK compartilhada não é anunciada como revogada até rotação/alteração confirmada.

## Posição do AP e mapa
- [ ] AP administrativo guarda latitude/longitude, precisão, origem, responsável, horários e trilha de auditoria.
- [ ] `exact_opt_in` divulga ponto preciso somente após consentimento específico do responsável.
- [ ] `coarse`/`hidden` não vazam coordenadas exatas por API, cache, logs, pesquisa ou federação.
- [ ] Observação de Wi-Fi feita por usuário visitante não redefine automaticamente posição oficial do AP.
- [ ] Mapa diferencia “cadastrado”, “sinal ao alcance”, “associado” e “internet verificada”.

## Conexão
- [ ] Android Network Suggestion gera `os_deciding`; não garante conexão.
- [ ] iOS NEHotspotConfiguration solicita aprovação e respeita negativa.
- [ ] Portal cativo, sem internet e conexão local não são marcados como `internet_verified`.
- [ ] Sem mudança desnecessária de rede durante sessão PTT ou SOS.

## Registro de acessos
- [ ] Pedido do app não aparece como autenticação bem-sucedida no AP.
- [ ] Só controlador confiável gera sessão atestada; duplicatas deduplicadas por evento/sessão.
- [ ] Bytes/tempo ausentes permanecem `null`; bytes não negativos e contadores tratados contra reset.
- [ ] Usuário consulta suas sessões; administrador somente de AP próprio; público nenhuma.
- [ ] URL, tráfego, senha e localização GNSS visitante nunca entram no log de auditoria.

## Federação, offline, segurança
- [ ] Revogação durante partição impede novas concessões a partir de réplicas obsoletas.
- [ ] Outbox/inbox em repetição e mensagens fora de ordem não duplicam grants ou sessões.
- [ ] Filtros por tenant, RLS, abuso de rate limit e recuperação de backup funcionam.
- [ ] Android/iOS em bateria baixa reduzem descoberta sem interromper ações essenciais já permitidas.

## Evidências
Registrar modelo, OS, firmware de AP, métodos de autenticação, permissões, sessão de teste anonimizada, resultado observado, logs redigidos, datas e responsáveis.

[Arquitetura](../01-arquitetura/wifi-autodiscover-autoshare.md) · [Integrações](../03-integracoes/wifi-android-ios-roteadores.md) · [Segurança](../04-seguranca/wifi-compartilhamento-privacidade.md).
