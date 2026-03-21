terraform {
  backend "s3" {
    bucket = "jluszcz-tf-state"
    key    = "list-of-lists/shared"
    region = "us-east-2"
  }
}

variable "aws_region" {
  type    = string
  default = "us-east-2"
}

provider "aws" {
  region = var.aws_region
}

data "aws_caller_identity" "current" {}

data "aws_s3_bucket" "code_bucket" {
  bucket = format("code-%s-%s-an", data.aws_caller_identity.current.account_id, var.aws_region)
}

data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

# Generator bucket

resource "aws_s3_bucket" "generator" {
  bucket           = format("list-of-lists-%s-%s-an", data.aws_caller_identity.current.account_id, var.aws_region)
  bucket_namespace = "account-regional"
}

resource "aws_s3_bucket_public_access_block" "generator" {
  bucket = aws_s3_bucket.generator.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_versioning" "generator" {
  bucket = aws_s3_bucket.generator.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "generator" {
  depends_on = [aws_s3_bucket_versioning.generator]

  bucket = aws_s3_bucket.generator.id

  rule {
    id = "expire-noncurrent"

    noncurrent_version_expiration {
      noncurrent_days = 30
    }

    status = "Enabled"
  }

  rule {
    id     = "abort-mpu"
    status = "Enabled"

    abort_incomplete_multipart_upload {
      days_after_initiation = 7
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "generator" {
  bucket = aws_s3_bucket.generator.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "aws:kms"
    }
    bucket_key_enabled = true
  }
}

# Lambda

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/aws/lambda/list-of-lists"
  retention_in_days = "7"
}

data "aws_iam_policy_document" "lambda_assume_role" {
  statement {
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "lambda" {
  name               = "list-of-lists.lambda"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

data "aws_iam_policy_document" "cw" {
  statement {
    actions   = ["cloudwatch:PutMetricData"]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "cloudwatch:namespace"
      values   = ["list_of_lists"]
    }
  }
}

resource "aws_iam_policy" "cw" {
  name   = "list-of-lists.cw"
  policy = data.aws_iam_policy_document.cw.json
}

resource "aws_iam_role_policy_attachment" "cw" {
  role       = aws_iam_role.lambda.name
  policy_arn = aws_iam_policy.cw.arn
}

resource "aws_iam_role_policy_attachment" "basic_execution_role_attachment" {
  role       = aws_iam_role.lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

data "aws_iam_policy_document" "s3" {
  statement {
    actions = ["s3:PutObject"]
    # Intentionally broad: grants write access to index.html in any bucket in the account
    # to support deploying to multiple site buckets without updating this policy per site.
    resources = ["arn:aws:s3:::*/index.html"]
  }

  statement {
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.generator.arn}/*"]
  }
}

resource "aws_iam_policy" "s3" {
  name   = "list-of-lists.s3"
  policy = data.aws_iam_policy_document.s3.json
}

resource "aws_iam_role_policy_attachment" "s3" {
  role       = aws_iam_role.lambda.name
  policy_arn = aws_iam_policy.s3.arn
}

resource "aws_s3_bucket_notification" "notification" {
  bucket = aws_s3_bucket.generator.id

  lambda_function {
    lambda_function_arn = aws_lambda_function.lambda.arn
    events              = ["s3:ObjectCreated:*"]
  }
}

resource "aws_lambda_permission" "allow_bucket" {
  statement_id  = "list-of-lists-allow-exec-from-s3"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda.function_name
  principal     = "s3.amazonaws.com"
  source_arn    = aws_s3_bucket.generator.arn
}

resource "aws_lambda_function" "lambda" {
  function_name = "list-of-lists"
  s3_bucket     = data.aws_s3_bucket.code_bucket.bucket
  s3_key        = "list-of-lists.zip"
  role          = aws_iam_role.lambda.arn
  architectures = ["arm64"]
  runtime       = "provided.al2023"
  handler       = "ignored"
  publish       = false
  description   = "Generate list-of-lists sites"
  timeout       = 5
  memory_size   = 128

  lifecycle {
    ignore_changes = [
      # Code is deployed by GitHub Actions, not Terraform
      s3_bucket,
      s3_key,
      s3_object_version,
    ]
  }

  environment {
    variables = {
      LOL_GENERATOR_BUCKET = aws_s3_bucket.generator.bucket
    }
  }
}

# GitHub Actions: upload index.template

data "aws_iam_policy_document" "github_update_template" {
  statement {
    actions   = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.generator.arn}/index.template"]
  }
}

resource "aws_iam_policy" "github_update_template" {
  name   = "list-of-lists.github-update"
  policy = data.aws_iam_policy_document.github_update_template.json
}

resource "aws_iam_role" "github_update_template" {
  name = "list-of-lists.github-update"

  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Principal = {
          Federated = data.aws_iam_openid_connect_provider.github.arn
        },
        Action = "sts:AssumeRoleWithWebIdentity",
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" : "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" : "repo:jluszcz/ListOfLists-rs:*"
          },
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "github_update_template" {
  role       = aws_iam_role.github_update_template.name
  policy_arn = aws_iam_policy.github_update_template.arn
}

# GitHub Actions: deploy Lambda code

data "aws_iam_policy_document" "github_deploy" {
  statement {
    actions   = ["s3:PutObject"]
    resources = ["${data.aws_s3_bucket.code_bucket.arn}/list-of-lists.zip"]
  }
}

resource "aws_iam_policy" "github_deploy" {
  name   = "list-of-lists.github-deploy"
  policy = data.aws_iam_policy_document.github_deploy.json
}

resource "aws_iam_role" "github_deploy" {
  name = "list-of-lists.github-deploy"

  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Principal = {
          Federated = data.aws_iam_openid_connect_provider.github.arn
        },
        Action = "sts:AssumeRoleWithWebIdentity",
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" : "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" : "repo:jluszcz/ListOfLists-rs:*"
          },
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "github_deploy" {
  role       = aws_iam_role.github_deploy.name
  policy_arn = aws_iam_policy.github_deploy.arn
}
