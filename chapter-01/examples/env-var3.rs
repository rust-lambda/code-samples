use std::env;

fn main() {
    let region = env::var("AWS_REGION").expect("AWS_REGION environment variable not set");

    println!("Selected region: {}", region);
}
