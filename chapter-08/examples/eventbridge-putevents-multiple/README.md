# EventBridge Put Events (Multiple)

An example demonstrating how to publish multiple events to Amazon EventBridge in a single API call using the AWS SDK for Rust.

## What it does

1. Reads the `EVENT_BUS_NAME` environment variable (defaults to `"default"`)
2. Creates an EventBridge client using default AWS credentials
3. Builds 3 `OrderCreated` events with different order details
4. Publishes all events in a single `put_events` API call
5. Prints each event ID on success

## Key concepts

- Batching multiple events in a single `put_events` call for efficiency
- Using `set_entries()` to send a vector of `PutEventsRequestEntry`
- EventBridge supports up to 10 events per `put_events` call

## Event structure

Each event follows this structure (with varying values):

```json
{
  "source": "custom.myapp",
  "detail-type": "OrderCreated",
  "detail": {
    "order_id": "order-0",
    "customer_id": "customer-0",
    "order_value": 100.0
  }
}
```

## Prerequisites

- AWS credentials configured (via environment variables, AWS CLI, or IAM role)
- Permissions to publish to EventBridge

## How to run

```bash
# Optional: specify a custom event bus (defaults to "default")
export EVENT_BUS_NAME="my-event-bus"

# Run the example
cargo run -p eventbridge-putevents-multiple
```

## Expected output

```
Event published successfully!
Event ID: 11111111-1111-1111-1111-111111111111
Event ID: 22222222-2222-2222-2222-222222222222
Event ID: 33333333-3333-3333-3333-333333333333
```
