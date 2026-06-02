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
//! W szablonie `{nazwa}` jest podstawiane przez nazwaną grupę przechwytującą
//! z wyrażenia regularnego (`(?P<nazwa>...)`). Można też wywołać funkcję
//! pomocniczą: `{funkcja(arg1, arg2)}`, gdzie argumentami są nazwy grup.
//!
//! Dostępne funkcje pomocnicze:
//! - `nodash(x)`          — usuwa myślniki (np. `PIT-5` → `PIT5`)
//! - `upper(x)` / `lower(x)` — zmiana wielkości liter
//! - `prevmonthyear(m, y)` — z miesiąca i roku tworzy `MMYYYY` poprzedniego
//!   miesiąca (z przeniesieniem roku w styczniu); używane dla ZUS.

use serde::{Deserialize, Serialize};
use regex::{Regex, Captures};
use log::{info, warn, error};
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
    Literal(String),
    Capture(String),
    Func(String, Vec<String>),
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

// --- Silnik szablonów ---------------------------------------------------

// `while let` (nie `for`) jest tu konieczne: zagnieżdżone pętle konsumują
// ten sam iterator, więc nie można go przenieść do pętli `for`.
#[allow(clippy::while_let_on_iterator)]
fn parse_template(template: &str) -> Result<Vec<TemplatePart>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if !literal.is_empty() {
                    parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                }
                let mut expr = String::new();
                let mut closed = false;
                while let Some(nc) = chars.next() {
                    if nc == '}' {
                        closed = true;
                        break;
                    }
                    expr.push(nc);
                }
                if !closed {
                    return Err(format!("niezamknięty '{{' w szablonie: {}", template));
                }
                parts.push(parse_expr(expr.trim())?);
            }
            '}' => return Err(format!("nieoczekiwany '}}' w szablonie: {}", template)),
            _ => literal.push(c),
        }
    }

    if !literal.is_empty() {
        parts.push(TemplatePart::Literal(literal));
    }
    Ok(parts)
}

fn parse_expr(expr: &str) -> Result<TemplatePart, String> {
    if let Some(open) = expr.find('(') {
        if !expr.ends_with(')') {
            return Err(format!("niepoprawne wywołanie funkcji: {}", expr));
        }
        let name = expr[..open].trim().to_string();
        let args = expr[open + 1..expr.len() - 1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(TemplatePart::Func(name, args))
    } else if expr.is_empty() {
        Err("pusty wyrażenie {} w szablonie".to_string())
    } else {
        Ok(TemplatePart::Capture(expr.to_string()))
    }
}

fn render(parts: &[TemplatePart], caps: &Captures) -> Option<String> {
    let mut out = String::new();
    for part in parts {
        match part {
            TemplatePart::Literal(s) => out.push_str(s),
            TemplatePart::Capture(name) => out.push_str(caps.name(name)?.as_str()),
            TemplatePart::Func(name, args) => {
                let vals: Option<Vec<&str>> =
                    args.iter().map(|a| caps.name(a).map(|m| m.as_str())).collect();
                let result = apply_helper(name, &vals?)?;
                out.push_str(&result);
            }
        }
    }
    Some(out)
}

/// Rejestr funkcji pomocniczych szablonu. Tu dodaje się nowe transformacje.
fn apply_helper(name: &str, args: &[&str]) -> Option<String> {
    match (name, args) {
        ("nodash", [x]) => Some(x.replace('-', "")),
        ("upper", [x]) => Some(x.to_uppercase()),
        ("lower", [x]) => Some(x.to_lowercase()),
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
}
