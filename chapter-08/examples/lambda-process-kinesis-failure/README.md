# Lambda Process Kinesis (Partial Batch Failure)

An AWS Lambda function in Rust demonstrating partial batch failure handling for Kinesis records.

## What it does

1. Receives a Kinesis event containing one or more records
2. Processes each record with a 50% simulated failure rate
3. Tracks failed records using `KinesisEventResponse`
4. Returns only the failed sequence numbers so AWS can retry them
5. Successfully processed records are checkpointed

## Key concepts

- **Partial batch failure**: Instead of failing the entire batch, report only failed items
- Using `KinesisEventResponse` and `KinesisBatchItemFailure` to report failures
- Item identifier is the record's `sequence_number`
- AWS Lambda will only retry records from the failed sequence number onwards
- Requires `ReportBatchItemFailures` enabled on the Kinesis event source mapping

## Why partial batch failures?

Without partial batch failure handling, if any record fails:
- The entire batch is retried from the beginning
- Successfully processed records are re-processed
- This can cause duplicate processing and wasted compute

With partial batch failures:
- Only failed records are retried
- Successfully processed records are checkpointed
- More efficient and reduces duplicates

## Prerequisites

- [Cargo Lambda](https://www.cargo-lambda.info/) installed
- AWS credentials configured (for deployment)

## How to run locally

```bash
# Start the Lambda runtime emulator
cargo lambda watch

# In another terminal, invoke with the example event
cargo lambda invoke --data-file examples/lambda-process-kinesis-failure/events/example.json lambda-process-kinesis-failure
```

## Expected output

Due to the 50% random failure rate, output varies. Example:

```json
{
  "batchItemFailures": [
    { "itemIdentifier": "49568167373333333333333333333333333333333333333333333333" }
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
