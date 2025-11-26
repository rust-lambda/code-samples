# Lambda Process SQS (Partial Batch Failure)

An AWS Lambda function in Rust demonstrating partial batch failure handling for SQS messages.

## What it does

1. Receives an SQS event containing one or more messages
2. Processes each message with a 50% simulated failure rate
3. Tracks failed messages using `SqsBatchResponse`
4. Returns only the failed message IDs so AWS can retry them
5. Successfully processed messages are removed from the queue

## Key concepts

- **Partial batch failure**: Instead of failing the entire batch, report only failed items
- Using `SqsBatchResponse` and `BatchItemFailure` to report failures
- AWS Lambda will only retry the specific messages that failed
- Requires `ReportBatchItemFailures` enabled on the SQS event source mapping

## Why partial batch failures?

Without partial batch failure handling, if any message fails:
- The entire batch is retried
- Successfully processed messages are re-processed
- This can cause duplicate processing

With partial batch failures:
- Only failed messages are retried
- Successfully processed messages are acknowledged
- More efficient and avoids duplicates

## Prerequisites

- [Cargo Lambda](https://www.cargo-lambda.info/) installed
- AWS credentials configured (for deployment)

## How to run locally

```bash
# Start the Lambda runtime emulator
cargo lambda watch

# In another terminal, invoke with the example event
cargo lambda invoke --data-file examples/lambda-process-sqs-failure/events/example.json lambda-process-sqs-failure
```

## Expected output

Due to the 50% random failure rate, output varies. Example:

```json
{
  "batchItemFailures": [
    { "itemIdentifier": "MessageID_1" }
  ]
}
```

Run multiple times to see different failure patterns.

## AWS configuration

When deploying, enable partial batch failures on your event source mapping:

```yaml
EventSourceMapping:
  Type: AWS::Lambda::EventSourceMapping
  Properties:
    FunctionResponseTypes:
      - ReportBatchItemFailures
```
