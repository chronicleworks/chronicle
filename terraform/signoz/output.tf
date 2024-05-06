output "otel_collector_service_name" {
  value = "${helm_release.signoz.name}-otel-collector"
  description = "The service name of the OpenTelemetry collector deployed with SigNoz."
}

output "otel_collector_service_url" {
  value = "http://${helm_release.signoz.name}-otel-collector.${var.signoz_namespace}.svc.cluster.local"
  description = "The URL to access the OpenTelemetry collector service."
}

output "otel_collector_service_port" {
  value = "4317"
  description = "The default gRPC port for the OpenTelemetry collector service."
}

output "signoz_frontend_service_name" {
  value = "${helm_release.signoz.name}-frontend"
  description = "The service name of the SigNoz frontend."
}

output "signoz_frontend_service_url" {
  value = "http://${helm_release.signoz.name}-frontend.${var.signoz_namespace}.svc.cluster.local"
  description = "The URL to access the SigNoz frontend service."
}

output "signoz_frontend_service_port" {
  value = "3000"
  description = "The default port for the SigNoz frontend service."
}



