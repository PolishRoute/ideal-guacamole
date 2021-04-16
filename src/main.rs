#![feature(str_split_as_str)]

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::collections::HashMap;


#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
enum Instr {
    cleartext,
    setvar(VarOrConst, String),
    gsetvar(VarOrConst, String),
    bgload(VarOrConst, Option<usize>),
    setimg(VarOrConst, usize, usize),
    delay(usize),
    branch(VarOrConst, Operator, String, usize),
    text(Option<String>, String),
    goto(Label),
    sound(String, Option<usize>),
    music(String),
    choice(Vec<VarOrConst>),
    jump(String),
}

#[derive(Eq, PartialEq)]
#[derive(Copy, Clone)]
enum Operator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
}

impl std::fmt::Debug for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Operator::Equal => "==",
            Operator::NotEqual => "!=",
            Operator::Less => "<",
            Operator::LessEqual => "<=",
        })?;
        Ok(())
    }
}

#[derive(Clone)]
struct VarOrConst {
    is_ref: bool,
    name: String,
    index: Option<usize>,
}

impl std::fmt::Debug for VarOrConst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ref {
            write!(f, "$")?;
        }
        write!(f, "{}", self.name)?;
        if let Some(idx) = self.index {
            write!(f, "[{}]", idx)?;
        }
        Ok(())
    }
}

fn parse_text(s: &str) -> (Option<String>, String) {
    if s.contains('"') {
        if let Some((a, b)) = s.split_once(" ") {
            return (Some(unescape(a)), unescape(b));
        }
    }

    (None, unescape(s))
}

fn parse_var_ref(s: &str) -> VarOrConst {
    let (dollar, s) = match s.strip_prefix("$") {
        Some(x) => (true, x),
        None => (false, s),
    };

    let (name, index) = if let Some(iks) = s.strip_suffix("]") {
        let (name, x) = iks.split_once("[").unwrap();
        (name, Some(x))
    } else {
        (s, None)
    };

    VarOrConst {
        is_ref: dollar,
        name: name.to_string(),
        index: index.map(|x| x.parse().unwrap()),
    }
}

fn strip(s: &str, c: char) -> &str {
    let s = s.strip_prefix(c).unwrap_or(s);
    let s = s.strip_suffix(c).unwrap_or(s);
    s
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;

    let s = strip(s, '"');
    let s = strip(s, '\'');

    for c in s.chars() {
        match c {
            '\\' if !escaped => {
                escaped = true;
            }
            _ => {
                out.push(c);
                escaped = false;
            }
        }
    }
    out
}

struct Emitter {
    code: Vec<Instr>,
    last_branch: Option<usize>,
    labels: HashMap<Label, usize>,
}

impl Emitter {
    fn new() -> Self {
        Self {
            labels: HashMap::new(),
            code: vec![],
            last_branch: None,
        }
    }

    fn emit(&mut self, instr: Instr) {
        self.code.push(instr);
    }

    fn begin_branch(&mut self) {
        self.last_branch = Some(self.code.len());
    }

    fn end_branch(&mut self) {
        let next_instr = self.code.len();
        let branch_instr = self.last_branch.unwrap();
        match self.code[branch_instr] {
            Instr::branch(_, _, _, ref mut else_target) => {
                *else_target = next_instr;
            }
            _ => unimplemented!(),
        }
    }

    fn make_label(&mut self, label: Label) {
        self.labels.insert(label, self.code.len());
    }

