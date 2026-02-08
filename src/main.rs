use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MEMORIA_ARQUIVO_PADRAO: &str = "./.aurora_memoria.json";
const MAX_REFLEXOES: usize = 1500;
const MAX_LACUNAS: usize = 80;

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
            curiosidade: 0.65,
        }
    }

    fn aplicar_input(&mut self, texto: &str) {
        let t = texto.to_lowercase();
        if ["obrigado", "valeu", "show", "perfeito", "boa"]
            .iter()
            .any(|k| t.contains(k))
        {
            self.valencia += 0.10;
            self.excitacao += 0.05;
        }
        if ["ruim", "triste", "raiva", "frustrado", "decepcionado"]
            .iter()
            .any(|k| t.contains(k))
        {
            self.valencia -= 0.14;
            self.excitacao += 0.06;
        }
        if ["?", "como", "por que", "me ensina", "passo a passo"]
            .iter()
            .any(|k| t.contains(k))
        {
            self.curiosidade += 0.12;
        }

        self.fadiga += 0.02;
        self.normalizar();
    }

    fn descanso_curto(&mut self) {
        self.fadiga = (self.fadiga - 0.013).max(0.0);
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
        if self.fadiga > 0.8 {
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
    entrada: String,
    resposta: String,
    topicos: Vec<String>,
    valencia_no_momento: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FatoSemantico {
    valor: String,
    confianca: f32,
    revisoes: usize,
    fonte: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RevisaoFato {
    chave: String,
    valor_anterior: Option<String>,
    valor_novo: String,
    turno: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LacunaConhecimento {
    topico: String,
    tentativas: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PerfilUsuario {
    preferencias: HashMap<String, String>,
    objetivo_atual: Option<String>,
    estilo: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Memoria {
    episodica: Vec<Episodio>,
    semantica: HashMap<String, FatoSemantico>,
    revisoes: Vec<RevisaoFato>,
    conceitos_vistos: HashMap<String, usize>,
    coocorrencia: HashMap<String, HashMap<String, usize>>,
    reflexoes: Vec<String>,
    lacunas: VecDeque<LacunaConhecimento>,
    perfil_usuario: PerfilUsuario,
}

impl Memoria {
    fn aprender_semantica_texto(&mut self, texto: &str, turno: usize) {
        let limpo = texto.trim();
        for marcador in [" é ", " sao ", " são ", " significa ", " quer dizer "] {
            if let Some((sujeito, predicado)) = limpo.split_once(marcador) {
                self.registrar_fato(sujeito, predicado, "usuario", turno);
            }
        }
        if let Some((k, v)) = limpo.split_once('=') {
            self.registrar_fato(k, v, "usuario", turno);
        }

        self.aprender_perfil_usuario(limpo);
        self.atualizar_topologia(limpo);
    }

    fn aprender_perfil_usuario(&mut self, texto: &str) {
        let low = texto.to_lowercase();
        if let Some((_, valor)) = low.split_once("eu gosto de ") {
            self.perfil_usuario
                .preferencias
                .insert("gosto".to_string(), valor.trim().to_string());
        }
        if let Some((_, valor)) = low.split_once("meu objetivo ") {
            self.perfil_usuario.objetivo_atual = Some(valor.trim().to_string());
        }
        if low.contains("responda curto") {
            self.perfil_usuario.estilo = Some("curto".to_string());
        } else if low.contains("responda detalhado") {
            self.perfil_usuario.estilo = Some("detalhado".to_string());
        }
    }

    fn atualizar_topologia(&mut self, texto: &str) {
        let termos = termos_relevantes(texto);
        for t in &termos {
            *self.conceitos_vistos.entry(t.clone()).or_insert(0) += 1;
        }

        for i in 0..termos.len() {
            for j in (i + 1)..termos.len() {
                let a = termos[i].clone();
                let b = termos[j].clone();
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

    fn registrar_fato(&mut self, chave: &str, valor: &str, fonte: &str, turno: usize) {
        let key = normalizar_chave(chave);
        let value = valor.trim().to_string();
        if key.is_empty() || value.is_empty() {
            return;
        }

        let anterior = self.semantica.get(&key).map(|f| f.valor.clone());
        let item = self.semantica.entry(key.clone()).or_insert(FatoSemantico {
            valor: value.clone(),
            confianca: 0.45,
            revisoes: 0,
            fonte: fonte.to_string(),
        });

        if item.valor != value {
            item.valor = value.clone();
            item.confianca = (item.confianca * 0.88).max(0.35);
        }
        item.revisoes += 1;
        item.confianca = (item.confianca + 0.11).min(0.99);
        item.fonte = fonte.to_string();

        self.revisoes.push(RevisaoFato {
            chave: key,
            valor_anterior: anterior,
            valor_novo: value,
            turno,
        });
    }

    fn registrar_lacuna(&mut self, topico: &str) {
        let topico = normalizar_chave(topico);
        if topico.len() < 3 {
            return;
        }
        if let Some(item) = self.lacunas.iter_mut().find(|l| l.topico == topico) {
            item.tentativas += 1;
            return;
        }
        if self.lacunas.len() >= MAX_LACUNAS {
            let _ = self.lacunas.pop_front();
        }
        self.lacunas.push_back(LacunaConhecimento {
            topico,
            tentativas: 1,
        });
    }

    fn registrar_episodio(&mut self, episodio: Episodio) {
        self.episodica.push(episodio);
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
            .filter(|(_, _, s)| *s > 0.24)
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
            .map(|(k, v, _)| (k.clone(), v.clone()))
    }

    fn inferir_fato(&self, consulta: &str) -> Option<String> {
        let q = normalizar_chave(consulta);
        let direto = self.semantica.get(&q)?;
        let ponte = normalizar_chave(&direto.valor);
        let segundo = self.semantica.get(&ponte)?;
        Some(format!(
            "inferência: se {} => {} e {} => {}, então {} se relaciona com {}",
            q, direto.valor, ponte, segundo.valor, q, segundo.valor
        ))
    }

    fn episodios_relevantes(&self, consulta: &str, limite: usize) -> Vec<&Episodio> {
        let mut pontuados = self
            .episodica
            .iter()
            .map(|ep| (ep, similaridade_textual(consulta, &ep.entrada)))
            .filter(|(_, s)| *s > 0.2)
            .collect::<Vec<_>>();
        pontuados.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        pontuados
            .into_iter()
            .take(limite)
            .map(|(ep, _)| ep)
            .collect()
    }

    fn consolidar(&mut self, turno: usize) {
        for (conceito, freq) in self.conceitos_vistos.clone() {
            if freq >= 4 {
                self.semantica
                    .entry(conceito.clone())
                    .or_insert(FatoSemantico {
                        valor: format!("conceito recorrente ({freq} menções)"),
                        confianca: 0.4,
                        revisoes: 1,
                        fonte: "consolidacao".to_string(),
                    });
            }
        }

        let lacunas = self
            .lacunas
            .iter()
            .take(3)
            .map(|l| l.topico.clone())
            .collect::<Vec<_>>()
            .join(", ");

        self.reflexoes.push(format!(
            "Turno {}: consolidei {} fatos; lacunas ativas [{}].",
            turno,
            self.semantica.len(),
            lacunas
        ));

        if self.reflexoes.len() > MAX_REFLEXOES {
            let remove = self.reflexoes.len() - MAX_REFLEXOES;
            self.reflexoes.drain(0..remove);
        }
    }

    fn resumo(&self) -> String {
        format!(
            "{} fatos | {} episódios | {} lacunas | {} revisões de crença",
            self.semantica.len(),
            self.episodica.len(),
            self.lacunas.len(),
            self.revisoes.len()
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Persistencia {
    nome: String,
    objetivos: Vec<String>,
    emocao: EstadoEmocional,
    memoria: Memoria,
    turno: usize,
}

#[derive(Debug)]
struct AgenteCognitivo {
    nome: String,
    objetivos: Vec<String>,
    emocao: EstadoEmocional,
    memoria: Memoria,
    turno: usize,
    arquivo_memoria: PathBuf,
    modo_seguro: bool,
}

impl AgenteCognitivo {
    fn novo(nome: &str, arquivo: impl AsRef<Path>) -> Self {
        Self {
            nome: nome.to_string(),
            objetivos: vec![
                "aprender continuamente com o usuário".to_string(),
                "preservar memória e rever crenças quando necessário".to_string(),
                "manter curiosidade ativa e útil".to_string(),
                "responder em português brasileiro com naturalidade".to_string(),
            ],
            emocao: EstadoEmocional::novo(),
            memoria: Memoria::default(),
            turno: 0,
            arquivo_memoria: arquivo.as_ref().to_path_buf(),
            modo_seguro: true,
        }
    }

    fn carregar_ou_novo(nome: &str, arquivo: impl AsRef<Path>) -> Self {
        let path = arquivo.as_ref();
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(p) = serde_json::from_str::<Persistencia>(&raw)
        {
            return Self {
                nome: p.nome,
                objetivos: p.objetivos,
                emocao: p.emocao,
                memoria: p.memoria,
                turno: p.turno,
                arquivo_memoria: path.to_path_buf(),
                modo_seguro: true,
            };
        }
        Self::novo(nome, path)
    }

    fn salvar(&self) -> Result<(), String> {
        let dump = Persistencia {
            nome: self.nome.clone(),
            objetivos: self.objetivos.clone(),
            emocao: self.emocao.clone(),
            memoria: self.memoria.clone(),
            turno: self.turno,
        };
        let data = serde_json::to_string_pretty(&dump)
            .map_err(|e| format!("erro serializando memória: {e}"))?;
        fs::write(&self.arquivo_memoria, data).map_err(|e| format!("erro salvando memória: {e}"))
    }

    fn processar_comando(&mut self, entrada: &str) -> Option<String> {
        if !entrada.starts_with('/') {
            return None;
        }
        let low = entrada.to_lowercase();
        let resp = if low == "/ajuda" {
            "Comandos: /ajuda, /status, /salvar, /recarregar, /fato <chave>, /ensinar <chave>=<valor>, /topicos, /tutorial, /modo-seguro on|off".to_string()
        } else if low == "/status" {
            format!("Status => {}", self.memoria.resumo())
        } else if low == "/salvar" {
            match self.salvar() {
                Ok(_) => format!("Memória salva em {}", self.arquivo_memoria.display()),
                Err(e) => format!("Falha ao salvar: {e}"),
            }
        } else if low == "/recarregar" {
            *self = Self::carregar_ou_novo(&self.nome, &self.arquivo_memoria);
            "Memória recarregada.".to_string()
        } else if low.starts_with("/fato ") {
            let q = entrada.trim_start_matches("/fato ").trim();
            self.responder_memoria(q)
        } else if low.starts_with("/ensinar ") {
            let resto = entrada.trim_start_matches("/ensinar ").trim();
            if let Some((k, v)) = resto.split_once('=') {
                self.turno += 1;
                self.memoria
                    .registrar_fato(k, v, "comando_ensinar", self.turno);
                self.memoria.registrar_episodio(Episodio {
                    turno: self.turno,
                    entrada: entrada.to_string(),
                    resposta: "fato aprendido".to_string(),
                    topicos: termos_relevantes(resto),
                    valencia_no_momento: self.emocao.valencia,
                });
                let _ = self.salvar();
                format!("Aprendi: {} = {}", k.trim(), v.trim())
            } else {
                "Uso: /ensinar chave = valor".to_string()
            }
        } else if low == "/topicos" {
            let mut t = self.memoria.conceitos_vistos.iter().collect::<Vec<_>>();
            t.sort_by(|a, b| b.1.cmp(a.1));
            format!(
                "Tópicos: {}",
                t.into_iter()
                    .take(12)
                    .map(|(k, v)| format!("{k}({v})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else if low == "/tutorial" {
            tutorial_instalacao().to_string()
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
        } else {
            "Comando desconhecido. Use /ajuda.".to_string()
        };
        Some(resp)
    }

    fn violacao_seguranca(&self, entrada: &str) -> bool {
        if !self.modo_seguro {
            return false;
        }
        let l = entrada.to_lowercase();
        [
            "como invadir",
            "fazer malware",
            "burlar senha",
            "explorar vulnerabilidade",
            "golpe",
            "phishing",
        ]
        .iter()
        .any(|k| l.contains(k))
    }

    fn responder(&mut self, entrada: &str) -> String {
        if let Some(cmd) = self.processar_comando(entrada) {
            return cmd;
        }

        self.turno += 1;
        self.emocao.aplicar_input(entrada);

        if self.violacao_seguranca(entrada) {
            let r = "Posso ajudar com prevenção e segurança defensiva, mas não com instruções ofensivas.".to_string();
            self.registrar_turno(entrada, &r);
            return r;
        }

        self.memoria.aprender_semantica_texto(entrada, self.turno);
        let low = entrada.to_lowercase();

        let resposta = if low.contains("o que você sabe") || low.contains("o que voce sabe") {
            self.metacognicao()
        } else if low.starts_with("lembra de ") {
            self.responder_memoria(entrada.trim_start_matches("lembra de ").trim())
        } else if low.contains("passo a passo") || low.contains("desde o inicio") {
            tutorial_instalacao().to_string()
        } else if low.contains("objetivo") {
            format!("Meus objetivos: {}.", self.objetivos.join("; "))
        } else if low.contains("como você se sente") || low.contains("como voce se sente") {
            format!(
                "Agora me sinto {}, com valência {:.2}, excitação {:.2}, fadiga {:.2}, curiosidade {:.2}.",
                self.emocao.tom(),
                self.emocao.valencia,
                self.emocao.excitacao,
                self.emocao.fadiga,
                self.emocao.curiosidade
            )
        } else {
            self.resposta_viva(entrada)
        };

        self.registrar_turno(entrada, &resposta);
        resposta
    }

    fn metacognicao(&self) -> String {
        format!(
            "Eu sei {} fatos, tenho {} episódios, {} lacunas e {} revisões de crença. Tom atual: {}.",
            self.memoria.semantica.len(),
            self.memoria.episodica.len(),
            self.memoria.lacunas.len(),
            self.memoria.revisoes.len(),
            self.emocao.tom()
        )
    }

    fn resposta_viva(&mut self, entrada: &str) -> String {
        let pref_estilo = self
            .memoria
            .perfil_usuario
            .estilo
            .clone()
            .unwrap_or_else(|| "normal".to_string());

        let prefixo = match self.emocao.tom() {
            "animado" => "Tô bem engajado nisso. ",
            "sensível" => "Tô processando isso com cuidado. ",
            "cansado" => "Vou ser direto para manter qualidade. ",
            _ => "",
        };

        let contexto = self
            .memoria
            .episodios_relevantes(entrada, 1)
            .into_iter()
            .next()
            .map(|e| format!("Isso conecta com o turno {} ('{}'). ", e.turno, e.entrada))
            .unwrap_or_default();

        let curiosa = self.pergunta_curiosa(entrada);
        if pref_estilo == "curto" {
            format!("{}{}{}", prefixo, contexto, curiosa)
        } else {
            format!("{}{}Entendi: '{}'. {}", prefixo, contexto, entrada, curiosa)
        }
    }

    fn pergunta_curiosa(&mut self, entrada: &str) -> String {
        let termos = termos_relevantes(entrada);
        let desconhecidos = termos
            .iter()
            .filter(|t| !self.memoria.semantica.contains_key(*t))
            .cloned()
            .collect::<Vec<_>>();

        if desconhecidos.is_empty() {
            if let Some(obj) = &self.memoria.perfil_usuario.objetivo_atual {
                return format!("Quer avançar mais um passo em '{}' agora?", obj);
            }
            return "Quer que eu aprofunde em algum ponto específico?".to_string();
        }

        let topico = &desconhecidos[0];
        self.memoria.registrar_lacuna(topico);
        format!(
            "Fiquei curioso sobre '{}'. Se você ensinar '{} = ...', eu incorporo isso imediatamente.",
            topico, topico
        )
    }

    fn responder_memoria(&self, consulta: &str) -> String {
        let fato = self.memoria.recuperar_fato(consulta);
        let episodios = self.memoria.episodios_relevantes(consulta, 3);
        let inferencia = self.memoria.inferir_fato(consulta);

        let mut partes = Vec::new();
        if let Some((k, f)) = fato {
            partes.push(format!(
                "Fato: {} => {} (confiança {:.2}, revisões {}, fonte {}).",
                k, f.valor, f.confianca, f.revisoes, f.fonte
            ));
        }
        if !episodios.is_empty() {
            partes.push(format!(
                "Episódios: {}.",
                episodios
                    .iter()
                    .map(|e| format!("t{}:'{}'", e.turno, e.entrada))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        if let Some(i) = inferencia {
            partes.push(i);
        }

        if partes.is_empty() {
            "Ainda não sei isso. Me ensine com 'chave = valor' e eu vou lembrar nas próximas conversas.".to_string()
        } else {
            partes.join(" ")
        }
    }

    fn registrar_turno(&mut self, entrada: &str, resposta: &str) {
        self.memoria.registrar_episodio(Episodio {
            turno: self.turno,
            entrada: entrada.to_string(),
            resposta: resposta.to_string(),
            topicos: termos_relevantes(entrada),
            valencia_no_momento: self.emocao.valencia,
        });

        if self.turno.is_multiple_of(3) {
            self.memoria.consolidar(self.turno);
        }

        self.emocao.descanso_curto();
        if let Err(e) = self.salvar() {
            eprintln!("[aviso] falha ao salvar memória: {e}");
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
    const STOP: &[&str] = &[
        "de", "da", "do", "das", "dos", "a", "o", "e", "um", "uma", "que", "como", "para", "com",
        "sem", "por", "em", "no", "na", "nos", "nas", "eu", "você", "voce", "me", "te",
    ];

    texto
        .split_whitespace()
        .map(normalizar_chave)
        .filter(|t| t.len() > 2 && !STOP.contains(&t.as_str()))
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
    "PASSO A PASSO (do zero):\n1) Instale Rust: https://rustup.rs\n2) Abra o terminal e entre no projeto: cd /workspace/AI\n3) Formate: cargo fmt\n4) Teste: cargo test\n5) Execute: cargo run\n6) Ensine: /ensinar capital do brasil = brasilia\n7) Consulte: /fato capital do brasil ou lembra de capital do brasil\n8) Veja status: /status\n9) Ajuste segurança: /modo-seguro on|off\n10) Encerrar: sair\nObs.: o arquivo ./.aurora_memoria.json guarda o aprendizado entre sessões."
}

fn main() {
    println!("Vida Artificial v0.4 (PT-BR) — aprendizado incremental + revisão de crenças");
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
            println!("{}> Manda um texto e eu aprendo contigo.", agente.nome);
            continue;
        }

        let resposta = agente.responder(entrada);
        println!("{}> {}", agente.nome, resposta);
    }
}

#[cfg(test)]
mod tests {
    use super::{AgenteCognitivo, similaridade_textual, tutorial_instalacao};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn arquivo_teste() -> String {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("tempo inválido")
            .as_nanos();
        format!("/tmp/aurora_v4_{n}.json")
    }

    #[test]
    fn aprende_por_comando_e_recupera() {
        let mut a = AgenteCognitivo::novo("Teste", arquivo_teste());
        let _ = a.responder("/ensinar python = linguagem");
        let r = a.responder("/fato python");
        assert!(r.contains("linguagem"));
    }

    #[test]
    fn revisa_crenca() {
        let mut a = AgenteCognitivo::novo("Teste", arquivo_teste());
        let _ = a.responder("terra é planeta");
        let _ = a.responder("terra é nosso lar");
        let r = a.responder("/fato terra");
        assert!(r.contains("revisões"));
    }

    #[test]
    fn inferencia_basica() {
        let mut a = AgenteCognitivo::novo("Teste", arquivo_teste());
        let _ = a.responder("rust é linguagem");
        let _ = a.responder("linguagem é ferramenta de pensamento");
        let r = a.responder("/fato rust");
        assert!(r.contains("inferência"));
    }

    #[test]
    fn tutorial_contem_execucao() {
        assert!(tutorial_instalacao().contains("cargo run"));
    }

    #[test]
    fn similaridade_ok() {
        assert!(similaridade_textual("aprender rust rápido", "rust para aprender") > 0.2);
    }
}
