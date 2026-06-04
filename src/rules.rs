//! Mechanizm reguł sterowany danymi.
//!
//! Reguły zmiany nazwy są wczytywane z pliku `rules.json` w katalogu
//! konfiguracyjnym, a nie zaszyte w kodzie. Każda reguła to wzór `regex`
//! oraz szablon nazwy pliku. Dzięki temu dodanie nowego typu dokumentu
//! (np. PIT-4, składka zdrowotna, opłata) sprowadza się do edycji pliku
//! JSON — bez ponownej kompilacji aplikacji.
//!
//! ## Szablony
//!
//! Tekst poza `{...}` jest dosłowny. Wewnątrz `{...}` znajduje się wyrażenie:
//! - `{nazwa}` — wartość nazwanej grupy przechwytującej (`(?P<nazwa>...)`),
//! - `{funkcja(arg1, arg2)}` — wywołanie funkcji pomocniczej, gdzie każdy
//!   argument jest **wyrażeniem**: grupą, literałem tekstowym `"..."`, albo
//!   **zagnieżdżonym** wywołaniem funkcji (np. `{upper(nodash(form))}`).
//!
//! Nawiasy klamrowe w dosłownym tekście zapisuje się przez podwojenie:
//! `{{` → `{`, `}}` → `}`. W literałach tekstowych działa `\"` i `\\`.
//!
//! Dostępne funkcje pomocnicze:
//! - `nodash(x)`            — usuwa myślniki (np. `PIT-5` → `PIT5`)
//! - `upper(x)` / `lower(x)` — zmiana wielkości liter
//! - `pad(x, "n")`          — dopełnia `x` zerami z lewej do szerokości `n`
//! - `prevmonthyear(m, y)`  — z miesiąca i roku tworzy `MMYYYY` poprzedniego
//!   miesiąca (z przeniesieniem roku w styczniu); używane dla ZUS.
//!
//! Przy kompilacji reguły sprawdzane są: poprawność składni szablonu, istnienie
//! użytych grup w wyrażeniu regularnym oraz nazwa i liczba argumentów funkcji.
//! Reguła z błędem jest pomijana (z wpisem w logu), a nie cicho ignorowana.

use serde::{Deserialize, Serialize};
use regex::{Regex, Captures};
use log::{info, warn, error};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Pojedyncza reguła w postaci danych (serializowana do JSON).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub pattern: String,
    pub template: String,
}

/// Zestaw reguł wczytywany z `rules.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}

impl Default for RulesConfig {
    fn default() -> Self {
        RulesConfig {
            rules: vec![
                Rule {
                    name: "Formularz podatkowy PIT-5 / VAT-7".to_string(),
                    pattern: r"OKR/\s*(?P<year>\d{2})M(?P<month>\d{2})/SFP/(?P<form>PIT-5|VAT-7)".to_string(),
                    template: "{nodash(form)}-{month}{year}.pdf".to_string(),
                },
                Rule {
                    name: "Potwierdzenie przelewu ZUS".to_string(),
                    // (?s): `.` obejmuje znaki nowej linii; `.*?` jest leniwe.
                    pattern: r"(?s)DANE ODBIORCY\s*Zakład Ubezpieczeń Społecznych.*?DATA OPERACJI\s*(?P<day>\d{2})-(?P<month>\d{2})-(?P<year>\d{4})".to_string(),
                    template: "ZUS-{prevmonthyear(month, year)}.pdf".to_string(),
                },
            ],
        }
    }
}

impl RulesConfig {
    pub fn rules_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("invoices-renamer");
        fs::create_dir_all(&path).ok();
        path.push("rules.json");
        path
    }

    /// Wczytuje reguły z pliku. Gdy plik nie istnieje, zapisuje i zwraca
    /// reguły domyślne (dzięki czemu użytkownik ma od czego zacząć edycję).
    /// Gdy plik jest uszkodzony, loguje błąd i używa reguł domyślnych.
    pub fn load_or_create() -> Self {
        let path = Self::rules_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str::<RulesConfig>(&contents) {
                    Ok(cfg) => {
                        info!("Załadowano {} reguł z {}", cfg.rules.len(), path.display());
                        cfg
                    }
                    Err(e) => {
                        error!("Błąd parsowania {} ({}), używam reguł domyślnych", path.display(), e);
                        RulesConfig::default()
                    }
                },
                Err(e) => {
                    error!("Nie można odczytać {} ({}), używam reguł domyślnych", path.display(), e);
                    RulesConfig::default()
                }
            }
        } else {
            let cfg = RulesConfig::default();
            match cfg.save() {
                Ok(_) => info!("Utworzono domyślny plik reguł: {}", path.display()),
                Err(e) => warn!("Nie można zapisać domyślnych reguł: {}", e),
            }
            cfg
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::rules_path();
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }
}

