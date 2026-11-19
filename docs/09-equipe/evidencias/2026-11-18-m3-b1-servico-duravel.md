# M3-B1 — serviço durável do piso de configuração

A coordenação aprovou a implementação isolada após o [aceite da M3-A](2026-11-18-m3-a-bundle-assinado.md#aceite-do-codec) e o parecer independente favorável à [SPEC M3-B1 v1.2](../../08-specs/federacao/m3-b1-servico-duravel.md). O SHA-256 da SPEC revisada é `7b98b1e76bb6b58718974f00948c33a6cadef1be9b8cad18c886cc182d77d7ea`.

1. Adicionar migração incremental e preflight para validar e reconstruir o estado com a política pinada.
2. Autenticar e ordenar bundles com o codec M3-A, manter replays, conflito e incompatibilidade duráveis e aplicar seleção e resolução sob o lock M1C.
3. Compartilhar um segundo explícito de autorização. Testar ativação, janela e classificação de operações sob o lock até commit.
4. Verificar os resultados P/N/B/F em PostgreSQL descartável, preservar as migrações aplicadas, rodar a suíte e solicitar revisão independente do código.

A [proposta operacional M3-B](../../08-specs/federacao/m3-b-bundle-operacional.md) documenta a etapa seguinte. O listener, o pull e o uso do piso nos efeitos de QUIC e ingest continuam sujeitos ao aceite da B2. Nenhum teste da B1 foi executado até a aprovação da SPEC.
