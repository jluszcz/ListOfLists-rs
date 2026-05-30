#!/usr/bin/env bash
#
# Scaffold a local, gitignored per-site Terraform workspace under sites/<site_url>/.
#
# The repo itself stays generic: site identities live only in these gitignored workspaces,
# never in committed code. Adding a site = run this script; no repo changes required.
#
# Usage:
#   scripts/new-site.sh <site_url> <github_org> <github_repo> [site_name]
#
#   site_name defaults to <site_url> with dots removed (e.g. list-of-l.ist -> listoflist).
#
# Example:
#   scripts/new-site.sh list-of-l.ist jluszcz ListOfL.ist

set -euo pipefail

# Backend state bucket/region — matches shared/main.tf. Override via env if needed.
STATE_BUCKET="${TF_STATE_BUCKET:-jluszcz-tf-state}"
STATE_REGION="${TF_STATE_REGION:-us-east-2}"

if [ "$#" -lt 3 ]; then
  echo "usage: $0 <site_url> <github_org> <github_repo> [site_name]" >&2
  exit 1
fi

site_url="$1"
github_org="$2"
github_repo="$3"
site_name="${4:-$(printf '%s' "$site_url" | tr -d '.')}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$repo_root/sites/$site_url"

if [ -e "$dir" ]; then
  echo "error: $dir already exists" >&2
  exit 1
fi

mkdir -p "$dir"

cat > "$dir/main.tf" <<EOF
terraform {
  backend "s3" {
    bucket       = "$STATE_BUCKET"
    key          = "list-of-lists/sites/$site_url"
    region       = "$STATE_REGION"
    use_lockfile = true
  }
}

provider "aws" {
  region = "$STATE_REGION"
}

provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"
}

module "site" {
  source = "../../site-module"

  providers = {
    aws           = aws
    aws.us_east_1 = aws.us_east_1
  }

  site_url    = "$site_url"
  site_name   = "$site_name"
  github_org  = "$github_org"
  github_repo = "$github_repo"
}
EOF

cat > "$dir/.envrc" <<EOF
# direnv: $site_url site context (gitignored). Terraform config lives in ./main.tf;
# these LOL_* vars are only for running the generator locally (cargo run --bin main).
export LOL_SITE_URL="$site_url"
export LOL_SITE_NAME="$site_name"
EOF

echo "Created sites/$site_url/ (gitignored):"
echo "  main.tf   backend key=list-of-lists/sites/$site_url, module \"site\" -> ../../site-module"
echo "  .envrc    LOL_SITE_URL=$site_url, LOL_SITE_NAME=$site_name"
echo
echo "Next:"
echo "  cd sites/$site_url"
echo "  direnv allow"
echo "  terraform init"
echo "  terraform apply        # creates the site (or use an imports.tf to adopt existing resources)"