/// Skompilowana, gotowa do użycia reguła.
pub struct CompiledRule {
    pub name: String,
    regex: Regex,
    template: Vec<TemplatePart>,
}

/// Fragment sparsowanego szablonu nazwy pliku.
enum TemplatePart {
    /// Dosłowny tekst (po odkodowaniu `{{`/`}}`).
    Literal(String),
    /// Wyrażenie z wnętrza `{...}`.
    Expr(Expr),
}

/// Wyrażenie w szablonie.
enum Expr {
    /// Nazwana grupa przechwytująca z regex.
    Capture(String),
    /// Literał tekstowy `"..."`.
    StrLit(String),
    /// Wywołanie funkcji pomocniczej z argumentami (mogą być zagnieżdżone).
    Call(String, Vec<Expr>),
}

/// Kompiluje zestaw reguł. Reguły z błędnym wzorem lub szablonem są
/// pomijane (z wpisem w logu), aby pojedyncza literówka nie wyłączyła
/// całej aplikacji.
pub fn compile(config: &RulesConfig) -> Vec<CompiledRule> {
    let mut compiled = Vec::new();
    for rule in &config.rules {
        let regex = match Regex::new(&rule.pattern) {
            Ok(r) => r,
            Err(e) => {
                error!("Pomijam regułę '{}': błędny wzór regex: {}", rule.name, e);
                continue;
            }
        };
        let template = match parse_template(&rule.template) {
            Ok(t) => t,
            Err(e) => {
                error!("Pomijam regułę '{}': błędny szablon: {}", rule.name, e);
                continue;
            }
        };
        // Walidacja: czy szablon odwołuje się tylko do istniejących grup i
        // znanych funkcji o właściwej liczbie argumentów.
        let capture_names: HashSet<&str> = regex.capture_names().flatten().collect();
        if let Err(e) = validate_template(&template, &capture_names) {
            error!("Pomijam regułę '{}': {}", rule.name, e);
            continue;
        }
        compiled.push(CompiledRule { name: rule.name.clone(), regex, template });
    }
    compiled
}

/// Zwraca nazwę pliku z pierwszej reguły, która pasuje do tekstu.
pub fn apply_rules(text: &str, rules: &[CompiledRule]) -> Option<String> {
    for rule in rules {
        if let Some(caps) = rule.regex.captures(text) {
            match render(&rule.template, &caps) {
                Some(name) => {
                    info!("Dopasowano regułę '{}'", rule.name);
                    return Some(name);
                }
                None => {
                    warn!("Reguła '{}' dopasowana, ale nie udało się zbudować nazwy", rule.name);
                    continue;
                }
            }
        }
    }
    None
}

/// Aktywne reguły (z pliku lub domyślne), kompilowane raz na czas życia procesu.
fn active_rules() -> &'static [CompiledRule] {
    static ACTIVE_RULES: OnceLock<Vec<CompiledRule>> = OnceLock::new();
    ACTIVE_RULES.get_or_init(|| compile(&RulesConfig::load_or_create()))
}

/// Wygodne wejście używane przez przetwarzanie plików: stosuje aktywne reguły.
pub fn determine_new_filename(text: &str) -> Option<String> {
    apply_rules(text, active_rules())
}

