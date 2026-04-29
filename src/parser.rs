use crate::commands::CommandFuture;
use crate::commands::{get_command_list, run_cmd};
use crate::programReturn::{ProcessError, Success};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::fmt;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::future::Future;
use core::num::ParseFloatError;
use core::pin::Pin;

#[derive(Clone, Debug)]
enum RispErr {
    Reason(String),
}

#[derive(Clone)]
struct RispEnv {
    data: BTreeMap<String, RispExp>,
}

fn tokenize(expr: String) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();

    let mut in_string = false;
    let mut in_comment = false;

    for c in expr.chars() {
        if in_comment {
            if c == '#' {
                in_comment = false;
            }
            continue;
        }

        if in_string {
            current_token.push(c);
            if c == '"' {
                in_string = false; // Exit string mode
                tokens.push(current_token.clone());
                current_token.clear();
            }
            continue;
        }

        match c {
            '#' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                in_comment = true;
            }
            '"' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                in_string = true;
                current_token.push(c);
            }
            '(' | ')' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(c.to_string());
            }
            _ if c.is_whitespace() => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            }
            _ => {
                current_token.push(c);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    tokens
}

fn parse<'a>(tokens: &'a [String]) -> Result<(RispExp, &'a [String]), RispErr> {
    let (token, rest) = tokens
        .split_first()
        .ok_or(RispErr::Reason("could not get token".to_string()))?;
    match &token[..] {
        "(" => read_seq(rest),
        ")" => Err(RispErr::Reason("unexpected `)`".to_string())),
        _ => Ok((parse_atom(token.as_str()), rest)),
    }
}

fn read_seq<'a>(tokens: &'a [String]) -> Result<(RispExp, &'a [String]), RispErr> {
    let mut res: Vec<RispExp> = vec![];
    let mut xs = tokens;
    loop {
        let (next_token, rest) = xs
            .split_first()
            .ok_or(RispErr::Reason("could not find closing `)`".to_string()))?;
        if next_token == ")" {
            return Ok((RispExp::List(res), rest));
        }
        let (exp, new_xs) = parse(&xs)?;
        res.push(exp);
        xs = new_xs;
    }
}

fn parse_atom(token: &str) -> RispExp {
    match token {
        "true" => RispExp::Bool(true),
        "false" => RispExp::Bool(false),
        _ if token.starts_with('"') && token.ends_with('"') => {
            let string_val = token[1..token.len() - 1].to_string();
            RispExp::String(string_val)
        }
        _ => {
            let potential_float: Result<f64, ParseFloatError> = token.parse();
            match potential_float {
                Ok(v) => RispExp::Number(v),
                Err(_) => RispExp::Symbol(token.to_string()),
            }
        }
    }
}

#[derive(Clone)]
enum RispExp {
    Bool(bool),
    Symbol(String),
    Number(f64),
    String(String),
    List(Vec<RispExp>),
    Map(BTreeMap<RispKey, RispExp>),
    Func(fn(&[RispExp]) -> Result<RispExp, RispErr>),
    Lambda(RispLambda),
    Syscall(Vec<String>),
}

#[derive(Clone)]
struct RispLambda {
    params_exp: Arc<RispExp>,
    body_exp: Arc<RispExp>,
    captured_env: RispEnv,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum RispKey {
    String(String),
    Number(i64), // Safe for ordering!
    Bool(bool),
}

impl fmt::Display for RispKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RispKey::String(s) => write!(f, "\"{}\"", s), // Keep quotes for map printing
            RispKey::Number(n) => write!(f, "{}", n),
            RispKey::Bool(b) => write!(f, "{}", b),
        }
    }
}

fn exp_to_key(exp: &RispExp) -> Result<RispKey, RispErr> {
    match exp {
        RispExp::String(s) => Ok(RispKey::String(s.clone())),
        RispExp::Number(n) => Ok(RispKey::Number(*n as i64)), // Cast float to 64-bit int
        RispExp::Bool(b) => Ok(RispKey::Bool(*b)),
        _ => Err(RispErr::Reason(
            "only strings, numbers, and bools can be map keys".to_string(),
        )),
    }
}