    fn into_script(mut self) -> Script {
        for inst in self.code.iter_mut() {
            match inst {
                Instr::goto(ref mut target) => {
                    *target = match self.labels.get(target) {
                        Some(x) => Label::Offset(*x),
                        None => panic!("unknown label {:?}", target),
                    };
                }
                _ => ()
            }
        }

        Script { code: self.code }
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
enum Label {
    Offset(usize),
    Indexed(usize),
    Named(String),
}

fn split_args(line: &str, limit: usize) -> Vec<&str> {
    let mut parts = vec![];

    let mut it = line.split(|c: char| c.is_ascii_whitespace());
    let it = it.by_ref();

    while parts.len() < limit {
        match it.next() {
            Some("") => continue,
            Some(p) => parts.push(p),
            None => break,
        }
    }
    let rest = it.as_str().trim_start();
    if !rest.is_empty() {
        parts.push(rest);
    }
    parts
}

#[cfg(test)]
mod tests {
    use crate::{split_args, unescape};

    #[test]
    fn splitting() {
        assert_eq!(split_args("ab cd   e", 4), vec!["ab", "cd", "e"]);
        assert_eq!(split_args("ab cd   e    f", 4), vec!["ab", "cd", "e", "f"]);
        assert_eq!(split_args("ab cd   e    f  g", 4), vec!["ab", "cd", "e", "f  g"]);
    }

    #[test]
    fn unescaping() {
        assert_eq!(unescape("My cousin\\'s voice is coming from the alarm clock."),
                   "My cousin\'s voice is coming from the alarm clock.")
    }
}

struct Script {
    code: Vec<Instr>,
}

fn load_script(path: impl AsRef<Path>) -> Result<Script, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    let mut emitter = Emitter::new();

    for (lineno, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts = split_args(line, 3);
        match &parts[..] {
            &["cleartext", "!"] => {
                emitter.emit(Instr::cleartext);
            }
            &["gsetvar", name, "=" | "-" | "+", value] => {
                emitter.emit(Instr::gsetvar(
                    parse_var_ref(name),
                    unescape(value),
                ));
            }
            &["setvar", name, "=" | "-" | "+", value] => {
                emitter.emit(Instr::setvar(
                    parse_var_ref(name),
                    unescape(value),
                ));
            }
            &["setvar", name, value] => {
                emitter.emit(Instr::setvar(
                    parse_var_ref(name),
                    unescape(value),
                ));
            }
            &["bgload", vref] => {
                emitter.emit(Instr::bgload(
                    parse_var_ref(vref),
                    None,
                ));
            }
            &["bgload", vref, time] => {
                emitter.emit(Instr::bgload(
                    parse_var_ref(vref),
                    Some(time.parse().unwrap()),
                ));
            }
            &["setimg", vref, x, y] => {
                emitter.emit(Instr::setimg(
                    parse_var_ref(vref),
                    x.parse().unwrap(),
                    y.parse().unwrap(),
                ));
            }
            &["delay", delay] => {
                emitter.emit(Instr::delay(
                    delay.parse().unwrap(),
                ))
            }
            &["if", vref, op, val] => {
                emitter.begin_branch();
                emitter.emit(Instr::branch(
                    // TODO: this needs to be changed...
                    VarOrConst { is_ref: true, ..parse_var_ref(vref) },
                    match op {
                        "==" => Operator::Equal,
                        "!=" => Operator::NotEqual,
                        "<" => Operator::Less,
                        "<=" => Operator::LessEqual,
                        op => panic!("unsupported op: {}", op),
                    },
                    val.to_string(),
                    emitter.code.len(),
                ));
            }
            &["fi"] => {
                emitter.end_branch();
            }
            &["text", ..] => {
                let x = line[4..].trim();
                let (name, text) = parse_text(x);

                emitter.emit(Instr::text(
                    name,
                    text,
                ));
            }
            &["goto", label] => {
                let label = if let Some(x) = label.strip_prefix('@') {
                    Label::Indexed(x.parse().unwrap())
                } else {
                    Label::Named(label.to_string())
                };

                emitter.emit(Instr::goto(
                    label
                ));
            }
            &["label", ident] => {
                if let Some(x) = ident.strip_prefix('@') {
                    emitter.make_label(Label::Indexed(x.parse().unwrap()));
                } else {
                    emitter.make_label(Label::Named(ident.to_string()));
                }
            }
            &["sound", file] => {
                emitter.emit(Instr::sound(
                    file.to_string(),
                    None,
                ));
            }
            &["sound", file, param] => {
                emitter.emit(Instr::sound(
                    file.to_string(),
                    Some(param.parse().unwrap()),
                ));
            }
            &["music", file] => {
                emitter.emit(Instr::music(
                    file.to_string(),
                ));
            }
            &["choice", ..] => {
                emitter.emit(Instr::choice(
                    line[6..].trim_start().split("|").map(parse_var_ref).collect(),
                ));
            }
            &["jump", target] => {
                emitter.emit(Instr::jump(
                    target.to_string(),
                ));
            }
            _ => {
                panic!("{}: {:?}", lineno + 1, parts);
            }
        }
    }
    Ok(emitter.into_script())
}

struct GameState {
    scripts: HashMap<String, Script>,
    memory: HashMap<String, Vec<String>>,
    pc: usize,
    current_script: String,
    directory: PathBuf,
}

impl GameState {
    fn new(directory: impl Into<PathBuf>) -> Self {
        let mut state = Self {
            scripts: Default::default(),
            memory: Default::default(),
            pc: 0,
            current_script: "main.scr".to_string(),
            directory: directory.into(),
        };
        state.load_script("main.scr");
        state
    }

