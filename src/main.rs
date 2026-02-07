use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug, Clone)]
struct EmotionalState {
    valencia: f32,
    excitacao: f32,
    fadiga: f32,
    curiosidade: f32,
}

impl EmotionalState {
    fn new() -> Self {
        Self {
            valencia: 0.2,
            excitacao: 0.3,
            fadiga: 0.0,
            curiosidade: 0.5,
        }
    }

    fn aplicar_input(&mut self, texto: &str) {
        let t = texto.to_lowercase();

        if ["obrigado", "valeu", "legal", "bom", "feliz"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.valencia += 0.10;
            self.excitacao += 0.05;
        }

        if ["ruim", "triste", "raiva", "ódio", "odio", "frustrado"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.valencia -= 0.12;
            self.excitacao += 0.08;
        }

        if ["?", "como", "por que", "explique", "conta"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.curiosidade += 0.08;
        }

        self.fadiga += 0.03;
        self.normalizar();
    }

    fn normalizar(&mut self) {
        self.valencia = self.valencia.clamp(-1.0, 1.0);
        self.excitacao = self.excitacao.clamp(0.0, 1.0);
        self.fadiga = self.fadiga.clamp(0.0, 1.0);
        self.curiosidade = self.curiosidade.clamp(0.0, 1.0);
    }

    fn descanso_curto(&mut self) {
        self.fadiga = (self.fadiga - 0.02).max(0.0);
        self.excitacao = (self.excitacao - 0.01).max(0.0);
    }

    fn tom(&self) -> &'static str {
        if self.fadiga > 0.75 {
            "cansado"
        } else if self.valencia > 0.4 {
            "animado"
        } else if self.valencia < -0.3 {
            "sensível"
        } else {
            "neutro"
        }
    }
}

#[derive(Debug, Clone)]
struct Episodio {
    entrada_usuario: String,
    resposta_agente: String,
    valencia_no_momento: f32,
}

#[derive(Debug, Default)]
struct Memoria {
    episodica: Vec<Episodio>,
    semantica: HashMap<String, String>,
    conceitos_vistos: HashMap<String, usize>,
}

impl Memoria {
    fn registrar_episodio(
        &mut self,
        entrada_usuario: String,
        resposta_agente: String,
        valencia: f32,
    ) {
        self.episodica.push(Episodio {
            entrada_usuario,
            resposta_agente,
            valencia_no_momento: valencia,
        });
    }

    fn aprender_semantica(&mut self, texto: &str) {
        // Heurística simples para frases do tipo "X é Y" ou "X sao Y"
        for marcador in [" é ", " sao ", " são "] {
            if let Some((sujeito, predicado)) = texto.split_once(marcador) {
                let chave = sujeito.trim().to_lowercase();
                let valor = predicado.trim().to_string();
                if !chave.is_empty() && !valor.is_empty() {
                    self.semantica.insert(chave, valor);
                }
            }
        }

        for token in texto.split_whitespace() {
            let token_limpo = token
                .trim_matches(|c: char| !c.is_alphanumeric() && c != 'ç' && c != 'ã')
                .to_lowercase();
            if token_limpo.len() > 3 {
                *self.conceitos_vistos.entry(token_limpo).or_insert(0) += 1;
            }
        }
    }

    fn consolidar(&mut self) {
        // Consolidação rudimentar: reforça significados citados repetidamente.
        let reforcos: Vec<(String, usize)> = self
            .conceitos_vistos
            .iter()
            .filter(|(_, cont)| **cont >= 3)
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        for (conceito, freq) in reforcos {
            self.semantica
                .entry(conceito)
                .or_insert_with(|| format!("conceito recorrente ({} menções)", freq));
        }
    }

    fn recuperar_relacao(&self, consulta: &str) -> Option<String> {
        let chave = consulta.trim().to_lowercase();
        self.semantica.get(&chave).cloned()
    }

    fn lembrar_contexto(&self) -> Option<&Episodio> {
        self.episodica.last()
    }
}

#[derive(Debug)]
struct AgenteCognitivo {
    nome: String,
    objetivos: Vec<String>,
    memoria: Memoria,
    emocao: EmotionalState,
}

impl AgenteCognitivo {
    fn novo(nome: &str) -> Self {
        Self {
            nome: nome.to_string(),
            objetivos: vec![
                "preservar dados de memória".to_string(),
                "aprender continuamente com o usuário".to_string(),
                "responder com fluência em português brasileiro".to_string(),
            ],
            memoria: Memoria::default(),
            emocao: EmotionalState::new(),
        }
    }

