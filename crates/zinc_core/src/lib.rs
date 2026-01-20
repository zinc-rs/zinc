// PLAN: 1. Write unit tests -> 2. Define parser -> 3. Implement statement traversal -> 4. Implement transpile rules
// Library choice: pest provides PEG parsing that maps cleanly to a compact language grammar with clear precedence.

use pest::iterators::Pair;
use pest::error::LineColLocation;
use pest::Parser;
use pest_derive::Parser;
use serde::Serialize;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ZincParser;

#[derive(Serialize)]
pub struct ZincError {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub suggestion: String,
}

#[cfg(test)]
mod tests {
    use super::transpile;

    #[test]
    fn transpile_print_to_println() {
        let input = "print(\"x\")";
        let output = transpile(input);
        assert_eq!(output, "println!(\"{}\", \"x\");");
    }

    #[test]
    fn transpile_spider_get_default_profile() {
        let input = "spider.get(url)";
        let output = transpile(input);
        assert_eq!(output, "zinc_std::spider::get(url, None);");
    }

    #[test]
    fn transpile_spider_get_with_profile() {
        let input = "spider.get(url, profile)";
        let output = transpile(input);
        assert_eq!(output, "zinc_std::spider::get(url, Some(profile));");
    }
}

pub fn transpile(source: &str) -> String {
    match transpile_with_error(source) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("Parse failed: {}", err.message);
            String::new()
        }
    }
}

pub fn transpile_with_error(source: &str) -> Result<String, ZincError> {
    let mut output = String::new();
    let mut src = source;
    if src.starts_with('\u{feff}') {
        src = &src[3..];
    }

    let mut pairs = ZincParser::parse(Rule::program, src).map_err(zinc_error_from_pest)?;

    let program = pairs.next().ok_or_else(|| ZincError {
        line: 0,
        column: 0,
        message: "No statements found".to_string(),
        suggestion: "Add at least one statement.".to_string(),
    })?;

    let mut saw_statement = false;
    for pair in program.into_inner() {
        if pair.as_rule() == Rule::statement {
            saw_statement = true;
            let stmt_out = transpile_statement(pair);
            output.push_str(&stmt_out);
        }
    }

    if !saw_statement {
        return Err(ZincError {
            line: 0,
            column: 0,
            message: "No statements found".to_string(),
            suggestion: "Add at least one statement.".to_string(),
        });
    }

    Ok(output)
}

pub fn format_error_json(err: &str) -> String {
    let data = ZincError {
        line: 0,
        column: 0,
        message: err.to_string(),
        suggestion: "Check syntax near the reported location.".to_string(),
    };
    serde_json::to_string(&data).unwrap_or_else(|_| "{\"message\":\"error\"}".to_string())
}

fn zinc_error_from_pest(err: pest::error::Error<Rule>) -> ZincError {
    let (line, column) = match err.line_col {
        LineColLocation::Pos((l, c)) => (l, c),
        LineColLocation::Span((l, c), _) => (l, c),
    };
    ZincError {
        line,
        column,
        message: err.to_string(),
        suggestion: "Check syntax near the reported location.".to_string(),
    }
}

