output "generator_bucket_name" {
  value = aws_s3_bucket.generator.bucket
}

output "generator_bucket_arn" {
  value = aws_s3_bucket.generator.arn
}
