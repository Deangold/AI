# Vida Artificial Conversacional (PT-BR) — v0.4

Agora a Aurora está mais inteligente para **aprender o que você ensina**, revisar crenças e conversar com mais continuidade.

## Melhorias principais

- Aprendizado explícito com comando: `/ensinar chave = valor`.
- Memória semântica com **revisões de crença** (quando um fato muda, o histórico é registrado).
- Inferência simples em cadeia (ex.: `A é B`, `B é C` ⇒ relação inferida entre `A` e `C`).
- Perfil de usuário em memória (preferências, objetivo atual e estilo de resposta curto/detalhado).
- Curiosidade ativa: detecta lacunas e pede ensino no formato certo.
- Recuperação mais forte: similaridade textual + coocorrência de conceitos.
- Persistência total em `./.aurora_memoria.json`.

## Comandos

- `/ajuda`
- `/status`
- `/salvar`
- `/recarregar`
- `/fato <chave>`
- `/ensinar <chave> = <valor>`
- `/topicos`
- `/tutorial`
- `/modo-seguro on|off`

## Passo a passo (desde o início)

1. Instale Rust: <https://rustup.rs>
2. Entre no projeto:
   ```bash
   cd /workspace/AI
   ```
3. Formate:
   ```bash
   cargo fmt
   ```
4. Rode testes:
   ```bash
   cargo test
   ```
5. Execute:
   ```bash
   cargo run
   ```
6. Ensine fatos:
   - `/ensinar capital do brasil = brasilia`
   - `/ensinar rust = linguagem de sistemas`
7. Consulte o que ela aprendeu:
   - `/fato capital do brasil`
   - `lembra de rust`
8. Veja status e evolução:
   - `/status`
   - `o que você sabe`
9. Encerrar:
   - `sair`

> O arquivo `./.aurora_memoria.json` guarda memória entre sessões.
