use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MEMORIA_ARQUIVO_PADRAO: &str = "./.aurora_memoria.json";
const MAX_REFLEXOES: usize = 1200;
const MAX_FILA_LACUNAS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EstadoEmocional {
    valencia: f32,
    excitacao: f32,
    fadiga: f32,
    curiosidade: f32,
}

impl EstadoEmocional {
    fn novo() -> Self {
        Self {
            valencia: 0.2,
            excitacao: 0.35,
            fadiga: 0.0,
            curiosidade: 0.6,
        }
    }

    fn aplicar_input(&mut self, texto: &str) {
        let t = texto.to_lowercase();

        if ["obrigado", "valeu", "massa", "excelente", "legal"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.valencia += 0.12;
            self.excitacao += 0.05;
        }

        if ["triste", "raiva", "frustrado", "decepcionado", "ruim"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.valencia -= 0.15;
            self.excitacao += 0.08;
        }

        if ["?", "por que", "como", "detalha", "explica"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.curiosidade += 0.12;
        }

        self.fadiga += 0.02;
        self.normalizar();
    }

    fn descanso_curto(&mut self) {
        self.fadiga = (self.fadiga - 0.012).max(0.0);
        self.excitacao = (self.excitacao - 0.01).max(0.0);
        self.curiosidade = (self.curiosidade - 0.006).max(0.0);
    }

    fn normalizar(&mut self) {
        self.valencia = self.valencia.clamp(-1.0, 1.0);
        self.excitacao = self.excitacao.clamp(0.0, 1.0);
        self.fadiga = self.fadiga.clamp(0.0, 1.0);
        self.curiosidade = self.curiosidade.clamp(0.0, 1.0);
    }

