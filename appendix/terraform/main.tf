provider "aws" {
  region = "eu-west-1"
}

module "rust_lambda_function" {
  source  = "terraform-aws-modules/lambda/aws"
  version = "~> 7.20"

  function_name = "hello-world-api"

  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]

  trigger_on_package_timestamp = false

  source_path = [
    {
      path = "${path.module}/../src/hello-world-api"
      commands = [
        "cargo lambda build --release --arm64",
        "cd target/lambda/hello-world-api",
        ":zip",
      ]
      patterns = [
        "!.*",
        "bootstrap",
      ]
    }
  ]

  create_lambda_function_url = true
}

output "function_url" {
  value       = module.rust_lambda_function.lambda_function_url
  description = "The URL of the Lambda function"
}
