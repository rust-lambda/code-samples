# Lambda Process EventBridge

An AWS Lambda function in Rust that processes events from Amazon EventBridge.

## What it does

1. Receives an EventBridge event with an `OrderCreated` detail type
2. Deserializes the event detail into a typed `OrderCreatedDetail` struct
3. Processes the order and logs the details
4. Returns `Ok(())` on success

## Key concepts

- Using `EventBridgeEvent<T>` to handle EventBridge triggers with typed detail
- Deserializing event detail directly into a Rust struct with `serde`
- Basic Lambda function structure with `lambda_runtime`

## Event structure

The Lambda expects EventBridge events with this structure:

```json
{
  "id": "7bf73129-1428-4cd3-a780-95db273d1602",
  "detail-type": "OrderCreated",
  "source": "myapp.orderCreated",
  "account": "123456789012",
  "time": "2021-11-11T21:29:54Z",
  "region": "us-east-1",
  "detail": {
    "order_id": "order-123",
    "customer_id": "customer-456",
    "order_value": 129.99
  }
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
cargo lambda invoke --data-file examples/lambda-process-eventbridge/events/example.json lambda-process-eventbridge
```

## Expected output

```
Processing order order-123 for customer customer-456 with value $129.99
Successfully processed order: order-123
```
