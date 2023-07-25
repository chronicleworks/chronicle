use metrics_exporter_prometheus::PrometheusHandle;

use crate::ApiError;

#[derive(Clone)]
pub enum Health {
    HealthEndpoint(PrometheusHandle),
    MetricsEndpoint(PrometheusHandle),
}

#[poem::async_trait]
impl poem::Endpoint for Health {
    type Output = poem::Response;

    async fn call(&self, _req: poem::Request) -> poem::Result<Self::Output> {
        match self {
            Health::HealthEndpoint(handle) => {
                let metrics = handle.render().to_string();

                let latest_round_trip_key = "latest_depth_charge_round_trip";
                let latest = match extract_latest_roundtrip(&metrics, latest_round_trip_key) {
                    Ok(value) => match value.parse::<f64>() {
                        Ok(value) => value,
                        Err(err) => return Ok(err.to_string().into()),
                    },
                    Err(err) => return Ok(err.to_string().into()),
                };

                let quantile = "0.999";
                let metric = match extract_value_for_quantile(&metrics, quantile) {
                    Ok(value) => match value.parse::<f64>() {
                        Ok(value) => value,
                        Err(err) => return Ok(err.to_string().into()),
                    },
                    Err(err) => return Ok(err.to_string().into()),
                };

                if latest < metric * 2.0 {
                    Ok(poem::Response::builder()
                        .status(poem::http::StatusCode::OK)
                        .body(format!(
                            "API is healthy! Latest round trip: {}ms, {}th percentile: {}ms",
                            latest, quantile, metric
                        )))
                } else {
                    Ok(poem::Response::builder()
                        .status(poem::http::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(format!(
                            "Health check failed! Latest round trip: {}ms, {}th percentile: {}ms",
                            latest, quantile, metric
                        )))
                }
            }
            Health::MetricsEndpoint(handle) => Ok(handle.render().into()),
        }
    }
}

fn extract_latest_roundtrip(metrics: &str, key: &str) -> Result<String, ApiError> {
    for line in metrics.lines() {
        if line.starts_with(key) {
            let value = line.trim_start_matches(key).trim().to_string();

            return Ok(value);
        }
    }

    Err(ApiError::MetricsLatestRoundTripNotFound)
}

fn extract_value_for_quantile(metrics: &str, quantile: &str) -> Result<String, ApiError> {
    let target_key = format!("depth_charge_round_trip{{quantile=\"{}\"}}", quantile);

    for line in metrics.lines() {
        if line.starts_with(&target_key) {
            let value = line.trim_start_matches(&target_key).trim().to_string();

            return Ok(value);
        }
    }

    Err(ApiError::MetricsQuantileNotFound {
        quantile: quantile.to_string(),
    })
}
