# Lambda Process Kinesis

An AWS Lambda function in Rust that processes records from an Amazon Kinesis Data Stream.

## What it does

1. Receives a Kinesis event containing one or more records
2. Iterates through each record in the batch
3. Decodes the base64-encoded data from `record.kinesis.data`
4. Deserializes the JSON into an `OrderEvent` struct
5. Processes and logs each order
6. Returns `Ok(())` on success

## Key concepts

- Using `KinesisEvent` to handle Kinesis stream triggers
- Kinesis data is base64-encoded and needs to be decoded
- Lambda receives records in batches for efficient processing
- Deserializing JSON payloads with `serde`

## Record data format

Each Kinesis record contains base64-encoded JSON with this structure:

```json
{
  "order_id": "order-1",
  "customer_id": "customer-1",
  "amount": 11.11
}
```

## Prerequisites

- [Cargo Lambda](https://www.cargo-lambda.info/) installed
- AWS credentials configured (for deployment)

## How to run locally

```bash
# Start the Lambda runtime emulator
cargo lambda watch

# In another terminal, invoke with the example event
cargo lambda invoke --data-file examples/lambda-process-kinesis/events/example.json lambda-process-kinesis
```

## Expected output

```
Processing order: order-1
Order order-1 for customer customer-1 with amount $11.11
Processing order: order-2
Order order-2 for customer customer-2 with amount $22.22
```