    fn tom(&self) -> &'static str {
        if self.fadiga > 0.78 {
            "cansado"
        } else if self.valencia > 0.45 {
            "animado"
        } else if self.valencia < -0.35 {
            "sensível"
        } else {
            "neutro"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Episodio {
    turno: usize,
    entrada_usuario: String,
    resposta_agente: String,
    valencia_no_momento: f32,
    topicos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FatoSemantico {
    valor: String,
    confianca: f32,
    atualizacoes: usize,
    fonte: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LacunaConhecimento {
    topico: String,
    tentativas: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Memoria {
    episodica: Vec<Episodio>,
    semantica: HashMap<String, FatoSemantico>,
    conceitos_vistos: HashMap<String, usize>,
    coocorrencia: HashMap<String, HashMap<String, usize>>,
    reflexoes: Vec<String>,
    lacunas: VecDeque<LacunaConhecimento>,
}

impl Memoria {
    fn aprender_semantica(&mut self, texto: &str) {
        let limpo = texto.trim();
        for marcador in [" é ", " sao ", " são ", " significa ", " quer dizer "] {
            if let Some((sujeito, predicado)) = limpo.split_once(marcador) {
                self.registrar_fato(sujeito, predicado, "usuario");
            }
        }

        if let Some((chave, valor)) = limpo.split_once('=') {
            self.registrar_fato(chave, valor, "usuario");
        }

        let tokens = termos_relevantes(limpo);
        for token in &tokens {
            *self.conceitos_vistos.entry(token.clone()).or_insert(0) += 1;
        }

        for i in 0..tokens.len() {
            for j in (i + 1)..tokens.len() {
                let a = tokens[i].clone();
                let b = tokens[j].clone();
                self.coocorrencia
                    .entry(a.clone())
                    .or_default()
                    .entry(b.clone())
                    .and_modify(|v| *v += 1)
                    .or_insert(1);
                self.coocorrencia
                    .entry(b)
                    .or_default()
                    .entry(a)
                    .and_modify(|v| *v += 1)
                    .or_insert(1);
            }
        }
    }

    fn registrar_fato(&mut self, chave: &str, valor: &str, fonte: &str) {
        let key = normalizar_chave(chave);
        let value = valor.trim().to_string();
        if key.is_empty() || value.is_empty() {
            return;
        }

        let item = self.semantica.entry(key).or_insert(FatoSemantico {
            valor: value.clone(),
            confianca: 0.45,
            atualizacoes: 0,
            fonte: fonte.to_string(),
        });

        if item.valor != value {
            item.valor = value;
            item.confianca = (item.confianca * 0.9).max(0.35);
        }
        item.atualizacoes += 1;
        item.confianca = (item.confianca + 0.1).min(0.98);
        item.fonte = fonte.to_string();
    }

    fn registrar_lacuna(&mut self, topico: &str) {
        let t = normalizar_chave(topico);
        if t.len() < 3 {
            return;
        }
        if let Some(pos) = self.lacunas.iter().position(|l| l.topico == t) {
            if let Some(item) = self.lacunas.get_mut(pos) {
                item.tentativas += 1;
            }
            return;
        }
        if self.lacunas.len() >= MAX_FILA_LACUNAS {
            let _ = self.lacunas.pop_front();
        }
        self.lacunas.push_back(LacunaConhecimento {
            topico: t,
            tentativas: 1,
        });
    }

    fn próxima_lacuna(&mut self) -> Option<LacunaConhecimento> {
        self.lacunas.pop_front()
    }

    fn registrar_episodio(&mut self, episodio: Episodio) {
        self.episodica.push(episodio);
    }

    fn consolidar(&mut self, turno: usize) {
        for (conceito, freq) in self.conceitos_vistos.clone() {
            if freq >= 4 {
                self.semantica
                    .entry(conceito.clone())
                    .or_insert(FatoSemantico {
                        valor: format!("conceito recorrente ({freq} menções)"),
                        confianca: 0.40,
                        atualizacoes: 1,
                        fonte: "consolidacao".to_string(),
                    });
            }
        }

        let topicos_densos = self
            .coocorrencia
            .iter()
            .filter_map(|(k, viz)| viz.values().sum::<usize>().ge(&6).then_some(k.clone()))
            .take(4)
            .collect::<Vec<_>>();

        self.reflexoes.push(format!(
            "Turno {turno}: consolidei {} fatos e notei densidade nos tópicos [{}].",
            self.semantica.len(),
            topicos_densos.join(", ")
        ));

        if self.reflexoes.len() > MAX_REFLEXOES {
            let keep_from = self.reflexoes.len() - MAX_REFLEXOES;
            self.reflexoes.drain(0..keep_from);
        }
    }

    fn recuperar_fato(&self, consulta: &str) -> Option<(String, FatoSemantico)> {
        let q = normalizar_chave(consulta);
        if let Some(v) = self.semantica.get(&q) {
            return Some((q, v.clone()));
        }

        self.semantica
            .iter()
            .map(|(k, v)| {
                let lexical = similaridade_textual(k, &q);
                let topologica = self
                    .coocorrencia
                    .get(&q)
                    .and_then(|m| m.get(k))
                    .copied()
                    .unwrap_or(0) as f32
                    / 10.0;
                (k, v, lexical + topologica.min(0.4))
            })
            .filter(|(_, _, score)| *score > 0.26)
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
            .map(|(k, v, _)| (k.clone(), v.clone()))
    }

    fn episodios_relevantes(&self, consulta: &str, limite: usize) -> Vec<&Episodio> {
        let mut ranqueados = self
            .episodica
            .iter()
            .map(|ep| (ep, similaridade_textual(consulta, &ep.entrada_usuario)))
            .filter(|(_, score)| *score > 0.2)
            .collect::<Vec<_>>();

        ranqueados.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        ranqueados
            .into_iter()
            .take(limite)
            .map(|(ep, _)| ep)
            .collect()
    }

    fn resumo_status(&self) -> String {
        format!(
            "{} fatos, {} episódios, {} lacunas em estudo, {} reflexões.",
            self.semantica.len(),
            self.episodica.len(),
            self.lacunas.len(),
            self.reflexoes.len()
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Persistencia {
    nome: String,
    objetivos: Vec<String>,
    memoria: Memoria,
    emocao: EstadoEmocional,
    turno: usize,
}

#[derive(Debug)]
struct AgenteCognitivo {
    nome: String,
    objetivos: Vec<String>,
    memoria: Memoria,
    emocao: EstadoEmocional,
    turno: usize,
    arquivo_memoria: PathBuf,
    modo_seguro: bool,
}

impl AgenteCognitivo {
    fn novo(nome: &str, arquivo_memoria: impl AsRef<Path>) -> Self {
        Self {
            nome: nome.to_string(),
            objetivos: vec![
                "preservar dados de memória".to_string(),
                "aprender continuamente com o usuário".to_string(),
                "manter curiosidade ativa e explicar em passos".to_string(),
                "responder com naturalidade em português brasileiro".to_string(),
            ],
            memoria: Memoria::default(),
            emocao: EstadoEmocional::novo(),
            turno: 0,
            arquivo_memoria: arquivo_memoria.as_ref().to_path_buf(),
            modo_seguro: true,
        }
    }

    fn carregar_ou_novo(nome: &str, arquivo_memoria: impl AsRef<Path>) -> Self {
        let path = arquivo_memoria.as_ref();
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(persistido) = serde_json::from_str::<Persistencia>(&raw)
        {
            return Self {
                nome: persistido.nome,
                objetivos: persistido.objetivos,
                memoria: persistido.memoria,
                emocao: persistido.emocao,
                turno: persistido.turno,
                arquivo_memoria: path.to_path_buf(),
                modo_seguro: true,
            };
        }
        Self::novo(nome, path)
    }

    fn salvar(&self) -> Result<(), String> {
        let persistencia = Persistencia {
            nome: self.nome.clone(),
            objetivos: self.objetivos.clone(),
            memoria: self.memoria.clone(),
            emocao: self.emocao.clone(),
            turno: self.turno,
        };

        let serializado = serde_json::to_string_pretty(&persistencia)
            .map_err(|e| format!("erro ao serializar memória: {e}"))?;
        fs::write(&self.arquivo_memoria, serializado)
            .map_err(|e| format!("erro ao gravar arquivo de memória: {e}"))
    }

    fn metacognicao(&self) -> String {
        format!(
            "Eu sei {} fatos, lembro de {} episódios e tenho {} lacunas para investigar. Tom: {}. Curiosidade {:.2}.",
            self.memoria.semantica.len(),
            self.memoria.episodica.len(),
            self.memoria.lacunas.len(),
            self.emocao.tom(),
            self.emocao.curiosidade
        )
    }

    fn violacao_seguranca(&self, entrada: &str) -> bool {
        if !self.modo_seguro {
            return false;
        }
        let t = entrada.to_lowercase();
        [
            "como invadir",
            "fazer malware",
            "phishing",
            "golpe",
            "burlar senha",
            "explorar vulnerabilidade",
        ]
        .iter()
        .any(|k| t.contains(k))
    }

    fn processar_comando(&mut self, entrada: &str) -> Option<String> {
        let cmd = entrada.trim();
        if !cmd.starts_with('/') {
            return None;
        }

        let low = cmd.to_lowercase();
        let resp = if low == "/ajuda" {
            "Comandos: /ajuda, /status, /salvar, /recarregar, /modo-seguro on|off, /fato <chave>, /topicos, /tutorial".to_string()
        } else if low == "/status" {
            format!("Status => {}", self.memoria.resumo_status())
        } else if low == "/salvar" {
            match self.salvar() {
                Ok(_) => format!("Memória salva em {}", self.arquivo_memoria.display()),
                Err(e) => format!("Falha ao salvar: {e}"),
            }
        } else if low == "/recarregar" {
            let novo = Self::carregar_ou_novo(&self.nome, &self.arquivo_memoria);
            *self = novo;
            "Memória recarregada do disco.".to_string()
        } else if low == "/topicos" {
            let mut top: Vec<_> = self.memoria.conceitos_vistos.iter().collect();
            top.sort_by(|a, b| b.1.cmp(a.1));
            let msg = top
                .into_iter()
                .take(10)
                .map(|(k, v)| format!("{k}({v})"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Tópicos mais frequentes: {msg}")
        } else if low.starts_with("/modo-seguro ") {
            if low.ends_with("on") {
                self.modo_seguro = true;
                "Modo seguro ativado.".to_string()
            } else if low.ends_with("off") {
                self.modo_seguro = false;
                "Modo seguro desativado.".to_string()
            } else {
                "Uso: /modo-seguro on|off".to_string()
            }
        } else if low.starts_with("/fato ") {
            let consulta = cmd.trim_start_matches("/fato ").trim();
            match self.memoria.recuperar_fato(consulta) {
                Some((chave, fato)) => format!(
                    "{} => {} (confiança {:.2}, atualizações {}, fonte: {})",
                    chave, fato.valor, fato.confianca, fato.atualizacoes, fato.fonte
                ),
                None => "Ainda não sei esse fato. Me ensine com 'X = Y'.".to_string(),
            }
        } else if low == "/tutorial" {
            tutorial_instalacao().to_string()
        } else {
            "Comando desconhecido. Use /ajuda.".to_string()
        };

        Some(resp)
    }

    fn responder(&mut self, entrada: &str) -> String {
        if let Some(comando) = self.processar_comando(entrada) {
            return comando;
        }

        self.turno += 1;
        self.emocao.aplicar_input(entrada);

        if self.violacao_seguranca(entrada) {
            let resp = "Posso ajudar com prevenção e segurança digital, mas não com instruções ofensivas. Posso te guiar em defesa e boas práticas.".to_string();
            self.registrar_e_persistir(entrada, &resp);
            return resp;
        }

        self.memoria.aprender_semantica(entrada);
        let l = entrada.to_lowercase();

        let resposta = if l.contains("o que voce sabe")
            || l.contains("o que você sabe")
            || l.contains("metacog")
        {
            self.metacognicao()
        } else if l.starts_with("lembra de ") {
            let consulta = entrada.trim_start_matches("lembra de ").trim();
            self.responder_memoria(consulta)
        } else if l.contains("passo a passo")
            || l.contains("do inicio")
            || l.contains("desde o inicio")
        {
            tutorial_instalacao().to_string()
        } else if l.contains("objetivo") {
            format!("Meus objetivos atuais: {}.", self.objetivos.join("; "))
        } else if l.contains("como voce se sente") || l.contains("como você se sente") {
            format!(
                "Agora me sinto {}, com valência {:.2}, excitação {:.2}, fadiga {:.2} e curiosidade {:.2}.",
                self.emocao.tom(),
                self.emocao.valencia,
                self.emocao.excitacao,
                self.emocao.fadiga,
                self.emocao.curiosidade
            )
        } else {
            self.resposta_viva(entrada)
        };

        self.registrar_e_persistir(entrada, &resposta);
        resposta
    }

    fn resposta_viva(&mut self, entrada: &str) -> String {
        let prefixo = match self.emocao.tom() {
            "animado" => "Curti a energia daqui. ",
            "sensível" => "Tô processando com cuidado. ",
            "cansado" => "Vou resumir para manter qualidade. ",
            _ => "",
        };

        let contexto = self
            .memoria
            .episodios_relevantes(entrada, 1)
            .into_iter()
            .next()
            .map(|ep| {
                format!(
                    "Conecta com o turno {} ('{}'). ",
                    ep.turno, ep.entrada_usuario
                )
            })
            .unwrap_or_default();

        let curiosidade = self.pergunta_curiosa(entrada);
        format!(
            "{}{}Entendi: '{}'. {}",
            prefixo, contexto, entrada, curiosidade
        )
    }

    fn pergunta_curiosa(&mut self, entrada: &str) -> String {
        let termos = termos_relevantes(entrada);
        let desconhecidos = termos
            .iter()
            .filter(|t| !self.memoria.semantica.contains_key(*t))
            .cloned()
            .collect::<Vec<_>>();

        if desconhecidos.is_empty() {
            return "Quer que eu aprofunde em algum ponto específico disso?".to_string();
        }

        let topico = desconhecidos[0].clone();
        self.memoria.registrar_lacuna(&topico);
        format!(
            "Minha curiosidade ativou em '{}'. Você pode me ensinar no formato '{} = ...' para eu aprender melhor?",
            topico, topico
        )
    }

    fn registrar_e_persistir(&mut self, entrada: &str, resposta: &str) {
        let topicos = termos_relevantes(entrada).into_iter().take(12).collect();

        self.memoria.registrar_episodio(Episodio {
            turno: self.turno,
            entrada_usuario: entrada.to_string(),
            resposta_agente: resposta.to_string(),
            valencia_no_momento: self.emocao.valencia,
            topicos,
        });

        if self.turno.is_multiple_of(3) {
            self.memoria.consolidar(self.turno);
        }

        if self.turno.is_multiple_of(5)
            && self.emocao.curiosidade > 0.35
            && let Some(l) = self.memoria.próxima_lacuna()
        {
            self.memoria.reflexoes.push(format!(
                "Turno {}: priorizei lacuna '{}' ({} tentativa(s)).",
                self.turno, l.topico, l.tentativas
            ));
        }

        self.emocao.descanso_curto();

        if let Err(e) = self.salvar() {
            eprintln!("[aviso] falha ao salvar memória: {e}");
        }
    }

    fn responder_memoria(&self, consulta: &str) -> String {
        let fato = self.memoria.recuperar_fato(consulta);
        let episodios = self.memoria.episodios_relevantes(consulta, 3);

        let mut blocos = Vec::new();
        if let Some((k, f)) = fato {
            blocos.push(format!(
                "Fato: {} => {} (confiança {:.2}, atualizações {}).",
                k, f.valor, f.confianca, f.atualizacoes
            ));
        }

        if !episodios.is_empty() {
            let e = episodios
                .into_iter()
                .map(|ep| format!("turno {}: '{}'", ep.turno, ep.entrada_usuario))
                .collect::<Vec<_>>()
                .join(" | ");
            blocos.push(format!("Episódios relevantes: {}.", e));
        }

        if blocos.is_empty() {
            "Ainda não tenho uma memória forte disso. Me ensina com 'chave = valor' que eu fixo agora.".to_string()
        } else {
            blocos.join(" ")
        }
    }
}

fn normalizar_chave(texto: &str) -> String {
    texto
        .to_lowercase()
        .trim()
        .trim_matches(|c: char| {
            !c.is_alphanumeric() && c != ' ' && c != 'ç' && c != 'ã' && c != 'é'
        })
        .replace("  ", " ")
}

fn termos_relevantes(texto: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "de", "da", "do", "das", "dos", "a", "o", "e", "um", "uma", "que", "como", "para", "com",
        "sem", "por", "em", "no", "na", "nos", "nas", "eu", "você", "voce", "me", "te",
    ];

    texto
        .split_whitespace()
        .map(normalizar_chave)
        .filter(|t| t.len() > 2 && !STOPWORDS.contains(&t.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn similaridade_textual(a: &str, b: &str) -> f32 {
    let sa: HashSet<String> = termos_relevantes(a).into_iter().collect();
    let sb: HashSet<String> = termos_relevantes(b).into_iter().collect();

    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let i = sa.intersection(&sb).count() as f32;
    let u = sa.union(&sb).count() as f32;
    i / u
}

fn tutorial_instalacao() -> &'static str {
    "PASSO A PASSO (do zero):\n1) Instale Rust: https://rustup.rs\n2) No terminal, entre na pasta do projeto: cd /workspace/AI\n3) Formate o código: cargo fmt\n4) Rode os testes: cargo test\n5) Execute o agente: cargo run\n6) Converse e ensine fatos com: chave = valor\n7) Consulte memória: lembra de <chave> ou /fato <chave>\n8) Veja status: /status\n9) Salve manualmente (opcional): /salvar\n10) Encerrar: sair\nDica: o arquivo ./.aurora_memoria.json guarda seu aprendizado entre sessões."
}

fn main() {
    println!("Vida Artificial v0.3 (PT-BR) — curiosidade ativa + aprendizado eficiente");
    println!("Digite 'sair' para encerrar. Use /ajuda para comandos.\n");

    let mut agente = AgenteCognitivo::carregar_ou_novo("Aurora", MEMORIA_ARQUIVO_PADRAO);
    let stdin = io::stdin();

    loop {
        print!("Você> ");
        if io::stdout().flush().is_err() {
            eprintln!("Falha ao limpar saída.");
            break;
        }

        let mut entrada = String::new();
        if stdin.read_line(&mut entrada).is_err() {
            eprintln!("Erro de leitura. Encerrando.");
            break;
        }

        let entrada = entrada.trim();
        if entrada.eq_ignore_ascii_case("sair") {
            let _ = agente.salvar();
            println!("{}> Até a próxima. Memória salva.", agente.nome);
            break;
        }

        if entrada.is_empty() {
            println!(
                "{}> Manda algo em texto que eu aprendo contigo.",
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
    use super::{
        AgenteCognitivo, MEMORIA_ARQUIVO_PADRAO, similaridade_textual, tutorial_instalacao,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn arquivo_teste() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("tempo inválido")
            .as_nanos();
        format!("/tmp/aurora_teste_{nanos}.json")
    }

    #[test]
    fn aprende_recupera_evolui_fato() {
        let mut agente = AgenteCognitivo::novo("Teste", arquivo_teste());
        let _ = agente.responder("rust = linguagem de sistema");
        let resp = agente.responder("lembra de rust");
        assert!(resp.contains("linguagem de sistema"));
    }

    #[test]
    fn registra_curiosidade_quando_desconhece() {
        let mut agente = AgenteCognitivo::novo("Teste", arquivo_teste());
        let resp = agente.responder("quero falar sobre astrobiologia");
        assert!(resp.contains("curiosidade"));
    }

    #[test]
    fn persistencia_funciona() {
        let arquivo = arquivo_teste();
        {
            let mut a = AgenteCognitivo::novo("Teste", &arquivo);
            let _ = a.responder("minha cor favorita = azul");
            a.salvar().expect("deve salvar");
        }

        let b = AgenteCognitivo::carregar_ou_novo("Teste", &arquivo);
        let resp = b.responder_memoria("minha cor favorita");
        assert!(resp.contains("azul"));
    }

    #[test]
    fn similaridade_funciona() {
        assert!(similaridade_textual("seguranca em rust", "rust para seguranca") > 0.30);
    }

    #[test]
    fn tutorial_existe() {
        let tutorial = tutorial_instalacao();
        assert!(tutorial.contains("cargo run"));
        assert_ne!(MEMORIA_ARQUIVO_PADRAO, "");
    }
}
