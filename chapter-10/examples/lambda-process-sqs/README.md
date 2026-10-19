# Lambda Process SQS

An AWS Lambda function in Rust that processes messages from an Amazon SQS queue.

## What it does

1. Receives an SQS event containing one or more messages
2. Iterates through each message record
3. Deserializes the JSON message body into a `MyMessage` struct
4. Processes each task and logs completion
5. Returns `Ok(())` on success (all messages acknowledged)

## Key concepts

- Using `aws_lambda_events::event::sqs::SqsEvent` to handle SQS triggers
- Deserializing JSON message bodies with `serde`
- Basic Lambda function structure with `lambda_runtime`

## Message format

The Lambda expects messages with this JSON structure:

```json
{
  "task_id": "task_1",
  "data": "Example message body"
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
cargo lambda invoke --data-file examples/lambda-process-sqs/events/example.json lambda-process-sqs
```

## Expected output

```
Processing task: task_1 - Example message body
Task task_1 completed
Processing task: task_2 - Example message body 2
Task task_2 completed
```
