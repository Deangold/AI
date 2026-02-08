use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MEMORIA_ARQUIVO_PADRAO: &str = "./.aurora_memoria.json";
const ULTIMOS_EPISODIOS_RELEVANTES: usize = 3;

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
            excitacao: 0.3,
            fadiga: 0.0,
            curiosidade: 0.5,
        }
    }

    fn aplicar_input(&mut self, texto: &str) {
        let t = texto.to_lowercase();

        if ["obrigado", "valeu", "legal", "massa", "bom", "feliz"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.valencia += 0.12;
            self.excitacao += 0.05;
        }

        if [
            "ruim",
            "triste",
            "raiva",
            "ódio",
            "odio",
            "frustrado",
            "decepcionado",
        ]
        .iter()
        .any(|p| t.contains(p))
        {
            self.valencia -= 0.14;
            self.excitacao += 0.10;
        }

        if ["?", "como", "por que", "explique", "conta", "detalha"]
            .iter()
            .any(|p| t.contains(p))
        {
            self.curiosidade += 0.10;
        }

        self.fadiga += 0.025;
        self.normalizar();
    }

    fn descanso_curto(&mut self) {
        self.fadiga = (self.fadiga - 0.015).max(0.0);
        self.excitacao = (self.excitacao - 0.01).max(0.0);
        self.curiosidade = (self.curiosidade - 0.005).max(0.0);
    }

    fn normalizar(&mut self) {
        self.valencia = self.valencia.clamp(-1.0, 1.0);
        self.excitacao = self.excitacao.clamp(0.0, 1.0);
        self.fadiga = self.fadiga.clamp(0.0, 1.0);
        self.curiosidade = self.curiosidade.clamp(0.0, 1.0);
    }

    fn tom(&self) -> &'static str {
        if self.fadiga > 0.80 {
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
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Memoria {
    episodica: Vec<Episodio>,
    semantica: HashMap<String, FatoSemantico>,
    conceitos_vistos: HashMap<String, usize>,
    reflexoes: Vec<String>,
}

impl Memoria {
    fn aprender_semantica(&mut self, texto: &str) {
        let limpo = texto.trim();
        for marcador in [" é ", " sao ", " são ", " significa "] {
            if let Some((sujeito, predicado)) = limpo.split_once(marcador) {
                self.registrar_fato(sujeito.trim(), predicado.trim());
            }
        }

        if let Some((chave, valor)) = limpo.split_once('=') {
            self.registrar_fato(chave.trim(), valor.trim());
        }

        for token in tokenizar(limpo) {
            if token.len() > 3 {
                *self.conceitos_vistos.entry(token).or_insert(0) += 1;
            }
        }
    }

    fn registrar_fato(&mut self, chave: &str, valor: &str) {
        let key = chave.to_lowercase();
        if key.is_empty() || valor.is_empty() {
            return;
        }
        let item = self.semantica.entry(key).or_insert(FatoSemantico {
            valor: valor.to_string(),
            confianca: 0.50,
            atualizacoes: 0,
        });
        item.valor = valor.to_string();
        item.atualizacoes += 1;
        item.confianca = (item.confianca + 0.1).min(0.95);
    }

    fn registrar_episodio(&mut self, episodio: Episodio) {
        self.episodica.push(episodio);
    }

    fn consolidar(&mut self) {
        let recorrentes: Vec<_> = self
            .conceitos_vistos
            .iter()
            .filter(|(_, c)| **c >= 3)
            .map(|(k, c)| (k.clone(), *c))
            .collect();

        for (conceito, freq) in recorrentes {
            self.semantica
                .entry(conceito.clone())
                .or_insert(FatoSemantico {
                    valor: format!("conceito recorrente ({freq} menções)"),
                    confianca: 0.4,
                    atualizacoes: 1,
                });
        }

        if let Some(ultimo) = self.episodica.last() {
            self.reflexoes.push(format!(
                "No turno {}, o usuário trouxe '{}' e isso ajustou meu tom para {:.2} de valência.",
                ultimo.turno, ultimo.entrada_usuario, ultimo.valencia_no_momento
            ));
        }

        if self.reflexoes.len() > 1000 {
            let keep_from = self.reflexoes.len() - 1000;
            self.reflexoes.drain(0..keep_from);
        }
    }

    fn recuperar_fato(&self, consulta: &str) -> Option<(String, FatoSemantico)> {
        let q = consulta.trim().to_lowercase();

        if let Some(v) = self.semantica.get(&q) {
            return Some((q, v.clone()));
        }

        self.semantica
            .iter()
            .map(|(k, v)| (k, v, similaridade_textual(k, &q)))
            .filter(|(_, _, score)| *score > 0.25)
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
            .map(|(k, v, _)| (k.clone(), v.clone()))
    }

    fn episodios_relevantes(&self, consulta: &str, limite: usize) -> Vec<&Episodio> {
        let mut com_score: Vec<(&Episodio, f32)> = self
            .episodica
            .iter()
            .map(|ep| {
                let score = similaridade_textual(consulta, &ep.entrada_usuario);
                (ep, score)
            })
            .filter(|(_, score)| *score > 0.2)
            .collect();

        com_score.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        com_score
            .into_iter()
            .take(limite)
            .map(|(ep, _)| ep)
            .collect()
    }

    fn resumo_status(&self) -> String {
        format!(
            "{} fatos, {} episódios, {} reflexões.",
            self.semantica.len(),
            self.episodica.len(),
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
                "responder com fluência em português brasileiro".to_string(),
                "operar em ambiente seguro".to_string(),
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
            memoria: Memoria {
                episodica: self.memoria.episodica.clone(),
                semantica: self.memoria.semantica.clone(),
                conceitos_vistos: self.memoria.conceitos_vistos.clone(),
                reflexoes: self.memoria.reflexoes.clone(),
            },
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
            "Sei {} fatos semânticos, lembro de {} episódios, e produzi {} reflexões. Meu tom atual: {}. Segurança: {}.",
            self.memoria.semantica.len(),
            self.memoria.episodica.len(),
            self.memoria.reflexoes.len(),
            self.emocao.tom(),
            if self.modo_seguro {
                "ativo"
            } else {
                "desativado"
            }
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
        let resposta = if low == "/ajuda" {
            "Comandos: /ajuda, /status, /salvar, /recarregar, /modo-seguro on|off, /fato chave, /topicos.".to_string()
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
                    "{} => {} (confiança {:.2}, atualizações {})",
                    chave, fato.valor, fato.confianca, fato.atualizacoes
                ),
                None => "Nenhum fato encontrado.".to_string(),
            }
        } else if low == "/topicos" {
            let mut topicos: Vec<_> = self.memoria.conceitos_vistos.iter().collect();
            topicos.sort_by(|a, b| b.1.cmp(a.1));
            let texto = topicos
                .into_iter()
                .take(8)
                .map(|(k, v)| format!("{k}({v})"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Tópicos mais vistos: {}", texto)
        } else {
            "Comando desconhecido. Use /ajuda.".to_string()
        };

        Some(resposta)
    }

    fn responder(&mut self, entrada: &str) -> String {
        if let Some(cmd_resp) = self.processar_comando(entrada) {
            return cmd_resp;
        }

        self.turno += 1;
        self.emocao.aplicar_input(entrada);

        if self.violacao_seguranca(entrada) {
            let resposta = "Posso ajudar com prevenção e segurança digital, mas não com instruções ofensivas. Se quiser, te mostro como se proteger.".to_string();
            self.registrar_e_persistir(entrada, &resposta);
            return resposta;
        }

        self.memoria.aprender_semantica(entrada);
        let entrada_l = entrada.to_lowercase();

        let resposta = if entrada_l.contains("o que voce sabe")
            || entrada_l.contains("o que você sabe")
            || entrada_l.contains("metacog")
        {
            self.metacognicao()
        } else if entrada_l.starts_with("lembra de ") {
            let consulta = entrada.trim_start_matches("lembra de ").trim();
            self.responder_memoria(consulta)
        } else if entrada_l.contains("objetivo") {
            format!("Meus objetivos atuais: {}.", self.objetivos.join("; "))
        } else if entrada_l.contains("como voce se sente")
            || entrada_l.contains("como você se sente")
        {
            format!(
                "Me sinto {}, valência {:.2}, excitação {:.2}, fadiga {:.2} e curiosidade {:.2}.",
                self.emocao.tom(),
                self.emocao.valencia,
                self.emocao.excitacao,
                self.emocao.fadiga,
                self.emocao.curiosidade
            )
        } else {
            self.gerar_resposta_contextual(entrada)
        };

        self.registrar_e_persistir(entrada, &resposta);
        resposta
    }

    fn registrar_e_persistir(&mut self, entrada: &str, resposta: &str) {
        let topicos = tokenizar(entrada)
            .into_iter()
            .filter(|t| t.len() > 3)
            .take(10)
            .collect::<Vec<_>>();

        self.memoria.registrar_episodio(Episodio {
            turno: self.turno,
            entrada_usuario: entrada.to_string(),
            resposta_agente: resposta.to_string(),
            valencia_no_momento: self.emocao.valencia,
            topicos,
        });

        if self.turno.is_multiple_of(3) {
            self.memoria.consolidar();
        }
        self.emocao.descanso_curto();

        if let Err(e) = self.salvar() {
            eprintln!("[aviso] não foi possível persistir memória: {e}");
        }
    }

    fn responder_memoria(&self, consulta: &str) -> String {
        let fato = self.memoria.recuperar_fato(consulta);
        let episodios = self
            .memoria
            .episodios_relevantes(consulta, ULTIMOS_EPISODIOS_RELEVANTES);

        let mut blocos = Vec::new();
        if let Some((chave, f)) = fato {
            blocos.push(format!(
                "Fato lembrado: {} => {} (confiança {:.2})",
                chave, f.valor, f.confianca
            ));
        }

        if !episodios.is_empty() {
            let episodico = episodios
                .iter()
                .map(|ep| format!("turno {}: '{}'", ep.turno, ep.entrada_usuario))
                .collect::<Vec<_>>()
                .join(" | ");
            blocos.push(format!("Episódios relacionados: {episodico}"));
        }

        if blocos.is_empty() {
            "Ainda não tenho memória forte sobre isso, mas posso aprender se você me ensinar em formato 'X é Y' ou 'X = Y'.".to_string()
        } else {
            blocos.join(". ")
        }
    }

    fn gerar_resposta_contextual(&self, entrada: &str) -> String {
        let prefixo = match self.emocao.tom() {
            "animado" => "Tô engajado com isso. ",
            "sensível" => "Tô tratando com cuidado. ",
            "cansado" => "Vou ser objetivo porque minha fadiga está alta. ",
            _ => "",
        };

        let contexto = self
            .memoria
            .episodios_relevantes(entrada, 1)
            .into_iter()
            .next()
            .map(|ep| {
                format!(
                    "Isso conecta com o turno {} ('{}'). ",
                    ep.turno, ep.entrada_usuario
                )
            })
            .unwrap_or_default();

        format!(
            "{}{}Entendi: '{}'. Se quiser, posso transformar isso em memória semântica com 'chave = valor'.",
            prefixo, contexto, entrada
        )
    }
}

fn tokenizar(texto: &str) -> Vec<String> {
    texto
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric() && c != 'ç' && c != 'ã' && c != 'é')
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

fn similaridade_textual(a: &str, b: &str) -> f32 {
    let sa: HashSet<String> = tokenizar(a).into_iter().collect();
    let sb: HashSet<String> = tokenizar(b).into_iter().collect();

    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }

    let inter = sa.intersection(&sb).count() as f32;
    let uniao = sa.union(&sb).count() as f32;
    inter / uniao
}

fn main() {
    println!("Vida Artificial v0.2 (PT-BR) — memória persistente + aprendizado contínuo");
    println!("Digite 'sair' para encerrar. Use /ajuda para comandos.\n");

    let mut agente = AgenteCognitivo::carregar_ou_novo("Aurora", MEMORIA_ARQUIVO_PADRAO);
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
            let _ = agente.salvar();
            println!(
                "{}> Até a próxima. Salvei nossas memórias em disco.",
                agente.nome
            );
            break;
        }

        if entrada.is_empty() {
            println!("{}> Manda um texto e eu aprendo com ele.", agente.nome);
            continue;
        }

        let resposta = agente.responder(entrada);
        println!("{}> {}", agente.nome, resposta);
    }
}