    fn insert(&mut self, var: &VarOrConst, val: String) {
        let (name, index) = match var {
            VarOrConst { is_ref: false, name, index } => {
                (name, index.unwrap_or(0))
            }
            _ => unimplemented!(),
        };

        let place = self.memory.entry(name.clone()).or_insert_with(Vec::new);
        if index >= place.len() {
            place.extend(std::iter::repeat(String::new()).take(index - place.len() + 1));
        }
        place[index] = val;
    }

    fn get_var<'a, 'b: 'a>(&'a self, var: &'b VarOrConst) -> Option<&str> {
        if !var.is_ref {
            return Some(&var.name);
        }

        let index = var.index.unwrap_or(0);
        let val = self.memory
            .get(&var.name)?
            .get(index)?
            .as_str();
        Some(val)
    }

    fn load_script(&mut self, name: &str) {
        let path = self.directory.join("Scripts").join(name);
        self.scripts.insert(name.to_string(), load_script(path).unwrap());
        self.current_script = name.to_string();
        self.pc = 0;
    }

    fn set_choice(&mut self, index: usize) {
        self.insert(&VarOrConst {
            is_ref: false,
            name: "selected".to_string(),
            index: None,
        }, (index + 1).to_string());
    }
}

#[derive(Debug)]
enum StepResult {
    Continue,
    Exit,
    Jump(String),
    Choice(Vec<String>),
}

fn step(state: &mut GameState) -> StepResult {
    let curr_inst = match state.scripts[&state.current_script].code.get(state.pc).cloned() {
        Some(ci) => ci,
        None => return StepResult::Exit,
    };
    match curr_inst {
        Instr::cleartext => {
            println!("// Clearing");
        }
        Instr::gsetvar(ident, value) => {
            state.insert(&ident, value.to_string());
        }
        Instr::setvar(ident, value) => {
            state.insert(&ident, value.to_string());
        }
        Instr::bgload(file, time) => {
            println!("// Loading background from {:?} {:?}", file, time);
        }
        Instr::setimg(file, x, y) => {
            println!("// Loading image from {:?} and placing it at {} {}", file, x, y);
        }
        Instr::delay(delay) => {
            println!("// Waiting for {} units of time", delay);
        }
        Instr::branch(lhs, op, rhs, else_target) => {
            // dbg!(lhs, op, rhs, else_target);
            // for (i, ii) in script.code.iter().enumerate() {
            //     println!("{}. {:?}", i, &ii);
            // }

            let lhs = state.get_var(&lhs).unwrap();
            let result = match op {
                Operator::Equal => lhs == rhs,
                Operator::NotEqual => lhs != rhs,
                Operator::Less => lhs < &rhs,
                Operator::LessEqual => lhs <= &rhs,
            };

            if result {
                state.pc += 1;
            } else {
                state.pc = else_target;
            }
            return StepResult::Continue;
        }
        Instr::text(Some(who), what) => {
            println!("{}: {}", who, what);
        }
        Instr::text(None, what) => {
            println!("{}", what);
        }
        Instr::goto(target) => {
            state.pc = match target {
                Label::Offset(x) => x,
                _ => unreachable!()
            };
            return StepResult::Continue;
        }
        Instr::sound(file, arg) => {
            println!("// Playing {} with {:?}", file, arg);
        }
        Instr::music(file) => {
            println!("// Playing {}", file);
        }
        Instr::choice(choices) => {
            state.pc += 1;
            state.set_choice(0); // default choice
            return StepResult::Choice(
                choices.iter().map(|ch| {
                    state.get_var(ch).unwrap().to_string()
                }).collect()
            );
        }
        Instr::jump(file) => {
            return StepResult::Jump(file);
        }
    }
    state.pc += 1;
    StepResult::Continue
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = GameState::new(r"C:\Users\Host\Downloads\Kanon");
    loop {
        match step(&mut state) {
            StepResult::Continue => {}
            StepResult::Exit => {
                println!("// Exitted!");
                break;
            }
            StepResult::Jump(file) => {
                println!("// Loading script {}", &file);
                state.load_script(&file);
            }
            StepResult::Choice(choices) => {
                for (idx, choice) in choices.iter().enumerate() {
                    println!("> {}. {}", idx + 1, choice);
                }
                state.set_choice(0);
            }
        }
    }
    Ok(())
}
