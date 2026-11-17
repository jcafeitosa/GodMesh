# Regras de uso das redes e integrações externas

## Três domínios separados
1. `internet-only`: PTT nativo GodMesh; Mumble/FRN apenas por bridges expressamente autorizadas e sem RF quando permitido.
2. `amateur-radio`: acesso a redes e repetidoras de radioamador exclusivamente por estações/operadores autorizados segundo leis e políticas dos provedores.
3. `business-radio`: rádios profissionais ligados a canais licenciados e equipamentos autorizados conforme país/organização.

**Nunca** ligar automaticamente o áudio de canais corporativos a frequências de radioamador. Ponte só após verificar direitos e receber consentimento do operador da infraestrutura.

## Requisitos de bridge
- Identificar quem transmite e qual rede recebe o áudio.
- Verificar licenças e políticas de rede; se faltarem, bloquear TX.
- Limitar tempo de transmissão, prevenção de loops, CTCSS/squelch/PTT e observabilidade.
- Exigir autorização do dono do gateway/repetidora e respeitar direitos de terceiros.
- Se ponte duplicar transmissões, impedir usando lease exclusivo e confirmação de controle.
- Sem autorização de transmissão RF, operar apenas em laboratório IP, recepção autorizada ou demonstração sem RF.

## FRN e Mumble
- **Mumble** pode hospedar canais privados/públicos em instância própria.
- **FRN** pode ter servidores abertos e gateways físicos. A disponibilidade pública de uma sala não concede direitos sobre rádio físico nem autorização comercial universal. Verificar termos de cada servidor/operador.
- O protocolo FRN publicado é legado; revisar compatibilidade, segurança e manutenção antes de produção.

## No contexto da Colômbia e internacional
Uso do espectro, equipamentos e radioamadorismo dependem de regulamentação local; confirmar com autoridades nacionais e responsáveis pela estação. Não afirmar conformidade por usar um protocolo aberto ou uma rede na internet. Não pressupor faixas LoRaWAN válidas em todo país sem projeto de radiofrequência.

## Checklist pré-integração
- [ ] Titular do servidor/gateway autorizou o uso?
- [ ] O caso de uso é comercial, doméstico ou de radioamador?
- [ ] A licença e a modalidade de rádio permitem a transmissão?
- [ ] Bridge habilitada somente para grupos permitidos?
- [ ] Há identificação do operador, auditoria e limites de TX?
- [ ] Existe mecanismo de desligamento e controle de incidentes?
- [ ] Foi testado isolamento de canais e anti-loop?
