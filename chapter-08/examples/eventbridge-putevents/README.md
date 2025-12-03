# EventBridge Put Events

An example demonstrating how to publish custom events to Amazon EventBridge using the AWS SDK for Rust.

## What it does

1. Reads the `EVENT_BUS_NAME` environment variable (defaults to `"default"`)
2. Creates an EventBridge client using default AWS credentials
3. Builds an `OrderCreated` event with structured JSON detail
4. Publishes the event to the specified event bus
5. Prints the event ID on success

## Key concepts

- Using `PutEventsRequestEntry` to build EventBridge events
- Setting event `source`, `detail-type`, and `detail` fields
- Serializing Rust structs to JSON for the event detail

## Event structure

```json
{
  "source": "custom.myapp",
  "detail-type": "OrderCreated",
  "detail": {
    "order_id": "order-123",
    "customer_id": "customer-456",
    "order_value": 129.99
  }
}
```

## Prerequisites

- AWS credentials configured (via environment variables, AWS CLI, or IAM role)
- Permissions to publish to EventBridge

## How to run

```bash
# Optional: specify a custom event bus (defaults to "default")
export EVENT_BUS_NAME="my-custom-bus"

# Run the example
cargo run -p eventbridge-putevents
```

## Expected output

```
Event published successfully!
Event ID: 12345678-1234-1234-1234-123456789012
```
