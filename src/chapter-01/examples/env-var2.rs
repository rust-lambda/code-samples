use std::env;

fn main() {
    let region = env::var("AWS_REGION");
    match region {
        Ok(value) => println!("Selected region: {}", value),
        Err(_) => eprintln!("Error: AWS_REGION not set"),
    }
}