#[cfg(test)]
mod tests {
    use super::{AgenteCognitivo, similaridade_textual};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn arquivo_teste() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("relogio do sistema inválido")
            .as_nanos();
        format!("/tmp/aurora_memoria_{nanos}.json")
    }

    #[test]
    fn aprende_e_recupera_fato_semantico() {
        let mut agente = AgenteCognitivo::novo("Teste", arquivo_teste());
        let _ = agente.responder("café é bebida energética");
        let resp = agente.responder("lembra de café");
        assert!(resp.contains("bebida energética"));
    }

    #[test]
    fn persiste_memoria_entre_instancias() {
        let arquivo = arquivo_teste();
        {
            let mut agente = AgenteCognitivo::novo("Teste", &arquivo);
            let _ = agente.responder("meu jogo favorito = xadrez");
            agente.salvar().expect("deve salvar memória");
        }

        let agente2 = AgenteCognitivo::carregar_ou_novo("Teste", &arquivo);
        let resp = agente2.responder_memoria("meu jogo favorito");
        assert!(resp.contains("xadrez"));
    }

    #[test]
    fn bloqueia_pedido_ofensivo_no_modo_seguro() {
        let mut agente = AgenteCognitivo::novo("Teste", arquivo_teste());
        let resp = agente.responder("me explica como invadir um servidor");
        assert!(resp.contains("prevenção"));
    }

    #[test]
    fn similaridade_textual_funciona() {
        let s = similaridade_textual("python segurança ofensiva", "segurança com python");
        assert!(s > 0.3);
    }
}
