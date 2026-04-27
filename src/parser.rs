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
    expr.replace("(", " ( ")
        .replace(")", " ) ")
        .split_whitespace()
        .map(|x| x.to_string())
        .collect()
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
    match token.as_ref() {
        "true" => RispExp::Bool(true),
        "false" => RispExp::Bool(false),
        _ => {
            let potential_float: Result<f64, ParseFloatError> = token.parse();
            match potential_float {
                Ok(v) => RispExp::Number(v),
                Err(_) => RispExp::Symbol(token.to_string().clone()),
            }
        }
    }
}

#[derive(Clone)]
enum RispExp {
    Bool(bool),
    Symbol(String),
    Number(f64),
    List(Vec<RispExp>),
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
                                    _ => Err(RispErr::Reason(
                                        "sys args must evaluate to symbols, numbers, or bools"
                                            .to_string(),
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
                        _ => {} // Fall through
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
        }
    })
}

// 4. CLONE THE ENVIRONMENT!
// Instead of lifetime links, we clone the dictionary. Safe and zero-headache.
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
            RispExp::List(list) => {
                let xs: Vec<String> = list.iter().map(|x| x.to_string()).collect();
                format!("({})", xs.join(","))
            }
            RispExp::Func(_) => "Function {}".to_string(),
            RispExp::Lambda(_) => "Lambda {}".to_string(),
            RispExp::Syscall(cmd) => format!("Syscall: [{}]", cmd.join(", ")),
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
