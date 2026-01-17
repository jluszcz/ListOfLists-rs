terraform {
  backend "s3" {
    bucket = "jluszcz-tf-state"
    key    = "list-of-lists"
    region = "us-east-2"
  }
}

# Sourced from environment variables named TF_VAR_${VAR_NAME}
variable "aws_acct_id" {}

variable "site_name" {}

variable "site_url" {}

variable "code_bucket" {}

variable "github_org" {}

variable "github_repo" {}

variable "aws_region" {
  type    = string
  default = "us-east-2"
}

provider "aws" {
  region = var.aws_region
}

provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

data "aws_s3_bucket" "code_bucket" {
  bucket = var.code_bucket
}

resource "aws_s3_bucket" "site" {
  bucket = var.site_url
}

resource "aws_s3_bucket_public_access_block" "site" {
  bucket = aws_s3_bucket.site.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_website_configuration" "site" {
  bucket = aws_s3_bucket.site.id

  index_document {
    suffix = "index.html"
  }
}

data "aws_iam_policy_document" "site" {
  statement {
    actions = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.site.arn}/*"]

    principals {
      type = "Service"
      identifiers = ["cloudfront.amazonaws.com"]
    }

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceArn"

      values = [aws_cloudfront_distribution.site.arn]
    }
  }
}

resource "aws_s3_bucket_policy" "site" {
  bucket = aws_s3_bucket.site.id
  policy = data.aws_iam_policy_document.site.json
}

resource "aws_s3_bucket_server_side_encryption_configuration" "site" {
  bucket = aws_s3_bucket.site.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "aws:kms"
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_object" "favicon" {
  count  = fileexists("buckets/${var.site_url}/images/favicon.ico") ? 1 : 0
  bucket = aws_s3_bucket.site.id
  key    = "images/favicon.ico"
  source = "buckets/${var.site_url}/images/favicon.ico"
  etag = filemd5("buckets/${var.site_url}/images/favicon.ico")
}

resource "aws_acm_certificate" "cert" {
  provider          = aws.us_east_1
  domain_name       = var.site_url
  subject_alternative_names = ["www.${var.site_url}"]
  validation_method = "DNS"
}

resource "aws_acm_certificate_validation" "cert" {
  provider                = aws.us_east_1
  certificate_arn         = aws_acm_certificate.cert.arn
  validation_record_fqdns = [for record in aws_route53_record.cert_validation : record.fqdn]
}

resource "aws_route53_record" "cert_validation" {
  for_each = {
    for dvo in aws_acm_certificate.cert.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }

  allow_overwrite = true
  name            = each.value.name
  type            = each.value.type
  zone_id         = aws_route53_zone.zone.id
  records = [each.value.record]
  ttl             = 60
}

resource "aws_cloudfront_origin_access_control" "site_distribution_oac" {
  name                              = var.site_name
  description                       = "OAC for ${var.site_url}"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_cloudfront_distribution" "site" {
  origin {
    domain_name              = aws_s3_bucket.site.bucket_domain_name
    origin_id                = "site_bucket_origin"
    origin_access_control_id = aws_cloudfront_origin_access_control.site_distribution_oac.id
  }

  enabled             = true
  is_ipv6_enabled     = true
  http_version        = "http2and3"
  default_root_object = "index.html"

  aliases = ["www.${var.site_url}", var.site_url]

  default_cache_behavior {
    allowed_methods = ["GET", "HEAD"]
    cached_methods = ["GET", "HEAD"]
    target_origin_id = "site_bucket_origin"

    forwarded_values {
      query_string = false

      cookies {
        forward = "none"
      }
    }

    viewer_protocol_policy = "redirect-to-https"
    min_ttl                = 3600
    default_ttl            = 86400
    max_ttl                = 604800
    compress               = true
  }

  price_class = "PriceClass_All"

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate.cert.arn
    minimum_protocol_version = "TLSv1.2_2021"
    ssl_support_method       = "sni-only"
  }
}

resource "aws_s3_bucket" "generator" {
  bucket = "${var.site_url}-generator"
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

resource "aws_route53_zone" "zone" {
  name    = var.site_url
  comment = "${var.site_name} Hosted Zone"
}

resource "aws_route53_record" "record" {
  zone_id = aws_route53_zone.zone.zone_id
  name    = var.site_url
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.site.domain_name
    zone_id                = aws_cloudfront_distribution.site.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "record_www" {
  zone_id = aws_route53_zone.zone.zone_id
  name    = "www.${var.site_url}"
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.site.domain_name
    zone_id                = aws_cloudfront_distribution.site.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/aws/lambda/${var.site_name}"
  retention_in_days = "7"
}

data "aws_iam_policy_document" "lambda_assume_role" {
  statement {
    principals {
      type = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "lambda" {
  name               = "${var.site_name}.lambda"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

data "aws_iam_policy_document" "cw" {
  statement {
    actions = ["cloudwatch:PutMetricData"]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "cloudwatch:namespace"
      values = ["list_of_lists"]
    }
  }
}

resource "aws_iam_policy" "cw" {
  name   = "${var.site_name}.cw"
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
    resources = ["${aws_s3_bucket.site.arn}/index.html"]
  }

  statement {
    actions = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.generator.arn}/*"]
  }
}

resource "aws_iam_policy" "s3" {
  name   = "${var.site_name}.s3"
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
    events = ["s3:ObjectCreated:*"]
  }
}

resource "aws_lambda_permission" "allow_bucket" {
  statement_id  = "${var.site_name}-allow-exec-from-s3"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda.arn
  principal     = "s3.amazonaws.com"
  source_arn    = aws_s3_bucket.generator.arn
}

resource "aws_lambda_function" "lambda" {
  function_name = "${var.site_name}"
  s3_bucket     = "${data.aws_s3_bucket.code_bucket.bucket}"
  s3_key        = "list-of-lists.zip"
  role          = aws_iam_role.lambda.arn
  architectures = ["arm64"]
  runtime       = "provided.al2023"
  handler       = "ignored"
  publish       = "false"
  description   = "Generate ${var.site_url}"
  timeout       = 5
  memory_size   = 128

  environment {
    variables = {
      LOL_SITE     = var.site_name
      LOL_SITE_URL = var.site_url
    }
  }
}

data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

data "aws_iam_policy_document" "github_update" {
  statement {
    actions = ["s3:PutObject"]
    resources = [
      "${aws_s3_bucket.generator.arn}/index.template",
      "${aws_s3_bucket.generator.arn}/${var.site_name}.json"
    ]
  }
}

resource "aws_iam_policy" "github_update" {
  name   = "${var.site_name}.github-update"
  policy = data.aws_iam_policy_document.github_update.json
}

resource "aws_iam_role" "github_update" {
  name = "${var.site_name}.github-update"

  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Principal = {
          Federated = "${data.aws_iam_openid_connect_provider.github.arn}"
        },
        Action = "sts:AssumeRoleWithWebIdentity",
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" : "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" : [
              "repo:jluszcz/ListOfLists-rs:*",
              "repo:${var.github_org}/${var.github_repo}:*"
            ]
          },
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "github_update" {
  role       = aws_iam_role.github_update.name
  policy_arn = aws_iam_policy.github_update.arn
}

data "aws_iam_policy_document" "github_deploy" {
  statement {
    actions = ["s3:PutObject"]
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
          Federated = "${data.aws_iam_openid_connect_provider.github.arn}"
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
