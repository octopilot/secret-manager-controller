# AWS Setup Guide

Configure the Secret Manager Controller to work with AWS Secrets Manager.

## Prerequisites

- AWS account with Secrets Manager access
- IAM user or role with appropriate permissions
- Kubernetes cluster with controller installed

## IAM Permissions

Your AWS credentials need the following minimum permissions to create and manage secrets:

### Minimum Required Permissions

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:CreateSecret",
        "secretsmanager:PutSecretValue",
        "secretsmanager:GetSecretValue",
        "secretsmanager:DescribeSecret",
        "secretsmanager:ListSecrets",
        "secretsmanager:UpdateSecret",
        "secretsmanager:DeleteSecret",
        "secretsmanager:TagResource"
      ],
      "Resource": "*"
    }
  ]
}
```

### Recommended: Scoped Permissions

For better security, scope permissions to specific secret paths:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:CreateSecret",
        "secretsmanager:PutSecretValue",
        "secretsmanager:GetSecretValue",
        "secretsmanager:DescribeSecret",
        "secretsmanager:UpdateSecret",
        "secretsmanager:DeleteSecret",
        "secretsmanager:TagResource"
      ],
      "Resource": [
        "arn:aws:secretsmanager:*:*:secret:my-service/*",
        "arn:aws:secretsmanager:*:*:secret:production/*"
      ]
    },
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:ListSecrets"
      ],
      "Resource": "*"
    }
  ]
}
```

### Using AWS Managed Policies

You can use the AWS managed policy `SecretsManagerReadWrite` for full access:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:*"
      ],
      "Resource": "*"
    }
  ]
}
```

**Note:** The managed policy `SecretsManagerReadWrite` provides read/write access. For production, prefer scoped permissions above.

## Authentication Methods

### Method 1: IAM Role (Recommended)

If running on EKS, use an IAM role for the service account:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: octopilot-system
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::ACCOUNT_ID:role/SecretManagerRole
```

### Method 2: Access Keys

Create a Kubernetes Secret with AWS credentials:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: aws-credentials
  namespace: octopilot-system
type: Opaque
stringData:
  AWS_ACCESS_KEY_ID: your-access-key-id
  AWS_SECRET_ACCESS_KEY: your-secret-access-key
  AWS_REGION: us-east-1
```

Reference in your SecretManagerConfig:

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: aws-secrets
spec:
  provider: aws
  region: us-east-1
  credentials:
    secretRef:
      name: aws-credentials
      namespace: octopilot-system
  secrets:
    - name: database-password
      key: /myapp/database/password
```

## Configuration Example

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: production-secrets
  namespace: production
spec:
  provider: aws
  region: us-east-1
  secrets:
    - name: db-password
      key: /production/database/password
    - name: api-key
      key: /production/api/key
```

## Troubleshooting

### Common Issues

1. **Authentication Failed**
   - Verify IAM permissions
   - Check credential configuration
   - Ensure region is correct

2. **Secret Not Found**
   - Verify secret exists in AWS Secrets Manager
   - Check secret key path
   - Verify IAM permissions include the secret

3. **Network Issues**
   - Check cluster network connectivity to AWS
   - Verify VPC endpoints if using private networking

## Next Steps

- [Azure Setup](./azure-setup.md)
- [GCP Setup](./gcp-setup.md)
- [GitOps Integration](./gitops-integration.md)

