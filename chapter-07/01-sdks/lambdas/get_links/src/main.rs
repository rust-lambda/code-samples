use http_handler::function_handler;
use lambda_http::{run, service_fn, tracing, Error};
use shared::adapters::DynamoDbUrlRepository;
use shared::configuration::Configuration;
use shared::core::UrlShortener;
use shared::url_info::HttpUrlInfo;

mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);

    let ssm_client = aws_sdk_ssm::Client::new(&config);
    let secret_client = aws_sdk_secretsmanager::Client::new(&config);
    let configuration = Configuration::load(&ssm_client, &secret_client).await;

    let http_client = shared::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    let url_info = HttpUrlInfo::new(http_client.clone());
    let url_repo = DynamoDbUrlRepository::new(&configuration, dynamodb_client);
    let shortener = UrlShortener::new(url_repo, url_info);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