fn key_to_exp(key: &RispKey) -> RispExp {
    match key {
        RispKey::String(s) => RispExp::String(s.clone()),
        RispKey::Number(n) => RispExp::Number(*n as f64), // Cast back to float
        RispKey::Bool(b) => RispExp::Bool(*b),
    }
}

macro_rules! ensure_tonicity {
    ($check_fn:expr) => {{
        |args: &[RispExp]| -> Result<RispExp, RispErr> {
            let floats = parse_list_of_floats(args)?;
            let first = floats
                .first()
                .ok_or(RispErr::Reason("expected at least one number".to_string()))?;
            let rest = &floats[1..];
            fn f(prev: &f64, xs: &[f64]) -> bool {
                match xs.first() {
                    Some(x) => $check_fn(prev, x) && f(x, &xs[1..]),
                    None => true,
                }
            }
            Ok(RispExp::Bool(f(first, rest)))
        }
    }};
}

fn default_env() -> RispEnv {
    let mut data: BTreeMap<String, RispExp> = BTreeMap::new();

    data.insert(
        "+".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let sum = parse_list_of_floats(args)?
                .iter()
                .fold(0.0, |sum, a| sum + a);
            Ok(RispExp::Number(sum))
        }),
    );
    data.insert(
        "-".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let floats = parse_list_of_floats(args)?;
            let first = *floats
                .first()
                .ok_or(RispErr::Reason("expected at least one number".to_string()))?;
            let sum_of_rest = floats[1..].iter().fold(0.0, |sum, a| sum + a);
            Ok(RispExp::Number(first - sum_of_rest))
        }),
    );

    data.insert(
        "=".to_string(),
        RispExp::Func(ensure_tonicity!(|a, b| a == b)),
    );
    data.insert(
        ">".to_string(),
        RispExp::Func(ensure_tonicity!(|a, b| a > b)),
    );
    data.insert(
        ">=".to_string(),
        RispExp::Func(ensure_tonicity!(|a, b| a >= b)),
    );
    data.insert(
        "<".to_string(),
        RispExp::Func(ensure_tonicity!(|a, b| a < b)),
    );
    data.insert(
        "<=".to_string(),
        RispExp::Func(ensure_tonicity!(|a, b| a <= b)),
    );

    data.insert(
        "*".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let prod = parse_list_of_floats(args)?
                .iter()
                .fold(1.0, |prod, a| prod * a);
            Ok(RispExp::Number(prod))
        }),
    );

    data.insert(
        "/".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let floats = parse_list_of_floats(args)?;
            let first = *floats
                .first()
                .ok_or(RispErr::Reason("expected at least one number".to_string()))?;
            let result = floats[1..].iter().fold(first, |div, a| (div / a));
            Ok(RispExp::Number(result))
        }),
    );

    data.insert(
        "&".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let floats = parse_list_of_floats(args)?;
            let first = *floats
                .first()
                .ok_or(RispErr::Reason("expected at least one number".to_string()))?
                as i64;
            let result = floats[1..].iter().fold(first, |acc, a| acc & (*a as i64));
            Ok(RispExp::Number(result as f64))
        }),
    );

    data.insert(
        "list".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            Ok(RispExp::List(args.to_vec()))
        }),
    );

    data.insert("[]".to_string(), RispExp::List(vec![]));

    data.insert(
        "len".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let arg = args
                .first()
                .ok_or(RispErr::Reason("len requires 1 argument".to_string()))?;
            match arg {
                RispExp::List(l) => Ok(RispExp::Number(l.len() as f64)),
                RispExp::String(s) => Ok(RispExp::Number(s.len() as f64)),
                _ => Err(RispErr::Reason(
                    "len only works on lists and strings".to_string(),
                )),
            }
        }),
    );

    data.insert(
        "!!".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            if args.len() != 2 {
                return Err(RispErr::Reason(
                    "!! requires exactly 2 arguments".to_string(),
                ));
            }

            let collection = &args[0];
            let index_exp = &args[1];

            match collection {
                RispExp::List(l) => {
                    let idx_f = parse_single_float(index_exp)?;
                    let mut idx = idx_f as i64;

                    // Handle negative indexing
                    if idx < 0 {
                        idx = (l.len() as i64) + idx;
                    }

                    if idx >= 0 && (idx as usize) < l.len() {
                        Ok(l[idx as usize].clone())
                    } else {
                        Err(RispErr::Reason("index out of bounds".to_string()))
                    }
                }
                RispExp::Map(m) => {
                    let key = exp_to_key(index_exp)?;

                    match m.get(&key) {
                        Some(val) => Ok(val.clone()),
                        None => Ok(RispExp::String("".to_string())),
                    }
                }
                _ => Err(RispErr::Reason(
                    "!! only works on lists, strings, and maps".to_string(),
                )),
            }
        }),
    );

    data.insert(
        "map".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            if args.len() % 2 != 0 {
                return Err(RispErr::Reason(
                    "map requires an even number of arguments".to_string(),
                ));
            }

            let mut m = alloc::collections::BTreeMap::new();
            let mut i = 0;
            while i < args.len() {
                let key = exp_to_key(&args[i])?; // Use the helper!
                m.insert(key, args[i + 1].clone());
                i += 2;
            }
            Ok(RispExp::Map(m))
        }),
    );

    data.insert(
        "mkeys".to_string(),
        RispExp::Func(|args: &[RispExp]| -> Result<RispExp, RispErr> {
            let arg = args
                .first()
                .ok_or(RispErr::Reason("mkeys requires 1 argument".to_string()))?;
            match arg {
                RispExp::Map(m) => {
                    // Convert RispKeys back to RispExps for the resulting List
                    let keys: Vec<RispExp> = m.keys().map(|k| key_to_exp(k)).collect();
                    Ok(RispExp::List(keys))
                }
                _ => Err(RispErr::Reason("mkeys only works on maps".to_string())),
            }
        }),
    );

    RispEnv { data }
}

