use std::env;

fn main() {
    let region = env::var("AWS_REGION").unwrap_or_else(|_| "eu-west-1".to_string());

    println!("Selected region: {}", region);
}
