import { join } from 'node:path';
import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import { RustFunction } from 'cargo-lambda-cdk';
import { FunctionUrlAuthType } from 'aws-cdk-lib/aws-lambda';
import { CfnOutput } from 'aws-cdk-lib';

export class RustyLambdaStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // defines the Rust function
    const helloWorldApi = new RustFunction(this, 'Rust function', {
      manifestPath: join(__dirname, '..', '..', 'src', 'hello-world-api', 'Cargo.toml'),
    });

    // adds a URL to the Rust function
    const helloWorldApiFnUrl = helloWorldApi.addFunctionUrl({
      // No authentication required (for demonstration purposes only)
      authType: FunctionUrlAuthType.NONE
    })

    // output the URL of the Rust function
    new CfnOutput(this, 'helloWorldApiFnUrl', { value: helloWorldApiFnUrl.url });
  }
}
