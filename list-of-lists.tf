terraform {
  backend "s3" {}
}

# Sourced from environment variables named TF_VAR_${VAR_NAME}
variable "aws_acct_id" {
}

variable "site_name" {
}

variable "site_url" {
}

variable "db_access_key" {
}

variable "db_file_path" {
}

variable "aws_region" {
  type    = string
  default = "us-east-2"
}

provider "aws" {
  region  = var.aws_region
}

provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

resource "aws_s3_bucket" "site" {
  bucket = var.site_url
  website {
    index_document = "index.html"
  }
}

resource "aws_s3_bucket_public_access_block" "site" {
  bucket = aws_s3_bucket.site.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

data "aws_iam_policy_document" "site" {
  statement {
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.site.arn}/*"]

    principals {
      type        = "AWS"
      identifiers = [aws_cloudfront_origin_access_identity.site_distribution_oai.iam_arn]
    }
  }

  statement {
    actions   = ["s3:ListBucket"]
    resources = [aws_s3_bucket.site.arn]

    principals {
      type        = "AWS"
      identifiers = [aws_cloudfront_origin_access_identity.site_distribution_oai.iam_arn]
    }
  }
}

resource "aws_s3_bucket_policy" "site" {
  bucket = aws_s3_bucket.site.id
  policy = data.aws_iam_policy_document.site.json
}

resource "aws_s3_bucket_object" "favicon" {
  bucket = aws_s3_bucket.site.id
  key    = "images/favicon.ico"
  source = "images/${var.site_name}.ico"
  etag   = filemd5("images/${var.site_name}.ico")
}

resource "aws_s3_bucket_object" "card_image" {
  count  = fileexists("images/${var.site_name}_card.png") ? 1 : 0
  bucket = aws_s3_bucket.site.id
  key    = "images/card.png"
  source = "images/${var.site_name}_card.png"
  etag   = filemd5("images/${var.site_name}_card.png")
}

resource "aws_acm_certificate" "cert" {
  provider                  = aws.us_east_1
  domain_name               = var.site_url
  subject_alternative_names = ["www.${var.site_url}"]
  validation_method         = "DNS"
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
  records         = [each.value.record]
  ttl             = 60
}

resource "aws_cloudfront_origin_access_identity" "site_distribution_oai" {
}

resource "aws_cloudfront_distribution" "site" {
  origin {
    domain_name = aws_s3_bucket.site.bucket_domain_name
    origin_id   = "site_bucket_origin"

    s3_origin_config {
      origin_access_identity = aws_cloudfront_origin_access_identity.site_distribution_oai.cloudfront_access_identity_path
    }
  }

  enabled             = true
  is_ipv6_enabled     = true
  default_root_object = "index.html"

  aliases = ["www.${var.site_url}", var.site_url]

  default_cache_behavior {
    allowed_methods  = ["GET", "HEAD"]
    cached_methods   = ["GET", "HEAD"]
    target_origin_id = "site_bucket_origin"

    forwarded_values {
      query_string = false

      cookies {
        forward = "none"
      }
    }

    viewer_protocol_policy = "redirect-to-https"
    min_ttl                = 0
    default_ttl            = 3600
    max_ttl                = 86400
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
    minimum_protocol_version = "TLSv1"
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

resource "aws_s3_bucket_object" "index_template" {
  bucket = aws_s3_bucket.generator.id
  key    = "index.template"
  source = "index.template"
  etag   = filemd5("index.template")
}

resource "aws_route53_zone" "zone" {
  name = var.site_url
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

resource "aws_cloudwatch_log_group" "updater" {
  name              = "/aws/lambda/${var.site_name}-updater"
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

resource "aws_iam_role" "lambda_generator" {
  name               = "lambda.${var.site_name}.generator"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role" "lambda_updater" {
  name               = "lambda.${var.site_name}.updater"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

data "aws_iam_policy_document" "cw_logs" {
  statement {
    actions   = ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents", "logs:Describe*"]
    resources = ["arn:aws:logs:${var.aws_region}:${var.aws_acct_id}:*"]
  }
}

resource "aws_iam_policy" "cw_logs" {
  policy = data.aws_iam_policy_document.cw_logs.json
}

resource "aws_iam_role_policy_attachment" "generator_cw_logs" {
  role       = aws_iam_role.lambda_generator.name
  policy_arn = aws_iam_policy.cw_logs.arn
}

resource "aws_iam_role_policy_attachment" "updater_cw_logs" {
  role       = aws_iam_role.lambda_updater.name
  policy_arn = aws_iam_policy.cw_logs.arn
}

data "aws_iam_policy_document" "generator_s3" {
  statement {
    actions   = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.site.arn}/index.html"]
  }

  statement {
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.site.arn}/images/card.png"]
  }

  statement {
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.generator.arn}/*"]
  }
}

resource "aws_iam_policy" "generator_s3" {
  policy = data.aws_iam_policy_document.generator_s3.json
}

resource "aws_iam_role_policy_attachment" "generator_s3" {
  role       = aws_iam_role.lambda_generator.name
  policy_arn = aws_iam_policy.generator_s3.arn
}

data "aws_iam_policy_document" "updater_s3" {
  statement {
    actions   = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.generator.arn}/${var.site_name}.json"]
  }

  statement {
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.generator.arn}/*"]
  }
}

resource "aws_iam_policy" "updater_s3" {
  policy = data.aws_iam_policy_document.updater_s3.json
}

resource "aws_iam_role_policy_attachment" "updater_s3" {
  role       = aws_iam_role.lambda_updater.name
  policy_arn = aws_iam_policy.updater_s3.arn
}

resource "aws_s3_bucket_notification" "generator_notification" {
  bucket = aws_s3_bucket.generator.id

  lambda_function {
    lambda_function_arn = aws_lambda_function.lambda_generator.arn
    events              = ["s3:ObjectCreated:Put"]
  }
}

resource "aws_lambda_permission" "generator_allow_bucket" {
  statement_id  = "${var.site_name}-AllowExecutionFromS3Bucket"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda_generator.arn
  principal     = "s3.amazonaws.com"
  source_arn    = aws_s3_bucket.generator.arn
}

variable "lambda_filename" {
  type    = string
  default = "listoflists.zip"
}

resource "aws_lambda_function" "lambda_generator" {
  filename         = var.lambda_filename
  function_name    = "${var.site_name}-generator"
  role             = aws_iam_role.lambda_generator.arn
  handler          = "generator.lambda_handler"
  source_code_hash = filebase64sha256(var.lambda_filename)
  runtime          = "python3.8"
  publish          = "false"
  description      = "Generate ${var.site_url}"
  timeout          = 5
  memory_size      = 256

  environment {
    variables = {
      SITE     = var.site_name
      SITE_URL = var.site_url
    }
  }
}

resource "aws_cloudwatch_event_rule" "updater_schedule" {
  name                = "${var.site_name}-updater-schedule"
  description         = "Update ${var.site_name} periodically"
  schedule_expression = "cron(0 5 * * ? *)"
}

resource "aws_lambda_permission" "generator_allow_cloudwatch" {
  statement_id   = "${var.site_name}-AllowExecutionFromCloudWatch"
  action         = "lambda:InvokeFunction"
  function_name  = aws_lambda_function.lambda_updater.arn
  principal      = "events.amazonaws.com"
  source_arn     = aws_cloudwatch_event_rule.updater_schedule.arn
}

resource "aws_cloudwatch_event_target" "updater_event_target" {
  target_id = var.site_name
  rule      = aws_cloudwatch_event_rule.updater_schedule.name
  arn       = aws_lambda_function.lambda_updater.arn
}

resource "aws_lambda_function" "lambda_updater" {
  filename         = var.lambda_filename
  function_name    = "${var.site_name}-updater"
  role             = aws_iam_role.lambda_updater.arn
  handler          = "updater.lambda_handler"
  source_code_hash = filebase64sha256(var.lambda_filename)
  runtime          = "python3.8"
  publish          = "false"
  description      = "Update ${var.site_url}"
  timeout          = 5
  memory_size      = 256

  environment {
    variables = {
      SITE          = var.site_name
      SITE_URL      = var.site_url
      DB_ACCESS_KEY = var.db_access_key
      DB_FILE_PATH  = var.db_file_path
    }
  }
}

