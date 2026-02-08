# Vida Artificial Conversacional (PT-BR) — v0.3

Agora a Aurora está mais **viva**, **curiosa** e com aprendizado mais eficiente.

## O que foi refinado

- **Curiosidade ativa**: quando encontra tema desconhecido, a Aurora pede para você ensinar no formato `chave = valor`.
- **Aprendizado mais eficiente**: usa termos relevantes (sem stopwords), coocorrência de conceitos e recuperação híbrida (similaridade lexical + ligação de tópicos).
- **Memória viva e persistente**: fatos, episódios, reflexões e lacunas de conhecimento são salvos em `./.aurora_memoria.json`.
- **Consolidação periódica**: a cada 3 turnos, reforça conceitos recorrentes e registra reflexões internas.
- **Estilo conversacional mais natural**: tom emocional muda com o contexto e com o estado interno.
- **Modo seguro**: por padrão, bloqueia conteúdo ofensivo e redireciona para segurança defensiva.

## Comandos

- `/ajuda`
- `/status`
- `/salvar`
- `/recarregar`
- `/modo-seguro on|off`
- `/fato <chave>`
- `/topicos`
- `/tutorial`

## Passo a passo (do zero)

1. Instale o Rust com rustup: <https://rustup.rs>
2. Abra o terminal e entre na pasta do projeto:
   ```bash
   cd /workspace/AI
   ```
3. Formate o código:
   ```bash
   cargo fmt
   ```
4. Rode os testes:
   ```bash
   cargo test
   ```
5. Execute o agente:
   ```bash
   cargo run
   ```
6. Ensine fatos durante a conversa:
   - `capital do brasil = brasilia`
   - `rust é uma linguagem de sistemas`
7. Faça consulta de memória:
   - `lembra de capital do brasil`
   - `/fato capital do brasil`
8. Veja o estado geral:
   - `/status`
   - `o que você sabe`
9. Encerrar sessão:
   - `sair`

> O aprendizado é salvo em `./.aurora_memoria.json`, então a Aurora continua evoluindo entre execuções.

## Observação

Este projeto simula cognição e emoção de forma explícita/auditável (não consciência fenomenal real).
