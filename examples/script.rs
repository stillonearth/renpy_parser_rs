use renpy_parser::{parse_scenario_from_file, parsers::ASTVec};

fn main() {
    let (ast, parse_error) = parse_scenario_from_file("assets/script_2.rpy").unwrap();

    for e in parse_error {
        println!("error: {:?}", e);
    }

    println!("{}", ASTVec(&ast));
}
