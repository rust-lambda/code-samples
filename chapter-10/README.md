# Query examples for Amazon CloudWatch

## OTEL in CloudWatch

1. Enable [CloudWatch Transaction Search](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/Enable-TransactionSearch.html). For testing purposes, set sampling to 100%.
2. View all process spans by querying `name ^ process`

## CloudWatch Log Insights

```
fields @timestamp, resource.attributes.service.name, severityText, body
| filter resource.attributes.faas.name = "GetLinksFunction-dev"
| sort @timestamp desc
| limit 10000
```