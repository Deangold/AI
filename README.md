# Vida Artificial Conversacional (PT-BR) — v0.2

Agora o agente ficou **mais inteligente e funcional**, com aprendizado contínuo durante a conversa e memória persistente em disco.

## O que melhorou

- **Memória persistente sem janela curta**: episódios, fatos semânticos e reflexões são salvos automaticamente em `./.aurora_memoria.json`.
- **Aprendizado em tempo real**: aceita padrões como `X é Y`, `X = Y`, `X significa Y` e usa isso para responder depois.
- **Recuperação inteligente**: busca por similaridade textual para lembrar fatos e episódios relevantes, mesmo sem match exato.
- **Reflexão periódica**: consolida memória a cada 3 turnos, reforçando conceitos recorrentes.
- **Modo seguro**: por padrão, bloqueia pedidos ofensivos e redireciona para uso defensivo.
- **Comandos de controle**:
  - `/ajuda`
  - `/status`
  - `/salvar`
  - `/recarregar`
  - `/modo-seguro on|off`
  - `/fato <chave>`
  - `/topicos`

## Como executar

```bash
cargo run
```

## Exemplo rápido

```text
Você> Brasil é um país continental
Aurora> ...

Você> capital do brasil = Brasília
Aurora> ...

Você> lembra de capital do brasil
Aurora> Fato lembrado: capital do brasil => Brasília ...
```

## Observação

Este sistema **simula** cognição/emoção com componentes explícitos e auditáveis. Não afirma consciência fenomenal real.