fn parse_list_of_floats(args: &[RispExp]) -> Result<Vec<f64>, RispErr> {
    args.iter().map(|x| parse_single_float(x)).collect()
}

fn parse_single_float(exp: &RispExp) -> Result<f64, RispErr> {
    match exp {
        RispExp::Number(num) => Ok(*num),
        _ => Err(RispErr::Reason("expected a number".to_string())),
    }
}

fn env_get(k: &str, env: &RispEnv) -> Option<RispExp> {
    env.data.get(k).cloned()
}
fn eval(
    exp: RispExp,
    env: &mut RispEnv,
) -> Pin<Box<dyn Future<Output = Result<RispExp, RispErr>> + Send + '_>> {
    Box::pin(async move {
        match exp {
            RispExp::Bool(_) => Ok(exp),
            RispExp::Number(_) => Ok(exp),
            RispExp::Symbol(ref k) => {
                env_get(k, env).ok_or(RispErr::Reason(format!("unexpected symbol k='{}'", k)))
            }
            RispExp::List(list) => {
                if list.is_empty() {
                    return Err(RispErr::Reason("expected a non-empty list".to_string()));
                }

                let mut iter = list.into_iter();
                let first_form = iter.next().unwrap();
                let arg_forms: Vec<RispExp> = iter.collect();

                if let RispExp::Symbol(s) = &first_form {
                    match s.as_str() {
                        "if" => {
                            let test_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected test form".to_string()))?;

                            // Explicitly re-borrow env with &mut *env to prevent moves
                            let test_eval = eval(test_form.clone(), &mut *env).await?;
                            match test_eval {
                                RispExp::Bool(b) => {
                                    let form_idx = if b { 1 } else { 2 };
                                    let res_form = arg_forms.get(form_idx).ok_or(
                                        RispErr::Reason(format!("expected form idx={}", form_idx)),
                                    )?;
                                    return eval(res_form.clone(), &mut *env).await;
                                }
                                _ => {
                                    return Err(RispErr::Reason(format!(
                                        "unexpected test form='{}'",
                                        test_form
                                    )));
                                }
                            }
                        }
                        "def" => {
                            let first_str = match arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected first form".to_string()))?
                            {
                                RispExp::Symbol(s) => s.clone(),
                                _ => {
                                    return Err(RispErr::Reason(
                                        "expected first form to be a symbol".to_string(),
                                    ));
                                }
                            };
                            let second_form = arg_forms
                                .get(1)
                                .ok_or(RispErr::Reason("expected second form".to_string()))?;
                            if arg_forms.len() > 2 {
                                return Err(RispErr::Reason(
                                    "def can only have two forms ".to_string(),
                                ));
                            }
                            let second_eval = eval(second_form.clone(), &mut *env).await?;
                            env.data.insert(first_str, second_eval);
                            return Ok(arg_forms[0].clone());
                        }
                        "fn" => {
                            return eval_lambda_args(&arg_forms, env.clone());
                        }
                        "sys" => {
                            if arg_forms.is_empty() {
                                return Err(RispErr::Reason(
                                    "sys expects at least one argument".to_string(),
                                ));
                            }

                            let mut evaluated_args = Vec::new();
                            for arg in arg_forms {
                                evaluated_args.push(eval(arg, &mut *env).await?);
                            }

                            let cmd_parts: Result<Vec<String>, RispErr> = evaluated_args
                                .into_iter()
                                .map(|arg| match arg {
                                    RispExp::Symbol(s) => Ok(s),
                                    RispExp::Number(n) => Ok(n.to_string()),
                                    RispExp::Bool(b) => Ok(b.to_string()),
                                    RispExp::String(s) => Ok(s),
                                    RispExp::List(_) | RispExp::Map(_) => Ok(arg.to_string()),
                                    _ => Err(RispErr::Reason(
                                        "sys args must evaluate to symbols, numbers, bools, strings, lists, or maps".to_string(),
                                    )),
                                })
                                .collect();

                            let cmd_vec = cmd_parts?;

                            match crate::commands::run_cmd(cmd_vec.clone()) {
                                Ok(command_future) => match command_future.await {
                                    Ok(success) => {
                                        if success.print_code {
                                            crate::println!("{}", success.success_code);
                                        }
                                        return Ok(RispExp::Syscall(cmd_vec));
                                    }
                                    Err(e) => {
                                        return Err(RispErr::Reason(format!(
                                            "Syscall failed: {}",
                                            e.error_code
                                        )));
                                    }
                                },
                                Err(e) => {
                                    return Err(RispErr::Reason(format!(
                                        "Command lookup failed: {}",
                                        e.error_code
                                    )));
                                }
                            }
                        }

                        "quote" => {
                            let form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected form to quote".to_string()))?;
                            return Ok(form.clone());
                        }
                        "and" => {
                            for arg in arg_forms {
                                let res = eval(arg, &mut *env).await?;
                                match res {
                                    RispExp::Bool(b) => {
                                        if !b {
                                            return Ok(RispExp::Bool(false));
                                        }
                                    }
                                    _ => {
                                        return Err(RispErr::Reason(
                                            "and expects booleans".to_string(),
                                        ));
                                    }
                                }
                            }
                            return Ok(RispExp::Bool(true));
                        }
                        "or" => {
                            for arg in arg_forms {
                                let res = eval(arg, &mut *env).await?;
                                match res {
                                    RispExp::Bool(b) => {
                                        if b {
                                            return Ok(RispExp::Bool(true));
                                        }
                                    }
                                    _ => {
                                        return Err(RispErr::Reason(
                                            "or expects booleans".to_string(),
                                        ));
                                    }
                                }
                            }
                            return Ok(RispExp::Bool(false));
                        }
                        "for" => {
                            let condition = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected loop condition".to_string()))?;
                            let body = arg_forms
                                .get(1)
                                .ok_or(RispErr::Reason("expected loop body".to_string()))?;

                            loop {
                                let cond_res = eval(condition.clone(), &mut *env).await?;
                                match cond_res {
                                    RispExp::Bool(b) if b => {
                                        eval(body.clone(), &mut *env).await?;
                                    }
                                    RispExp::Bool(_) => break,
                                    _ => {
                                        return Err(RispErr::Reason(
                                            "loop condition must be boolean".to_string(),
                                        ));
                                    }
                                }
                            }
                            return Ok(RispExp::Bool(false));
                        }

                        "append" => {
                            let sym_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected symbol for append".to_string()))?;
                            let val_form = arg_forms
                                .get(1)
                                .ok_or(RispErr::Reason("expected value to append".to_string()))?;

                            let sym_name = match sym_form {
                                RispExp::Symbol(s) => s.clone(),
                                _ => {
                                    return Err(RispErr::Reason(
                                        "append requires a variable name (symbol)".to_string(),
                                    ));
                                }
                            };

                            let evaled_val = eval(val_form.clone(), &mut *env).await?;

                            let mut current_list = match env_get(&sym_name, env) {
                                Some(RispExp::List(l)) => l,
                                Some(_) => {
                                    return Err(RispErr::Reason(format!(
                                        "'{}' is not a list",
                                        sym_name
                                    )));
                                }
                                None => {
                                    return Err(RispErr::Reason(format!(
                                        "undefined variable '{}'",
                                        sym_name
                                    )));
                                }
                            };

                            current_list.push(evaled_val.clone());
                            env.data
                                .insert(sym_name.to_string(), RispExp::List(current_list.clone()));

                            return Ok(RispExp::List(current_list));
                        }

                        "pop" => {
                            let sym_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected symbol for pop".to_string()))?;

                            let sym_name = match sym_form {
                                RispExp::Symbol(s) => s.clone(),
                                _ => {
                                    return Err(RispErr::Reason(
                                        "pop requires a variable name (symbol)".to_string(),
                                    ));
                                }
                            };

                            let mut current_list = match env_get(&sym_name, env) {
                                Some(RispExp::List(l)) => l,
                                Some(_) => {
                                    return Err(RispErr::Reason(format!(
                                        "'{}' is not a list",
                                        sym_name
                                    )));
                                }
                                None => {
                                    return Err(RispErr::Reason(format!(
                                        "undefined variable '{}'",
                                        sym_name
                                    )));
                                }
                            };

                            let popped_val = current_list.pop().ok_or(RispErr::Reason(
                                "cannot pop from an empty list".to_string(),
                            ))?;

                            env.data
                                .insert(sym_name.to_string(), RispExp::List(current_list));

                            return Ok(popped_val);
                        }

                        "mset" => {
                            let sym_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected symbol for mset".to_string()))?;
                            let key_form = arg_forms
                                .get(1)
                                .ok_or(RispErr::Reason("expected key for mset".to_string()))?;
                            let val_form = arg_forms
                                .get(2)
                                .ok_or(RispErr::Reason("expected value for mset".to_string()))?;

                            let sym_name = match sym_form {
                                RispExp::Symbol(s) => s.clone(),
                                _ => {
                                    return Err(RispErr::Reason(
                                        "mset requires a variable name (symbol)".to_string(),
                                    ));
                                }
                            };

                            let evaled_key = eval(key_form.clone(), &mut *env).await?;
                            let key = exp_to_key(&evaled_key)?;

                            let evaled_val = eval(val_form.clone(), &mut *env).await?;

                            let mut current_map = match env_get(&sym_name, env) {
                                Some(RispExp::Map(m)) => m,
                                Some(_) => {
                                    return Err(RispErr::Reason(format!(
                                        "'{}' is not a map",
                                        sym_name
                                    )));
                                }
                                None => {
                                    return Err(RispErr::Reason(format!(
                                        "undefined variable '{}'",
                                        sym_name
                                    )));
                                }
                            };

                            current_map.insert(key, evaled_val.clone());
                            env.data.insert(sym_name, RispExp::Map(current_map));

                            return Ok(evaled_val);
                        }

                        "mdel" => {
                            let sym_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("expected symbol for mdel".to_string()))?;
                            let key_form = arg_forms
                                .get(1)
                                .ok_or(RispErr::Reason("expected key for mdel".to_string()))?;

                            let sym_name = match sym_form {
                                RispExp::Symbol(s) => s.clone(),
                                _ => {
                                    return Err(RispErr::Reason(
                                        "mdel requires a variable name (symbol)".to_string(),
                                    ));
                                }
                            };

                            let evaled_key = eval(key_form.clone(), &mut *env).await?;
                            let key = exp_to_key(&evaled_key)?;

                            let mut current_map = match env_get(&sym_name, env) {
                                Some(RispExp::Map(m)) => m,
                                Some(_) => {
                                    return Err(RispErr::Reason(format!(
                                        "'{}' is not a map",
                                        sym_name
                                    )));
                                }
                                None => {
                                    return Err(RispErr::Reason(format!(
                                        "undefined variable '{}'",
                                        sym_name
                                    )));
                                }
                            };

                            current_map.remove(&key);
                            env.data.insert(sym_name, RispExp::Map(current_map));

                            return Ok(RispExp::Bool(true));
                        }

                        "do" => {
                            if arg_forms.is_empty() {
                                return Ok(RispExp::List(vec![]));
                            }

                            let mut last_result = RispExp::List(vec![]);

                            for form in arg_forms {
                                last_result = eval(form, &mut *env).await?;
                            }

                            return Ok(last_result);
                        }

                        "error" => {
                            let msg_form = arg_forms
                                .get(0)
                                .ok_or(RispErr::Reason("error requires a message".to_string()))?;

                            let evaled_msg = eval(msg_form.clone(), &mut *env).await?;

                            if let Some(handler) = env_get("error", env) {
                                match handler {
                                    RispExp::Lambda(lambda) => {
                                        let mut merged_env = env.clone();
                                        for (k, v) in lambda.captured_env.data.iter() {
                                            merged_env.data.insert(k.clone(), v.clone());
                                        }

                                        if let Ok(mut new_env) = env_for_lambda(
                                            lambda.params_exp.clone(),
                                            &[evaled_msg.clone()],
                                            &mut merged_env,
                                        ) {
                                            let _ = eval((*lambda.body_exp).clone(), &mut new_env)
                                                .await;
                                        }
                                    }
                                    RispExp::Func(f) => {
                                        let _ = f(&[evaled_msg.clone()]);
                                    }
                                    _ => {}
                                }
                            }

                            let err_str = match evaled_msg {
                                RispExp::String(s) => s,
                                _ => evaled_msg.to_string(),
                            };

                            return Err(RispErr::Reason(err_str));
                        }

                        _ => {}
                    }
                }

                // Standard Functions & Lambdas
                let first_eval = eval(first_form, &mut *env).await?;
                match first_eval {
                    RispExp::Func(f) => {
                        let mut evaluated_args = Vec::new();
                        for arg in arg_forms {
                            evaluated_args.push(eval(arg, &mut *env).await?);
                        }
                        f(&evaluated_args)
                    }
                    RispExp::Lambda(lambda) => {
                        let mut evaluated_args = Vec::new();
                        for arg in arg_forms {
                            evaluated_args.push(eval(arg, &mut *env).await?);
                        }

                        let mut merged_env = env.clone();

                        for (k, v) in lambda.captured_env.data.iter() {
                            merged_env.data.insert(k.clone(), v.clone());
                        }

                        let mut new_env = env_for_lambda(
                            lambda.params_exp.clone(),
                            &evaluated_args,
                            &mut merged_env,
                        )?;

                        // Evaluate the body
                        eval((*lambda.body_exp).clone(), &mut new_env).await
                    }
                    _ => Err(RispErr::Reason("first form must be a function".to_string())),
                }
            }
            RispExp::Func(_) | RispExp::Lambda(_) | RispExp::Syscall(_) => {
                Err(RispErr::Reason("unexpected form".to_string()))
            }
            RispExp::String(_) => Ok(exp), // <- Added string base case
            RispExp::Map(_) => Ok(exp),
        }
    })
}

