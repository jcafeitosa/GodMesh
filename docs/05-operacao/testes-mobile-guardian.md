# Testes integrados — Android/iOS, Guardian, CrowdFind e contingência

> **Matriz de validação, não resultados executados.** Cobrir modelos, versões, regiões, operadoras, estados de bateria, background, SIM/eSIM, permissões e conectividade.

## Descoberta e sensores
- [ ] Android sem permissão de localização não cria rota exata nem coleta dados indevidos.
- [ ] iOS sem API de scan genérico Wi-Fi não anuncia resultados que não possui.
- [ ] RSSI Wi-Fi não é exibido como ângulo confiável 360°.
- [ ] GNSS/GNSS desatualizado, PDR e observações de beacons têm origem, precisão e idade distintas.
- [ ] Android/iOS com UWB ou Wi-Fi Aware ausentes declaram `unsupported` em vez de falhar silenciosamente.

## Guardian e privacidade
- [ ] Sessão exige opt-in e destinatários aceitos; expiração/revogação interrompem consultas futuras.
- [ ] Atraso de check-in gera notificação, mas não dispara SOS automaticamente.
- [ ] Chat/PTT e bridges não recebem coordenadas precisas; Guardian e SOS respeitam escopos separados.
- [ ] Smartphone beacon usa identificador rotativo e não inclui GMC, GMI, telefone ou coordenadas.
- [ ] Sem internet, avistamentos BLE são marcados por frescor e incerteza, nunca como GPS do beacon.
- [ ] Operação background no iOS/Android testada com tela bloqueada, restrições e modo de economia de energia; reportar falhas por modelo.
- [ ] Detector com permissão revogada não consegue continuar publicando/consultando posições.

## Monitoramento móvel e satélite
- [ ] Queda só de 4G, com Wi-Fi ainda online, não é rotulada como ausência total de conexão.
- [ ] Switch entre IPv6, IPv4, Wi-Fi e celular preserva autenticação e dados pendentes sem duplicação.
- [ ] Transportes de satélite só são anunciados quando houver evidência real; `unknown` é diferente de `available`.
- [ ] Satélite ativo mas limitado a SMS/apps específicas não recebe PTT, anexos e GMP inelegível.
- [ ] Terminal Starlink Wi-Fi aparece como Wi-Fi/IP; não confundir com Direct-to-Cell.
- [ ] Sem automação permitida: cartão de orientação aparece, sem alterar ajuste do SO e sem afirmar ação concluída.
- [ ] Android abre tela apropriada de economia de bateria quando suportada; iOS oferece instruções de ajustes.
- [ ] GodMesh pausa ASR remoto/anexos/streaming próprio em link limitado; preserva ações essenciais do próprio app.
- [ ] GodMesh **não** fecha apps terceiros, não força mudança de operadora e não instala plano satelital.
- [ ] Orientações não pedem para desativar localização, discador ou recursos oficiais de emergência.

## SOS e falhas
- [ ] SOS gerado offline registra ACK local e exibe ausência de confirmação externa.
- [ ] Falha de satélite ou falta de elegibilidade não aparece como SOS entregue.
- [ ] Se houver serviços de emergência do aparelho, orientação não os substitui.
- [ ] Recuperação da conexão sincroniza eventos por prioridade sem duplicação; não expõe localização precisa em chat público.
- [ ] Testes reais de rádio/satélite dependem de operadora, região e autorização; não simular suporte inexistente.

## Evidência de teste
Registrar `device_model`, `os_version`, `carrier`, `network_capability`, `permissions`, `background_state`, `sos_test_mode`, `expected`, `actual`, métricas, logs redigidos, data, responsável e resultado. Redigir registros para evitar vazamento de localização e identificadores permanentes.
