# Lambda Process SQS Example

This example demonstrates how to create an AWS Lambda function in Rust that processes messages from an Amazon SQS queue. The function is triggered by SQS events and processes each message by deserializing its content and performing a mock task.

## Local invocation

From `chapter-08` directory, run:

```bash
cargo lambda invoke --data-file examples/lambda-process-sqs/events/example.json lambda-process-sqs
```