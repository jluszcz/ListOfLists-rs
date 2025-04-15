terraform {
  backend "s3" {}
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

resource "aws_s3_object" "favicon" {
  count  = fileexists("buckets/${var.site_url}/images/favicon.ico") ? 1 : 0
  bucket = aws_s3_bucket.site.id
  key    = "images/favicon.ico"
  source = "buckets/${var.site_url}/images/favicon.ico"
  etag = filemd5("buckets/${var.site_url}/images/favicon.ico")
}

resource "aws_s3_object" "card_image" {
  count  = fileexists("buckets/${var.site_url}/images/card.png") ? 1 : 0
  bucket = aws_s3_bucket.site.id
  key    = "images/card.png"
  source = "buckets/${var.site_url}/images/card.png"
  etag = filemd5("buckets/${var.site_url}/images/card.png")
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
}

resource "aws_s3_object" "index_template" {
  bucket = aws_s3_bucket.generator.id
  key    = "index.template"
  source = "index.template"
  etag = filemd5("index.template")
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

resource "aws_cloudwatch_log_group" "generator" {
  name              = "/aws/lambda/${var.site_name}-generator"
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

resource "aws_iam_role" "lambda_generator" {
  name               = "lambda.${var.site_name}.generator"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

data "aws_iam_policy_document" "cw_logs" {
  statement {
    actions = ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents", "logs:Describe*"]
    resources = ["arn:aws:logs:${var.aws_region}:${var.aws_acct_id}:*"]
  }
}

resource "aws_iam_policy" "cw_logs" {
  name   = "${var.site_name}.cw_logs"
  policy = data.aws_iam_policy_document.cw_logs.json
}

resource "aws_iam_role_policy_attachment" "generator_cw_logs" {
  role       = aws_iam_role.lambda_generator.name
  policy_arn = aws_iam_policy.cw_logs.arn
}

data "aws_iam_policy_document" "generator_s3" {
  statement {
    actions = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.site.arn}/index.html"]
  }

  statement {
    actions = ["s3:GetObject", "s3:HeadObject"]
    resources = ["${aws_s3_bucket.site.arn}/images/card.png"]
  }

  statement {
    actions = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.generator.arn}/*"]
  }
}

resource "aws_iam_policy" "generator_s3" {
  name   = "${var.site_name}.generator_s3"
  policy = data.aws_iam_policy_document.generator_s3.json
}

resource "aws_iam_role_policy_attachment" "generator_s3" {
  role       = aws_iam_role.lambda_generator.name
  policy_arn = aws_iam_policy.generator_s3.arn
}

resource "aws_s3_bucket_notification" "generator_notification" {
  bucket = aws_s3_bucket.generator.id

  lambda_function {
    lambda_function_arn = aws_lambda_function.lambda_generator.arn
    events = ["s3:ObjectCreated:Put"]
  }
}

resource "aws_lambda_permission" "generator_allow_bucket" {
  statement_id  = "${var.site_name}-AllowExecutionFromS3Bucket"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda_generator.arn
  principal     = "s3.amazonaws.com"
  source_arn    = aws_s3_bucket.generator.arn
}

resource "aws_lambda_function" "lambda_generator" {
  function_name = "${var.site_name}-generator"
  s3_bucket     = var.code_bucket
  s3_key        = "generator.zip"
  role          = aws_iam_role.lambda_generator.arn
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

data "aws_iam_policy_document" "github" {
  statement {
    actions = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.generator.arn}/${var.site_name}.json"]
  }
}

resource "aws_iam_policy" "github" {
  name   = "${var.site_name}.github"
  policy = data.aws_iam_policy_document.github.json
}

resource "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"

  client_id_list = ["sts.amazonaws.com"]

  thumbprint_list = ["6938fd4d98bab03faadb97b34396831e3780aea1"]
}

resource "aws_iam_role" "github" {
  name = "github.${var.site_name}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Principal = {
          Federated = aws_iam_openid_connect_provider.github.arn
        },
        Action = "sts:AssumeRoleWithWebIdentity",
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" : "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" : "repo:${var.github_org}/${var.github_repo}:*"
          },
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "github" {
  role       = aws_iam_role.github.name
  policy_arn = aws_iam_policy.github.arn
}
