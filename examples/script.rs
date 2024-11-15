use renpy_parser::parse_scenario;

fn main() {
    let (ast, _parse_error) = parse_scenario("assets/script.rpy").unwrap();

    for ast in ast {
        println!("ast: {:?}", ast);
    }
}
