# SQS Send Message

A minimal example demonstrating how to send a plain text message to an Amazon SQS queue using the AWS SDK for Rust.

## What it does

1. Reads the `QUEUE_URL` environment variable
2. Creates an SQS client using default AWS credentials
3. Sends a simple text message ("Hello from SQS!") to the queue
4. Prints the message ID on success

## Prerequisites

- AWS credentials configured (via environment variables, AWS CLI, or IAM role)
- An existing SQS queue

## How to run

```bash
# Set the queue URL
export QUEUE_URL="https://sqs.us-east-1.amazonaws.com/123456789012/my-queue"

# Run the example
cargo run -p sqs-sendmessage
```

## Expected output

```
Message sent successfully!
Message ID: 12345678-1234-1234-1234-123456789012
```
