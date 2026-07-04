# tk — token killer para MCP

Proxy transparente que envelopa qualquer servidor MCP stdio e mede o
consumo estimado de tokens por servidor e por tool. Funciona com o
Claude Desktop e o Claude Code (qualquer cliente MCP stdio).

Inspirado no [rtk](https://github.com/rtk-ai/rtk), que comprime output
de comandos Bash no Claude Code — o tk cobre o que o rtk não vê:
respostas de tools MCP. A v1 **só mede** (nunca modifica o tráfego);
compressão vem na v2, guiada pelos dados.

## Instalação

    cargo install --path .

## Uso

Envelopar manualmente um servidor na config (`claude_desktop_config.json`
ou `.mcp.json`):

    { "command": "tk", "args": ["--name", "fetch", "--", "uvx", "mcp-server-fetch"] }

Ou automaticamente (cria backup `.tk-backup` ao lado da config):

    tk init             # config do Claude Desktop deste SO
    tk init --file caminho\para\.mcp.json
    tk restore          # desfaz

Relatório:

    tk stats            # últimos 7 dias
    tk stats --since 24h

## Privacidade

O tk grava apenas tamanhos e contagens (JSONL diário em
`%LOCALAPPDATA%\token-killer\logs`), nunca o conteúdo dos payloads.

## Garantia de transparência

Toda linha é repassada byte a byte, mesmo que não parseie. Falha de
medição nunca interrompe o servidor: o pior caso é "não mediu".
Os valores de tokens são estimados (~4 chars/token).