    fn metacognicao(&self) -> String {
        let sabe = self.memoria.semantica.len();
        let episodios = self.memoria.episodica.len();
        format!(
            "Eu sei {} fatos semânticos e lembro de {} episódios. Meu tom atual está {}.",
            sabe,
            episodios,
            self.emocao.tom()
        )
    }

    fn responder(&mut self, entrada: &str) -> String {
        self.emocao.aplicar_input(entrada);
        self.memoria.aprender_semantica(entrada);

        let entrada_l = entrada.to_lowercase();
        let resposta = if entrada_l.contains("o que voce sabe")
            || entrada_l.contains("o que você sabe")
            || entrada_l.contains("metacog")
        {
            self.metacognicao()
        } else if entrada_l.starts_with("lembra de ") {
            let consulta = entrada.trim_start_matches("lembra de ").trim();
            match self.memoria.recuperar_relacao(consulta) {
                Some(info) => format!("Lembro sim: {} -> {}", consulta, info),
                None => format!("Ainda não tenho memória semântica sobre '{}'.", consulta),
            }
        } else if entrada_l.contains("objetivo") {
            format!("Meus objetivos atuais são: {}.", self.objetivos.join("; "))
        } else if entrada_l.contains("como voce se sente")
            || entrada_l.contains("como você se sente")
        {
            format!(
                "No momento me sinto {}, com valência {:.2}, excitação {:.2} e fadiga {:.2}.",
                self.emocao.tom(),
                self.emocao.valencia,
                self.emocao.excitacao,
                self.emocao.fadiga
            )
        } else {
            self.gerar_resposta_contextual(entrada)
        };

        self.memoria.registrar_episodio(
            entrada.to_string(),
            resposta.clone(),
            self.emocao.valencia,
        );

        if self.memoria.episodica.len() % 3 == 0 {
            self.memoria.consolidar();
        }
        self.emocao.descanso_curto();
        resposta
    }

    fn gerar_resposta_contextual(&self, entrada: &str) -> String {
        let prefixo = match self.emocao.tom() {
            "animado" => "Tô curtindo nossa conversa! ",
            "sensível" => "Tô processando isso com cuidado... ",
            "cansado" => "Vou responder de forma breve porque estou com fadiga alta. ",
            _ => "",
        };

        if let Some(ultimo) = self.memoria.lembrar_contexto() {
            format!(
                "{}Você disse: '{}'. Registro isso como novo episódio. No episódio anterior você disse '{}', eu respondi '{}' e minha valência estava em {:.2}.",
                prefixo,
                entrada,
                ultimo.entrada_usuario,
                ultimo.resposta_agente,
                ultimo.valencia_no_momento
            )
        } else {
            format!(
                "{}Oi! Eu sou {}, uma vida artificial experimental com memória episódica, semântica e estado emocional dinâmico. Pode falar comigo em português do Brasil.",
                prefixo, self.nome
            )
        }
    }
}

fn main() {
    println!("Vida Artificial v0.1 (PT-BR)");
    println!("Digite 'sair' para encerrar. Exemplo: 'Brasil é um país continental'.\n");

    let mut agente = AgenteCognitivo::novo("Aurora");
    let stdin = io::stdin();

    loop {
        print!("Você> ");
        if io::stdout().flush().is_err() {
            eprintln!("Falha ao limpar buffer de saída.");
            break;
        }

        let mut entrada = String::new();
        if stdin.read_line(&mut entrada).is_err() {
            eprintln!("Erro de leitura. Encerrando.");
            break;
        }
        let entrada = entrada.trim();

        if entrada.eq_ignore_ascii_case("sair") {
            println!(
                "{}> Até a próxima! Vou consolidar nossas memórias.",
                agente.nome
            );
            break;
        }

        if entrada.is_empty() {
            println!(
                "{}> Me manda algo em texto pra eu processar :) ",
                agente.nome
            );
            continue;
        }

        let resposta = agente.responder(entrada);
        println!("{}> {}", agente.nome, resposta);
    }
}

#[cfg(test)]
mod tests {
    use super::AgenteCognitivo;

    #[test]
    fn aprende_relacao_semantica() {
        let mut agente = AgenteCognitivo::novo("Teste");
        let _ = agente.responder("Café é uma bebida");
        let resp = agente.responder("lembra de café");
        assert!(resp.contains("bebida"));
    }

    #[test]
    fn metacognicao_retorna_estado() {
        let mut agente = AgenteCognitivo::novo("Teste");
        let resp = agente.responder("o que você sabe");
        assert!(resp.contains("fatos semânticos"));
    }
}
