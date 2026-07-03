# tk — proxy MCP de medição de tokens (v1)

**Data:** 2026-07-03
**Status:** Aprovado

## Problema

Respostas de tools MCP entram inteiras no contexto do modelo e ninguém as mede nem comprime. O rtk (github.com/rtk-ai/rtk) resolve isso para comandos Bash no Claude Code via hook `PreToolUse`, mas o Claude Desktop não tem shell nem hooks — e o próprio rtk não intercepta tráfego MCP. Como o protocolo MCP é o mesmo no Claude Desktop e no Claude Code, um proxy MCP cobre os dois de uma vez.

## Objetivo

Um binário Rust (`tk`) que envelopa qualquer servidor MCP stdio, mede o consumo estimado de tokens por servidor e por tool, e produz um relatório (`tk stats`). A v1 **só mede** — compressão fica para a v2, guiada pelos dados coletados. Objetivo duplo: otimizar o uso pessoal do autor e evoluir para ferramenta publicável.

## Decisões de design

| Decisão | Escolha | Motivo |
|---|---|---|
| Alvo | Servidores MCP stdio locais | Único ponto interceptável localmente; tools nativas do Desktop ficam fora do alcance de qualquer solução local |
| Linguagem | Rust | Binário único, zero dependências para o usuário final, latência mínima; cargo 1.96 disponível na máquina (Node local é v14, antigo demais para o ecossistema MCP) |
| Escopo v1 | Medição, sem compressão | Menos risco de quebrar servidores; dados reais orientam a v2 |
| Persistência | JSONL diário, sem banco | Simplicidade; agregação na leitura |
| Privacidade | Nunca gravar payloads, só tamanhos | Pré-requisito para ferramenta distribuível |

## Arquitetura

Um único binário com dois modos:

### Modo proxy

`tk -- <comando do servidor> [args...]`

1. Spawna o servidor MCP real como processo filho.
2. Encaminha stdin → stdin do filho e stdout do filho → stdout, **byte a byte, sem modificar frames**. stderr do filho passa direto.
3. O transporte stdio do MCP é JSON-RPC delimitado por newline. Cada linha é parseada apenas para extrair o mínimo necessário à medição; linha que não parsear é repassada intacta.
4. Flag `--name <alias>` identifica o servidor nos logs (injetada pelo `tk init`; fallback: basename do comando filho).

### Modo CLI

- `tk stats [--since <período>]` — relatório agregado: total por servidor, por tool, top ofensores. Período no formato `7d`/`24h` (padrão: `7d`).
- `tk init` — localiza `claude_desktop_config.json` (Claude Desktop) e `.mcp.json`/config do Claude Code, reescreve cada entrada de servidor para rodar via `tk`, criando backup do arquivo original.
- `tk restore` — desfaz o `tk init` a partir do backup.

## Medição

- Estimativa de tokens por heurística de ~4 caracteres/token, sempre apresentada como estimada (o tokenizer do Claude não é público; para comparação relativa a heurística basta).
- O proxy mantém em memória um mapa `id da requisição JSON-RPC → nome da tool` (respostas só carregam o `id`, não o método), para atribuir cada resposta à tool correta.
- Evento registrado: timestamp, alias do servidor, nome da tool, tokens estimados da requisição (params) e da resposta (result).

## Persistência e relatório

- Eventos em JSONL, um arquivo por dia, em `%LOCALAPPDATA%\token-killer\logs\` no Windows e `~/.local/share/token-killer/logs/` em Linux/macOS (via crate `dirs`).
- `tk stats` lê e agrega os JSONL do período pedido. Sem estado além dos arquivos de log.

## Tratamento de erros

- Processo filho morre → `tk` propaga o exit code e encerra.
- Falha de logging (disco cheio, permissão) → degrada silenciosamente para proxy puro; nunca interrompe o cano.
- v1 não tem timeout, retry nem modificação de frames. Invariante: o pior caso do `tk` é "não mediu"; nunca "quebrou o servidor".

## Testes

- **Unitários:** parser de frames JSON-RPC, casamento id→tool, estimador de tokens.
- **Integração:** servidor MCP fake (binário de teste no repo) exercitando `initialize` → `tools/list` → `tools/call`; asserções de que (a) os bytes de saída são idênticos aos de entrada e (b) os stats registrados batem com o tráfego.
- **Manual:** envelopar um servidor real no Claude Desktop do autor e confirmar funcionamento normal + logs coerentes.

## Fora do escopo da v1

- Compressão de respostas (v2, orientada pelos dados de medição).
- Servidores MCP remotos (HTTP/SSE).
- Tools nativas do Claude Desktop (web search, arquivos do Cowork) — inalcançáveis por solução local.
- Dashboard visual; o relatório é texto no terminal.
