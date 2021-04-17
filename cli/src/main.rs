use engine::{StepResult, step, EngineState};

fn user_choice(choices: &[String]) -> usize {
    for (idx, choice) in choices.iter().enumerate() {
        println!(" {}. {}", idx + 1, choice);
    }

    let mut input = String::new();
    loop {
        print!(">> ");
        if std::io::stdin().read_line(&mut input).is_err() {
            continue;
        }
        match input.trim().parse::<usize>() {
            Ok(x) if x >= 1 && x <= choices.len() => return x - 1,
            _ => (),
        }
        input.clear();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = EngineState::new(r"C:\Users\Host\Downloads\Kanon");
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
                state.set_choice(user_choice(&choices));
            }
        }
    }
    Ok(())
}
