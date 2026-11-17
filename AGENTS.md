# GodMesh — instruções do repositório

## Entrada e fontes de autoridade
- Responda em português brasileiro, salvo pedido explícito em outro idioma.
- Antes de qualquer tarefa, leia este arquivo e a documentação pública pertinente em `docs/`.
- `docs/README.md` explica a estrutura e as regras de precedência; `docs/01-arquitetura/arquitetura-referencia.md` e `docs/06-planejamento/decisoes.md` são as fontes normativas de arquitetura e decisões quando disponíveis.
- A documentação descreve o sistema planejado. Não a trate como prova de implementação, disponibilidade, compatibilidade validada ou teste executado.
- Arquivos locais de perfis/skills de agentes não são parte deste repositório; não dependa de `.agents/`, de caminhos absolutos de máquinas, nem de configuração não versionada.

## Escopo, planejamento e implementação
- Preserve o escopo integral GodMesh; fases de entrega não reduzem o produto final.
- Antes de mudanças substanciais, registre contexto, objetivo, limites, dependências, critérios de aceite e estratégia de verificação. Divida o trabalho em no máximo seis subtarefas claras e rastreáveis; use fluxo Mermaid quando contribuir para a decisão.
- Não implemente comportamento sem SPEC completa, rastreável e aprovada, nem antes dos gates de planejamento definidos pela documentação vigente.
- Quando a implementação estiver autorizada, aplique SDD e TDD: teste que falha pelo comportamento esperado → implementação mínima → teste passando → refatoração.
- Diferencie explicitamente proposta, decisão aprovada, implementação e validação. Parâmetros desconhecidos permanecem desconhecidos; não fabrique medições, critérios, fontes ou resultados.

## Qualidade e segurança
- Mantenha rastreabilidade requisito → decisão → SPEC → código → teste → evidência. Planeje validações positivas, negativas, de borda e de recuperação proporcionais ao risco; declare o que não foi executado.
- Para dependências novas ou alteradas, consulte documentação oficial e avalie compatibilidade, manutenção, segurança, licença e custo. Registre versão resolvida, fonte e data; não faça upgrades ou migrações destrutivas silenciosamente.
- Use validação nas fronteiras, autorização explícita, isolamento por tenant, minimização de dados, retenção e revogação conforme contratos aprovados. Proteja segredos em código, logs, fixtures e documentação.
- Especifique erros, idempotência, timeouts, cancelamento, retry limitado e comportamento offline quando aplicáveis. Não anuncie garantias de entrega, emergências ou disponibilidade que não possam ser demonstradas.
- Preserve compatibilidade de APIs, eventos e formatos persistidos; mudanças incompatíveis exigem versionamento e plano de migração.
- Remova arquivos/código somente depois de verificar referências, uso em geração/distribuição e documentação. Não remova artefatos apenas por parecerem antigos.
- Solicite revisão independente para alterações de código. Não enfraqueça testes, verificações ou controles para obter um resultado verde.

## Git e colaboração
- Antes de stage/commit/push, confira estado, branch, arquivos não rastreados, remotos, diff e possíveis segredos. Use commits pequenos e mensagens claras.
- Não sobrescreva, remova ou troque remotos sem autorização explícita. Criar/alterar remotos, publicar código, abrir PR, configurar CI/CD ou operar serviços exige autorização para a ação e o destino.
- Nunca publique credenciais, configuração pessoal, exportações locais de agentes, caminhos privados ou histórico interno que não tenha sido aprovado para publicação.
- Não presuma licença, stack, política de branch, CI ou infraestrutura; siga decisões verificadas e solicite decisão quando necessária.

## Conclusão
- Ao concluir, liste arquivos alterados, verificações realmente executadas e seus resultados, limitações e bloqueios restantes.
- Não declare planejamento, SPECs, implementação ou validação completos sem evidência correspondente.
