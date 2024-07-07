use std::env;

fn main() {
    let region = env::var("AWS_REGION");
    //  ^? Result<String, std::env::VarError>

    println!("{:?}", region);
}
