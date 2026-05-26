use kaspascript_lexer::lex_file;
use kaspascript_parser::parse_file;

#[test]
fn lexer_and_parser_never_panic_on_random_ascii() {
    let mut state = 0xC0FFEE_u64;
    for case in 0..512 {
        let len = (next(&mut state) % 128) as usize;
        let mut input = String::new();
        for _ in 0..len {
            let byte = 32 + (next(&mut state) % 95) as u8;
            input.push(char::from(byte));
        }

        let file = format!("fuzz-{case}.ks");
        let lex_result = std::panic::catch_unwind(|| lex_file(&input, &file));
        assert!(lex_result.is_ok(), "lexer panicked on case {case}");

        let parse_result = std::panic::catch_unwind(|| parse_file(&input, &file));
        assert!(parse_result.is_ok(), "parser panicked on case {case}");
    }
}

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}
