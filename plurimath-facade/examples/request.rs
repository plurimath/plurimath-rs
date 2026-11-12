use std::io::{self, BufRead};
fn main() {
    for l in io::stdin().lock().lines() {
        let l = l.unwrap();
        println!("{}", plurimath_facade::request_json(l.as_bytes()));
    }
}