// --- Silnik szablonów: parser rekurencyjny ------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Parser { chars: s.chars().collect(), pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// template := ( "{{" | "}}" | literał | "{" expr "}" )*
    fn parse_template(&mut self) -> Result<Vec<TemplatePart>, String> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('{') => {
                    self.bump();
                    if self.peek() == Some('{') {
                        self.bump();
                        literal.push('{');
                        continue;
                    }
                    if !literal.is_empty() {
                        parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                    }
                    let expr = self.parse_expr()?;
                    self.skip_ws();
                    match self.bump() {
                        Some('}') => {}
                        _ => return Err("oczekiwano '}' zamykającego wyrażenie".to_string()),
                    }
                    parts.push(TemplatePart::Expr(expr));
                }
                Some('}') => {
                    self.bump();
                    if self.peek() == Some('}') {
                        self.bump();
                        literal.push('}');
                    } else {
                        return Err("nieoczekiwany '}' (dla dosłownego znaku użyj '}}')".to_string());
                    }
                }
                Some(c) => {
                    self.bump();
                    literal.push(c);
                }
            }
        }
        if !literal.is_empty() {
            parts.push(TemplatePart::Literal(literal));
        }
        Ok(parts)
    }

    /// expr := literał_tekstowy | ident [ "(" args ")" ]
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        match self.peek() {
            Some('"') => self.parse_string_literal(),
            Some(c) if is_ident_start(c) => {
                let ident = self.parse_ident();
                self.skip_ws();
                if self.peek() == Some('(') {
                    self.bump();
                    let args = self.parse_args()?;
                    self.skip_ws();
                    match self.bump() {
                        Some(')') => {}
                        _ => return Err(format!("oczekiwano ')' po argumentach '{}'", ident)),
                    }
                    Ok(Expr::Call(ident, args))
                } else {
                    Ok(Expr::Capture(ident))
                }
            }
            Some(c) => Err(format!("nieoczekiwany znak '{}' w wyrażeniu", c)),
            None => Err("nieoczekiwany koniec szablonu w wyrażeniu".to_string()),
        }
    }

    /// args := [ expr ( "," expr )* ]
    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        self.skip_ws();
        if self.peek() == Some(')') {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(')') => break,
                _ => return Err("oczekiwano ',' lub ')' w argumentach".to_string()),
            }
        }
        Ok(args)
    }

    fn parse_ident(&mut self) -> String {
        let mut s = String::new();
        while matches!(self.peek(), Some(c) if is_ident_char(c)) {
            s.push(self.bump().unwrap());
        }
        s
    }

    fn parse_string_literal(&mut self) -> Result<Expr, String> {
        self.bump(); // otwierający "
        let mut s = String::new();
        loop {
            match self.bump() {
                None => return Err("niezamknięty literał tekstowy".to_string()),
                Some('"') => return Ok(Expr::StrLit(s)),
                Some('\\') => match self.bump() {
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(other) => {
                        s.push('\\');
                        s.push(other);
                    }
                    None => return Err("niezamknięty literał tekstowy".to_string()),
                },
                Some(c) => s.push(c),
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn parse_template(template: &str) -> Result<Vec<TemplatePart>, String> {
    let mut parser = Parser::new(template);
    let parts = parser.parse_template()?;
    if parser.pos != parser.chars.len() {
        return Err("niesparsowana reszta szablonu".to_string());
    }
    Ok(parts)
}

// --- Walidacja przy kompilacji ------------------------------------------

fn validate_template(parts: &[TemplatePart], captures: &HashSet<&str>) -> Result<(), String> {
    for part in parts {
        if let TemplatePart::Expr(e) = part {
            validate_expr(e, captures)?;
        }
    }
    Ok(())
}

fn validate_expr(expr: &Expr, captures: &HashSet<&str>) -> Result<(), String> {
    match expr {
        Expr::StrLit(_) => Ok(()),
        Expr::Capture(name) => {
            if captures.contains(name.as_str()) {
                Ok(())
            } else {
                Err(format!("szablon używa nieznanej grupy '{}'", name))
            }
        }
        Expr::Call(name, args) => {
            match helper_arity(name) {
                Some(arity) if arity == args.len() => {}
                Some(arity) => {
                    return Err(format!(
                        "funkcja '{}' oczekuje {} argument(ów), podano {}",
                        name,
                        arity,
                        args.len()
                    ))
                }
                None => return Err(format!("nieznana funkcja '{}'", name)),
            }
            for a in args {
                validate_expr(a, captures)?;
            }
            Ok(())
        }
    }
}

/// Liczba argumentów znanych funkcji (źródło prawdy dla walidacji).
fn helper_arity(name: &str) -> Option<usize> {
    match name {
        "nodash" | "upper" | "lower" => Some(1),
        "pad" | "prevmonthyear" => Some(2),
        _ => None,
    }
}

// --- Ewaluacja ----------------------------------------------------------

fn render(parts: &[TemplatePart], caps: &Captures) -> Option<String> {
    let mut out = String::new();
    for part in parts {
        match part {
            TemplatePart::Literal(s) => out.push_str(s),
            TemplatePart::Expr(e) => out.push_str(&eval_expr(e, caps)?),
        }
    }
    Some(out)
}

fn eval_expr(expr: &Expr, caps: &Captures) -> Option<String> {
    match expr {
        Expr::StrLit(s) => Some(s.clone()),
        Expr::Capture(name) => caps.name(name).map(|m| m.as_str().to_string()),
        Expr::Call(name, args) => {
            let vals: Option<Vec<String>> = args.iter().map(|a| eval_expr(a, caps)).collect();
            apply_helper(name, &vals?)
        }
    }
}

/// Rejestr funkcji pomocniczych szablonu. Dodając nową, zaktualizuj też
/// `helper_arity` (używane przy walidacji).
fn apply_helper(name: &str, args: &[String]) -> Option<String> {
    match (name, args) {
        ("nodash", [x]) => Some(x.replace('-', "")),
        ("upper", [x]) => Some(x.to_uppercase()),
        ("lower", [x]) => Some(x.to_lowercase()),
        ("pad", [x, width]) => {
            let w: usize = width.parse().ok()?;
            Some(format!("{:0>width$}", x, width = w))
        }
        ("prevmonthyear", [m, y]) => {
            let month: i32 = m.parse().ok()?;
            let year: i32 = y.parse().ok()?;
            let (prev_month, prev_year) = if month == 1 {
                (12, year - 1)
            } else {
                (month - 1, year)
            };
            Some(format!("{:02}{}", prev_month, prev_year))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_for(text: &str) -> Option<String> {
        apply_rules(text, &compile(&RulesConfig::default()))
    }

    #[test]
    fn default_rules_all_compile() {
        let config = RulesConfig::default();
        let compiled = compile(&config);
        assert_eq!(
            compiled.len(),
            config.rules.len(),
            "wszystkie domyślne reguły powinny się skompilować"
        );
    }

    #[test]
    fn default_config_json_roundtrips() {
        // To, co zapiszemy do rules.json, musi dać się wczytać i zadziałać.
        let cfg = RulesConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: RulesConfig = serde_json::from_str(&json).unwrap();
        let compiled = compile(&parsed);
        assert_eq!(compiled.len(), cfg.rules.len());
        assert_eq!(
            apply_rules("OKR/25M09/SFP/PIT-5", &compiled),
            Some("PIT5-0925.pdf".to_string())
        );
    }

    #[test]
    fn matches_pit5_tax_form() {
        assert_eq!(
            name_for("blah blah OKR/ 25M09/SFP/PIT-5 blah"),
            Some("PIT5-0925.pdf".to_string())
        );
    }

    #[test]
    fn matches_vat7_tax_form() {
        assert_eq!(name_for("OKR/25M09/SFP/VAT-7"), Some("VAT7-0925.pdf".to_string()));
    }

    #[test]
    fn zus_uses_previous_month_and_correct_year() {
        let text = "DANE ODBIORCY Zakład Ubezpieczeń Społecznych foo DATA OPERACJI 15-09-2025";
        assert_eq!(name_for(text), Some("ZUS-082025.pdf".to_string()));
    }

    #[test]
    fn zus_matches_when_fields_span_multiple_lines() {
        let text = "DANE ODBIORCY\nZakład Ubezpieczeń Społecznych\nul. Szamocka 3, 5\n01-748 Warszawa\nTYTUŁ\nskładka\nDATA OPERACJI 15-09-2025";
        assert_eq!(name_for(text), Some("ZUS-082025.pdf".to_string()));
    }

    #[test]
    fn zus_january_rolls_back_to_previous_december() {
        let text = "DANE ODBIORCY Zakład Ubezpieczeń Społecznych foo DATA OPERACJI 10-01-2025";
        assert_eq!(name_for(text), Some("ZUS-122024.pdf".to_string()));
    }

    #[test]
    fn returns_none_when_no_pattern_matches() {
        assert_eq!(name_for("jakiś losowy tekst"), None);
    }

    #[test]
    fn custom_rule_with_plain_substitution() {
        // Reguła użytkownika bez funkcji pomocniczych — czyste podstawienie.
        let config = RulesConfig {
            rules: vec![Rule {
                name: "PIT-4 zaliczka".to_string(),
                pattern: r"PIT-4.*?(?P<month>\d{2})/(?P<year>\d{4})".to_string(),
                template: "PIT4-{month}{year}.pdf".to_string(),
            }],
        };
        let compiled = compile(&config);
        assert_eq!(
            apply_rules("przelew PIT-4 za okres 03/2025", &compiled),
            Some("PIT4-032025.pdf".to_string())
        );
    }

    #[test]
    fn invalid_rule_is_skipped_not_fatal() {
        let config = RulesConfig {
            rules: vec![
                Rule {
                    name: "zła regex".to_string(),
                    pattern: r"(niezamknieta".to_string(),
                    template: "x.pdf".to_string(),
                },
                Rule {
                    name: "dobra".to_string(),
                    pattern: r"(?P<n>FOO)".to_string(),
                    template: "{n}.pdf".to_string(),
                },
            ],
        };
        let compiled = compile(&config);
        assert_eq!(compiled.len(), 1, "błędna reguła powinna zostać pominięta");
        assert_eq!(apply_rules("FOO", &compiled), Some("FOO.pdf".to_string()));
    }

    #[test]
    fn template_helpers_upper_lower() {
        let config = RulesConfig {
            rules: vec![Rule {
                name: "test".to_string(),
                pattern: r"(?P<x>abc)".to_string(),
                template: "{upper(x)}-{lower(x)}.pdf".to_string(),
            }],
        };
        let compiled = compile(&config);
        assert_eq!(apply_rules("abc", &compiled), Some("ABC-abc.pdf".to_string()));
    }

    fn one_rule(pattern: &str, template: &str) -> Vec<CompiledRule> {
        compile(&RulesConfig {
            rules: vec![Rule {
                name: "test".to_string(),
                pattern: pattern.to_string(),
                template: template.to_string(),
            }],
        })
    }

    #[test]
    fn nested_function_calls() {
        // {upper(nodash(form))}: pit-5 -> nodash -> pit5 -> upper -> PIT5
        let compiled = one_rule(r"(?P<form>pit-5)", "{upper(nodash(form))}.pdf");
        assert_eq!(apply_rules("pit-5", &compiled), Some("PIT5.pdf".to_string()));
    }

    #[test]
    fn string_literal_argument() {
        // pad(month, "4"): "9" -> "0009"
        let compiled = one_rule(r"(?P<month>9)", r#"X{pad(month, "4")}.pdf"#);
        assert_eq!(apply_rules("9", &compiled), Some("X0009.pdf".to_string()));
    }

    #[test]
    fn brace_escaping() {
        let compiled = one_rule(r"(?P<n>FOO)", "{{kopia}}-{n}.pdf");
        assert_eq!(apply_rules("FOO", &compiled), Some("{kopia}-FOO.pdf".to_string()));
    }

    #[test]
    fn literal_parentheses_pass_through() {
        // Nawiasy poza {...} są dosłowne.
        let compiled = one_rule(r"(?P<n>FOO)", "{n} (kopia).pdf");
        assert_eq!(apply_rules("FOO", &compiled), Some("FOO (kopia).pdf".to_string()));
    }

    #[test]
    fn unknown_capture_rejected_at_compile() {
        // Wcześniej cicho zawodziło przy renderowaniu; teraz reguła jest odrzucana.
        let compiled = one_rule(r"(?P<n>FOO)", "{nieznana}.pdf");
        assert_eq!(compiled.len(), 0);
    }

    #[test]
    fn unknown_helper_rejected_at_compile() {
        let compiled = one_rule(r"(?P<n>FOO)", "{frobnicate(n)}.pdf");
        assert_eq!(compiled.len(), 0);
    }

    #[test]
    fn wrong_arity_rejected_at_compile() {
        let compiled = one_rule(r"(?P<n>FOO)", "{nodash(n, n)}.pdf");
        assert_eq!(compiled.len(), 0);
    }

    #[test]
    fn unbalanced_braces_rejected_at_compile() {
        let compiled = one_rule(r"(?P<n>FOO)", "{n.pdf");
        assert_eq!(compiled.len(), 0);
    }
}
