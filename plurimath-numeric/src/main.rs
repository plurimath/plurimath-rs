use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        match line {
            Ok(input) => println!("{}", plurimath_numeric::evaluate_json(&input)),
            Err(_) => break,
        }
    }
}