fn env_for_lambda(
    params: Arc<RispExp>,
    evaluated_args: &[RispExp],
    outer_env: &mut RispEnv,
) -> Result<RispEnv, RispErr> {
    let ks = parse_list_of_symbol_strings(params)?;
    if ks.len() != evaluated_args.len() {
        return Err(RispErr::Reason(format!(
            "expected {} arguments, got {}",
            ks.len(),
            evaluated_args.len()
        )));
    }

    let mut new_env = outer_env.clone();
    for (k, v) in ks.iter().zip(evaluated_args.iter()) {
        new_env.data.insert(k.clone(), v.clone());
    }
    Ok(new_env)
}

fn parse_list_of_symbol_strings(form: Arc<RispExp>) -> Result<Vec<String>, RispErr> {
    let list = match form.as_ref() {
        RispExp::List(s) => Ok(s.clone()),
        _ => Err(RispErr::Reason(
            "expected args form to be a list".to_string(),
        )),
    }?;
    list.iter()
        .map(|x| match x {
            RispExp::Symbol(s) => Ok(s.clone()),
            _ => Err(RispErr::Reason(
                "expected symbols in the argument list".to_string(),
            )),
        })
        .collect()
}

fn eval_lambda_args(arg_forms: &[RispExp], captured_env: RispEnv) -> Result<RispExp, RispErr> {
    let params_exp = arg_forms
        .first()
        .ok_or(RispErr::Reason("expected args form".to_string()))?;
    let body_exp = arg_forms
        .get(1)
        .ok_or(RispErr::Reason("expected second form".to_string()))?;
    if arg_forms.len() > 2 {
        return Err(RispErr::Reason(
            "fn definition can only have two forms ".to_string(),
        ));
    }

    Ok(RispExp::Lambda(RispLambda {
        body_exp: Arc::new(body_exp.clone()),
        params_exp: Arc::new(params_exp.clone()),
        captured_env,
    }))
}

