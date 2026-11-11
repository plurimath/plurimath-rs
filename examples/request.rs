use std::io::{self, BufRead};
fn main() {
    for l in io::stdin().lock().lines() {
        let l = l.unwrap();
        let v: serde_json::Value = serde_json::from_str(&l).unwrap();
        println!("{}", plurimath_facade::request(&v));
    }
}
