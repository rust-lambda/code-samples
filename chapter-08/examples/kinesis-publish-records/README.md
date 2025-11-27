# Kinesis Publish Records

An example demonstrating how to publish a record to an Amazon Kinesis Data Stream using the AWS SDK for Rust.

## What it does

1. Reads the `STREAM_NAME` environment variable
2. Creates a Kinesis client using default AWS credentials
3. Builds an `OrderEvent` struct and serializes it to JSON
4. Publishes the record using `put_record` with `order_id` as the partition key
5. Prints the shard ID and sequence number on success

## Key concepts

- Using `put_record` to publish a single record to Kinesis
- Partition keys determine which shard receives the record
- Data is sent as a `Blob` (binary)
- Kinesis returns shard ID and sequence number for each record

## Record format

The record body sent to Kinesis will be JSON:

```json
{
  "order_id": "order-123",
  "customer_id": "customer-456",
  "amount": 99.99
}
```

## Prerequisites

- AWS credentials configured (via environment variables, AWS CLI, or IAM role)
- An existing Kinesis Data Stream

## How to run

```bash
# Set the stream name
export STREAM_NAME="your-kinesis-stream-name"

# Run the example
cargo run -p kinesis-publish-records
```

## Expected output

```
Record published successfully!
Shard ID: shardId-000000000003
Sequence Number: 49669288035386241132338264381729324601959482583584604210
```