impl fmt::Display for RispExp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = match self {
            RispExp::Bool(a) => a.to_string(),
            RispExp::Symbol(s) => (*s).clone(),
            RispExp::Number(n) => n.to_string(),
            RispExp::String(s) => s.clone(), // Print the raw string without quotes for sys echo
            RispExp::List(list) => {
                let xs: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                format!("({})", xs.join(" ")) // Space-separated Lisp style: (1 2 3)
            }
            RispExp::Map(map) => {
                let xs: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                format!("{{{}}}", xs.join(", "))
            }
            RispExp::Func(_) => "<Function>".to_string(),
            RispExp::Lambda(_) => "<Lambda>".to_string(),
            RispExp::Syscall(cmd) => format!("<Syscall: [{}]>", cmd.join(", ")),
        };
        write!(f, "{}", str)
    }
}

async fn parse_eval(expr: String, env: &mut RispEnv) -> Result<RispExp, RispErr> {
    let (parsed_exp, _) = parse(&tokenize(expr))?;
    // Passed by value!
    eval(parsed_exp, env).await
}

pub fn interpret(expr: Vec<String>) -> CommandFuture {
    Box::pin(async move {
        if expr.len() < 2 {
            return Err(ProcessError {
                error_code: "expected at least two params".to_string(),
            });
        }

        let mut env = default_env();

        let syscalls = get_command_list();
        for call in syscalls {
            env.data.insert(call.clone(), RispExp::Symbol(call));
        }

        for i in 0..2 {
            env.data.insert(format!("n{}", i), RispExp::Number(0.0));
            env.data.insert(format!("b{}", i), RispExp::Bool(false));
        }

        let mut b_count = 0;
        let mut n_count = 0;

        for arg_str in expr.iter().skip(2) {
            if arg_str == "true" || arg_str == "false" {
                let val = arg_str == "true";
                let key = format!("b{}", b_count);
                env.data.insert(key, RispExp::Bool(val));
                b_count += 1;
            } else if let Ok(val) = arg_str.parse::<f64>() {
                let key = format!("n{}", n_count);
                env.data.insert(key, RispExp::Number(val));
                n_count += 1;
            }
        }

        let mut worked = true;
        let mut error_msg = "".to_string();
        let binding = expr[1].to_string().clone();
        let mut statments: Vec<&str> = binding.split(";").collect();

        let _ = parse_eval("(def error (fn (s) (sys echo s)))".to_string(), &mut env).await;

        statments.pop();

        if statments.is_empty() {
            return Err(ProcessError {
                error_code: "statements must end with a semicolon".to_string(),
            });
        }

        for code in statments {
            match parse_eval(code.to_string(), &mut env).await {
                Ok(_) => (),
                Err(RispErr::Reason(msg)) => {
                    worked = false;
                    error_msg = msg;
                }
            }
        }

        if worked {
            Ok(Success {
                success_code: "worked".to_string(),
                print_code: false,
            })
        } else {
            Err(ProcessError {
                error_code: error_msg,
            })
        }
    })
}
