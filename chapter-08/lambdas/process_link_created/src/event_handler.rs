use aws_lambda_events::{
    event::sqs::SqsEvent,
    sqs::{BatchItemFailure, SqsBatchResponse, SqsMessage},
};
use lambda_runtime::{tracing, Error, LambdaEvent};
use shared::core::{ShortUrl, UrlInfo, UrlRepository};

pub(crate) struct HandlerDeps<R: UrlRepository, I: UrlInfo> {
    pub url_repo: R,
    pub url_info: I,
}

pub(crate) async fn function_handler<R: UrlRepository, I: UrlInfo>(
    deps: &HandlerDeps<R, I>,
    event: LambdaEvent<SqsEvent>,
) -> Result<SqsBatchResponse, Error> {
    let mut sqs_batch_response = SqsBatchResponse::default();
    let payload = event.clone().payload;
    let tasks: Vec<_> = event
        .payload
        .records
        .into_iter()
        .map(|message| process_message(&deps.url_repo, &deps.url_info, message))
        .collect();
    let results = futures::future::join_all(tasks).await; // Run tasks concurrently

    let failure_items = results
        .into_iter()
        .zip(payload.records.into_iter())
        .filter_map(|(result, message)| {
            if let Err(e) = result {
                tracing::error!("Failed to process message {:?}: {}", message.message_id, e);
                let mut failure_item = BatchItemFailure::default();
                failure_item.item_identifier = message.message_id.unwrap_or_default();
                Some(failure_item)
            } else {
                None
            }
        })
        .collect::<Vec<BatchItemFailure>>();

    sqs_batch_response.batch_item_failures = failure_items;
    Ok(sqs_batch_response)
}

async fn process_message<R: UrlRepository, I: UrlInfo>(
    url_repo: &R,
    url_info: &I,
    message: SqsMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = message.body.clone();
    if url.is_none() {
        tracing::warn!(
            "Discarding empty SQS message body for message {:?}",
            message.message_id
        );
        // NOTE: we don't add this to the failed list as we don't want to reprocess it
        return Ok(());
    }
    let short_url: ShortUrl = serde_json::from_str(&url.unwrap())?;
    let info = url_info.fetch_details(&short_url.original_link).await?;
    tracing::debug!(
        "Fetched info for URL {}: {:?}",
        short_url.original_link,
        info
    );
    url_repo
        .add_details_to_short_url(short_url.original_link, info)
        .await?;
    Ok(())
}
