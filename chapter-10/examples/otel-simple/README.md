# SQS Send Message (Structured)

An example demonstrating how to send a structured JSON message to an Amazon SQS queue using the AWS SDK for Rust.

## What it does

1. Reads the `QUEUE_URL` environment variable
2. Creates an SQS client using default AWS credentials
3. Defines a `ScrapeLinkMessage` struct with `link_id` and `target_url` fields
4. Serializes the struct to JSON and sends it to the queue
5. Prints the message ID on success

## Key concepts

- Using `serde` to serialize Rust structs to JSON
- Sending structured data through SQS for downstream processing

## Prerequisites

- AWS credentials configured (via environment variables, AWS CLI, or IAM role)
- An existing SQS queue

## How to run

```bash
# Set the queue URL
export QUEUE_URL="https://sqs.us-east-1.amazonaws.com/123456789012/my-queue"

# Run the example
cargo run -p sqs-sendmessage-structured
```

## Expected output

```
Message sent successfully!
Message ID: 12345678-1234-1234-1234-123456789012
```

## Message format

The message body sent to SQS will be JSON:

```json
{
  "link_id": "abc123",
  "target_url": "https://example.com"
}
```
