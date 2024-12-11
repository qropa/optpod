use proconio::input;

fn main() {
    input! {
        n: usize,
    }
    for i in [0, 1, 0] {
        let name = "apple".to_string() + &i.to_string();
        eprintln!("[DATA] {} = {}", name, n + i);
    }
    println!("n={}", n);
    eprintln!("[DATA] score = {}", n);
}
