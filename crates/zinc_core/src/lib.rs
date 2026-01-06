// PLAN: 1. Write unit tests -> 2. Define parser -> 3. Implement statement traversal -> 4. Implement transpile rules
// Library choice: pest provides PEG parsing that maps cleanly to a compact language grammar with clear precedence.

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ZincParser;

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
    let mut output = String::new();
    let mut src = source;
    if src.starts_with('\u{feff}') {
        src = &src[3..];
    }

    let mut pairs = match ZincParser::parse(Rule::program, src) {
        Ok(pairs) => pairs,
        Err(err) => {
            eprintln!("Parse failed: {}", err);
            return output;
        }
    };

    let program = match pairs.next() {
        Some(pair) => pair,
        None => {
            eprintln!("Warning: No statements found");
            return output;
        }
    };

    let mut saw_statement = false;
    for pair in program.into_inner() {
        if pair.as_rule() == Rule::statement {
            saw_statement = true;
            let stmt_out = transpile_statement(pair);
            output.push_str(&stmt_out);
        }
    }

    if !saw_statement {
        eprintln!("Warning: No statements found");
    }

    output
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
        Rule::expr => pair
            .into_inner()
            .next()
            .map(transpile_expr)
            .unwrap_or_default(),
        Rule::call => transpile_call(pair),
        Rule::member_call => transpile_member_call(pair),
        Rule::string => pair.as_str().to_string(),
        Rule::identifier => pair.as_str().to_string(),
        _ => String::new(),
    }
}

fn transpile_call(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let args = inner.next().map(transpile_arg_list).unwrap_or_default();
    let args_joined = args.join(", ");

    if name == "print" {
        format!("println!(\"{}\", {})", "{}", args_joined)
    } else {
        format!("{}({})", name, args_joined)
    }
}

fn transpile_member_call(pair: Pair<Rule>) -> String {
    let mut inner = pair.into_inner();
    let obj = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let method = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let args = inner.next().map(transpile_arg_list).unwrap_or_default();
    let args_joined = args.join(", ");

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