fn transpile_statement(pair: Pair<Rule>) -> String {
    let inner = pair.into_inner().next();
    if let Some(inner_pair) = inner {
        match inner_pair.as_rule() {
            Rule::expr_stmt => transpile_expr_stmt(inner_pair),
            Rule::let_stmt => transpile_let_stmt(inner_pair),
            Rule::if_stmt => transpile_if_stmt(inner_pair),
            Rule::loop_stmt => transpile_loop_stmt(inner_pair),
            Rule::break_stmt => transpile_break_stmt(inner_pair),
            Rule::fn_def => transpile_fn_def(inner_pair),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

fn transpile_fn_def(pair: Pair<Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::block {
            return transpile_block(inner);
        }
    }
    String::new()
}

fn transpile_let_stmt(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let expr = inner
        .next()
        .map(transpile_expr)
        .unwrap_or_default();

    if name.is_empty() || expr.is_empty() {
        String::new()
    } else {
        format!("let {} = {};", name, expr)
    }
}

fn transpile_expr_stmt(pair: Pair<Rule>) -> String {
    let expr_pair = pair.into_inner().next();
    if let Some(expr_pair) = expr_pair {
        let expr_out = transpile_expr(expr_pair);
        if expr_out.is_empty() {
            String::new()
        } else {
            format!("{};", expr_out)
        }
    } else {
        String::new()
    }
}

fn transpile_if_stmt(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let condition = inner
        .next()
        .map(transpile_expr)
        .unwrap_or_default();
    let then_block = inner
        .next()
        .map(transpile_block)
        .unwrap_or_default();
    let else_block = inner
        .next()
        .map(transpile_block)
        .unwrap_or_default();

    if condition.is_empty() || then_block.is_empty() {
        return String::new();
    }

    if else_block.is_empty() {
        format!("if {} {{\n{}}}", condition, then_block)
    } else {
        format!("if {} {{\n{}}} else {{\n{}}}", condition, then_block, else_block)
    }
}

fn transpile_loop_stmt(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let body = inner.next().map(transpile_block).unwrap_or_default();
    if body.is_empty() {
        String::new()
    } else {
        format!("loop {{\n{}}}", body)
    }
}

fn transpile_break_stmt(_pair: Pair<Rule>) -> String {
    "break;".to_string()
}

fn transpile_expr(pair: Pair<Rule>) -> String {
    match pair.as_rule() {
        Rule::expr => {
            let mut inner = pair.into_inner();
            let mut current = match inner.next() {
                Some(p) => transpile_expr(p),
                None => return String::new(),
            };
            while let Some(op) = inner.next() {
                let rhs_pair = match inner.next() {
                    Some(p) => p,
                    None => break,
                };
                match op.as_str() {
                    "|>" => {
                        current = transpile_pipeline(current, rhs_pair);
                    }
                    "+" => {
                        let rhs = transpile_expr(rhs_pair);
                        current = format!("format!(\"{{}}{{}}\", {}, {})", current, rhs);
                    }
                    "==" | "!=" | ">" | "<" | ">=" | "<=" => {
                        let rhs = transpile_expr(rhs_pair);
                        current = format!("({} {} {})", current, op.as_str(), rhs);
                    }
                    _ => {}
                }
            }
            current
        }
        Rule::term => transpile_term(pair),
        Rule::call => transpile_call(pair),
        Rule::array => transpile_array(pair),
        Rule::string => {
            transpile_string(pair.as_str())
        }
        Rule::number => pair.as_str().to_string(),
        Rule::identifier => pair.as_str().to_string(),
        _ => String::new(),
    }
}

fn transpile_call(pair: Pair<Rule>) -> String {
    let (name, args) = parse_call(pair);
    transpile_call_with_args(&name, &args)
}


fn transpile_arg_list(pair: Pair<Rule>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in pair.into_inner() {
        let value = transpile_expr(arg);
        if !value.is_empty() {
            out.push(value);
        }
    }
    out
}

fn transpile_block(pair: Pair<Rule>) -> String {
    let mut out = String::new();
    for stmt in pair.into_inner() {
        if stmt.as_rule() == Rule::statement {
            out.push_str(&transpile_statement(stmt));
        }
    }
    out
}


fn transpile_array(pair: Pair<Rule>) -> String {
    let mut items = Vec::new();
    let mut inner = pair.into_inner();
    if let Some(elements) = inner.next() {
        for expr in elements.into_inner() {
            if expr.as_rule() == Rule::expr {
                let value = transpile_expr(expr);
                if !value.is_empty() {
                    items.push(value);
                }
            }
        }
    }
    format!("vec![{}]", items.join(", "))
}

fn transpile_pipeline(lhs: String, rhs_pair: Pair<Rule>) -> String {
    if rhs_pair.as_rule() != Rule::term {
        return format!("{}({})", transpile_expr(rhs_pair), lhs);
    }

    let mut inner = rhs_pair.into_inner();
    let mut atom = match inner.next() {
        Some(p) => p,
        None => return String::new(),
    };

    if atom.as_rule() == Rule::atom {
        if let Some(inner_atom) = atom.into_inner().next() {
            atom = inner_atom;
        } else {
            return String::new();
        }
    }

    match atom.as_rule() {
        Rule::call => {
            let (name, mut args) = parse_call(atom);
            args.insert(0, lhs);
            let mut out = transpile_call_with_args(&name, &args);
            for suffix in inner {
                out = transpile_suffix(out, suffix);
            }
            out
        }
        Rule::identifier => {
            let ident = atom.as_str().to_string();
            if let Some(first_suffix) = inner.next() {
                let first_suffix = unwrap_suffix(first_suffix);
                if first_suffix.as_rule() == Rule::member_suffix {
                    let mut suffix_inner = first_suffix.into_inner();
                    let method = suffix_inner
                        .next()
                        .map(|p| p.as_str().to_string())
                        .unwrap_or_default();
                    let mut args = suffix_inner
                        .next()
                        .map(transpile_arg_list)
                        .unwrap_or_default();
                    args.insert(0, lhs);
                    let mut out = transpile_member_call_with_args(&ident, &method, &args);
                    for suffix in inner {
                        out = transpile_suffix(out, suffix);
                    }
                    return out;
                }
                let mut out = ident;
                out = transpile_suffix(out, first_suffix);
                for suffix in inner {
                    out = transpile_suffix(out, suffix);
                }
                return format!("{}({})", out, lhs);
            }
            return transpile_call_with_args(&ident, &[lhs]);
        }
        _ => {
            let mut out = transpile_atom(atom);
            for suffix in inner {
                out = transpile_suffix(out, suffix);
            }
            format!("{}({})", out, lhs)
        }
    }
}

fn parse_call(pair: Pair<Rule>) -> (String, Vec<String>) {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let args = inner.next().map(transpile_arg_list).unwrap_or_default();
    (name, args)
}


fn transpile_call_with_args(name: &str, args: &[String]) -> String {
    let args_joined = args.join(", ");
    match name {
        "print" => format!("println!(\"{{:?}}\", {})", args_joined),
        "leak" => "zinc_std::leak()".to_string(),
        _ => format!("{}({})", name, args_joined),
    }
}

fn transpile_member_call_with_args(obj: &str, method: &str, args: &[String]) -> String {
    let args_joined = args.join(", ");
    if obj == "db" && method == "query" {
        if args.len() == 2 {
            return format!("zinc_std::db::query({}, {})", args[0], args[1]);
        }
        return String::new();
    }
    if obj == "fs" && method == "read" {
        if args.len() == 1 {
            return format!("zinc_std::fs::read({})", args[0]);
        }
        return String::new();
    }
    if obj == "fs" && method == "write" {
        if args.len() == 2 {
            return format!("zinc_std::fs::write({}, {})", args[0], args[1]);
        }
        return String::new();
    }
    if obj == "html" && method == "select" {
        if args.len() == 2 {
            return format!("zinc_std::html::select_text({}, {})", args[0], args[1]);
        }
        return String::new();
    }
    if obj == "json" && method == "parse" {
        if args.len() == 1 {
            return format!("zinc_std::json::parse({})", args[0]);
        }
        return String::new();
    }
    if obj == "json" && method == "get" {
        if args.len() == 2 {
            return format!("zinc_std::json::get(&{}, {})", args[0], args[1]);
        }
        return String::new();
    }
    if obj == "json" && method == "at" {
        if args.len() == 2 {
            return format!("zinc_std::json::at(&{}, {})", args[0], args[1]);
        }
        return String::new();
    }
    if obj == "json" && method == "to_string" {
        if args.len() == 1 {
            return format!("zinc_std::json::to_string({})", args[0]);
        }
        return String::new();
    }
    if obj == "spider" && method == "get_proxy" {
        if args.len() == 3 {
            return format!(
                "zinc_std::spider::get_with_proxy({}, {}, {})",
                args[0], args[1], args[2]
            );
        }
        return String::new();
    }
    if obj == "py" && method == "eval" {
        return format!("zinc_std::python::eval({})", args_joined);
    }
    if obj == "spider" && method == "get" {
        if args.len() == 1 {
            format!("zinc_std::spider::get({}, None)", args[0])
        } else if args.len() >= 2 {
            format!("zinc_std::spider::get({}, Some({}))", args[0], args[1])
        } else {
            String::new()
        }
    } else {
        format!("{}.{}({})", obj, method, args_joined)
    }
}

fn transpile_term(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let atom = match inner.next() {
        Some(p) => p,
        None => return String::new(),
    };
    let mut current = transpile_atom(atom);
    for suffix in inner {
        current = transpile_suffix(current, suffix);
    }
    current
}

fn transpile_atom(pair: Pair<Rule>) -> String {
    match pair.as_rule() {
        Rule::atom => {
            let mut inner = pair.into_inner();
            if let Some(p) = inner.next() {
                transpile_atom(p)
            } else {
                String::new()
            }
        }
        Rule::array => transpile_array(pair),
        Rule::call => transpile_call(pair),
        Rule::string => {
            transpile_string(pair.as_str())
        }
        Rule::number => pair.as_str().to_string(),
        Rule::identifier => pair.as_str().to_string(),
        Rule::expr => transpile_expr(pair),
        Rule::term => transpile_term(pair),
        _ => String::new(),
    }
}

fn transpile_suffix(current: String, suffix: Pair<Rule>) -> String {
    let suffix = unwrap_suffix(suffix);
    match suffix.as_rule() {
        Rule::indexing_suffix => {
            let mut inner = suffix.into_inner();
            let index_expr = inner.next().map(transpile_expr).unwrap_or_default();
            if current.is_empty() || index_expr.is_empty() {
                String::new()
            } else {
                format!("{}[{} as usize]", current, index_expr)
            }
        }
        Rule::member_suffix => {
            let mut inner = suffix.into_inner();
            let method = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let args = inner.next().map(transpile_arg_list).unwrap_or_default();
            if method.is_empty() {
                return String::new();
            }
            if is_simple_identifier(&current) {
                return transpile_member_call_with_args(&current, &method, &args);
            }
            format!("{}.{}({})", current, method, args.join(", "))
        }
        _ => current,
    }
}

fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn unwrap_suffix(pair: Pair<Rule>) -> Pair<Rule> {
    if pair.as_rule() == Rule::suffix {
        return pair.into_inner().next().unwrap();
    }
    pair
}

fn transpile_string(raw: &str) -> String {
    if raw.len() < 2 {
        return String::new();
    }
    let inner = &raw[1..raw.len() - 1];
    let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
    format!("r#\"{}\"#", unescaped)
}


