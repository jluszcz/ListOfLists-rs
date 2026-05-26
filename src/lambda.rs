use anyhow::{Context, Result, anyhow};
use aws_config::ConfigLoader;
use aws_lambda_events::s3::S3Event;
use aws_sdk_cloudfront::Client as CloudFrontClient;
use aws_sdk_cloudfront::types::{InvalidationBatch, Paths};
use aws_sdk_s3::Client as S3Client;
use jluszcz_rust_utils::lambda;
use lambda_runtime::{LambdaEvent, service_fn};
use list_of_lists::{APP_NAME, generator, s3util};
use log::{debug, info, warn};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const MINIFY: bool = true;

static INVALIDATION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let func = service_fn(function);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn function(event: LambdaEvent<Value>) -> Result<Value, lambda_runtime::Error> {
    lambda::init(APP_NAME, module_path!(), false).await?;

    let generator_bucket = env::var(list_of_lists::GENERATOR_BUCKET_VAR)?;

    let aws_config = ConfigLoader::default().load().await;
    let s3_client = S3Client::new(&aws_config);
    let cloudfront_client = CloudFrontClient::new(&aws_config);

    let event: S3Event = serde_json::from_value(event.payload)?;

    let mut site_urls: Vec<String> = Vec::new();
    let mut regenerate_all = false;

    for record in event.records {
        let bucket = record.s3.bucket.name;
        let key = record.s3.object.key;
        if let (Some(bucket), Some(key)) = (bucket, key) {
            if key == generator::SITE_INDEX_TEMPLATE {
                info!("Regenerating all sites on update of {bucket}/{key}");
                regenerate_all = true;
                break;
            } else if let Some(site_url) = key.strip_suffix(".json") {
                info!("Will update {site_url} on update of {bucket}/{key}");
                site_urls.push(site_url.to_string());
            }
        }
    }

    if regenerate_all {
        site_urls = s3util::list_keys(&s3_client, &generator_bucket, ".json")
            .await?
            .into_iter()
            .filter_map(|k| k.strip_suffix(".json").map(String::from))
            .collect();
    }

    // Dedupe so duplicate S3 events don't trigger duplicate renders or invalidations.
    site_urls.sort();
    site_urls.dedup();

    if site_urls.is_empty() {
        return Ok(json!({}));
    }

    let template = generator::read_template(&generator_bucket, Some(&s3_client)).await?;
    let env = generator::build_environment(&template)?;

    let render_futures = site_urls.iter().map(|site_url| {
        let io = generator::Io::new(
            site_url.clone(),
            generator_bucket.clone(),
            Some(s3_client.clone()),
        );
        let env = &env;
        async move {
            info!("Updating {site_url}");
            generator::render_site(&io, env, site_url, MINIFY).await
        }
    });
    let render_results = futures::future::join_all(render_futures).await;

    let mut rendered_sites: Vec<String> = Vec::new();
    let mut render_failures = 0usize;
    for (site_url, result) in site_urls.iter().zip(render_results) {
        match result {
            Ok(()) => rendered_sites.push(site_url.clone()),
            Err(err) => {
                warn!("Failed to render {site_url}: {err:#}");
                render_failures += 1;
            }
        }
    }
    if rendered_sites.is_empty() {
        return Err(anyhow!("all {render_failures} site render(s) failed").into());
    }

    // Group rendered sites by distribution_id so we issue one invalidation per
    // distribution even if duplicate events or multiple aliases collapse onto
    // the same one. Lookups are serial because they share the cache mutex.
    let mut by_distribution: HashMap<String, Vec<String>> = HashMap::new();
    for site_url in &rendered_sites {
        match distribution_id_for_alias(&cloudfront_client, site_url).await {
            Ok(Some(distribution_id)) => by_distribution
                .entry(distribution_id)
                .or_default()
                .push(site_url.clone()),
            Ok(None) => warn!("No CloudFront distribution found with alias {site_url}"),
            Err(err) => warn!("CloudFront lookup failed for {site_url}: {err:#}"),
        }
    }

    let invalidation_futures = by_distribution.iter().map(|(distribution_id, sites)| {
        invalidate_distribution(&cloudfront_client, distribution_id, sites)
    });
    let results = futures::future::join_all(invalidation_futures).await;
    let mut invalidation_failures = 0usize;
    for ((distribution_id, _), result) in by_distribution.iter().zip(results) {
        if let Err(err) = result {
            warn!("CloudFront invalidation failed for distribution {distribution_id}: {err:#}");
            invalidation_failures += 1;
        }
    }

    info!(
        "rendered {}/{} sites; invalidated {}/{} distributions",
        rendered_sites.len(),
        site_urls.len(),
        by_distribution.len() - invalidation_failures,
        by_distribution.len(),
    );

    Ok(json!({}))
}

async fn invalidate_distribution(
    client: &CloudFrontClient,
    distribution_id: &str,
    sites: &[String],
) -> Result<()> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let counter = INVALIDATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let caller_reference = format!("list-of-lists-{distribution_id}-{nanos}-{counter}");

    let paths = Paths::builder()
        .quantity(1)
        .items("/index.html")
        .build()
        .context("build invalidation Paths")?;

    let batch = InvalidationBatch::builder()
        .paths(paths)
        .caller_reference(caller_reference)
        .build()
        .context("build InvalidationBatch")?;

    info!("Invalidating /index.html on distribution {distribution_id} for sites {sites:?}");
    client
        .create_invalidation()
        .distribution_id(distribution_id)
        .invalidation_batch(batch)
        .send()
        .await
        .with_context(|| format!("create_invalidation for distribution {distribution_id}"))?;

    Ok(())
}

// Cached per warm container: alias -> distribution id. Populated on first
// lookup and refreshed on miss so newly created distributions are picked up
// without a redeploy.
static DISTRIBUTION_CACHE: LazyLock<Mutex<Option<HashMap<String, String>>>> =
    LazyLock::new(|| Mutex::new(None));

async fn distribution_id_for_alias(
    client: &CloudFrontClient,
    alias: &str,
) -> Result<Option<String>> {
    // Hold the lock across the API call so concurrent callers on a cold cache
    // don't each fire their own list_distributions request.
    let mut cache = DISTRIBUTION_CACHE.lock().await;
    if let Some(map) = cache.as_ref()
        && let Some(id) = map.get(alias)
    {
        debug!("Distribution cache hit for {alias}");
        return Ok(Some(id.clone()));
    }

    debug!("Distribution cache miss for {alias}; refreshing");
    let fresh = list_distribution_aliases(client).await?;
    let result = fresh.get(alias).cloned();
    *cache = Some(fresh);
    Ok(result)
}

async fn list_distribution_aliases(client: &CloudFrontClient) -> Result<HashMap<String, String>> {
    let mut out: HashMap<String, String> = HashMap::new();
    let mut marker: Option<String> = None;
    loop {
        let mut req = client.list_distributions();
        if let Some(m) = marker {
            req = req.marker(m);
        }
        let response = req.send().await.context("list_distributions")?;
        let Some(list) = response.distribution_list else {
            break;
        };
        for item in list.items() {
            if let Some(aliases) = item.aliases.as_ref() {
                for alias in aliases.items() {
                    if let Some(existing) = out.insert(alias.clone(), item.id.clone())
                        && existing != item.id
                    {
                        warn!(
                            "CloudFront alias {alias} appears on multiple distributions ({existing} and {new}); using {new}",
                            new = item.id,
                        );
                    }
                }
            }
        }
        if list.is_truncated {
            marker = list.next_marker;
            if marker.is_none() {
                break;
            }
        } else {
            break;
        }
    }
    Ok(out)
}
