resource "helm_release" "signoz" {
  name       = "signoz"
  repository = var.signoz_helm_repository
  chart      = var.signoz_helm_chart_name
  namespace  = var.signoz_namespace
  create_namespace = true

  set {
    name  = "frontend.replicaCount"
    value = var.signoz_fe_replicas
  }

  set {
    name  = "queryService.replicaCount"
    value = var.signoz_query_replicas
  }

  set {
    name  = "storage.size"
    value = var.signoz_storage_size
  }
}

resource "helm_release" "opentelemetry_operator" {
  name       = "opentelemetry-operator"
  repository = var.opentelemetry_operator_helm_repository
  chart      = var.opentelemetry_operator_helm_chart_name
  namespace  = var.signoz_namespace
  create_namespace = true


  set {
    name = "manager.collectorImage.repository"
    value = "ghcr.io/open-telemetry/opentelemetry-collector-releases/opentelemetry-collector-contrib"
  }
}

resource "helm_release" "cert_manager" {
  name       = "cert-manager"
  repository = "https://charts.jetstack.io"
  chart      = "cert-manager"
  namespace  = "cert-manager"
  create_namespace = true
  version    = "v1.14.5"

  set {
    name  = "installCRDs"
    value = "true"
  }
}



variable "namespaces" {
  type        = list(string)
  description = "List of Kubernetes namespaces to deploy OpenTelemetry Collector into"
  default     = ["chronicle-substrate","chronicle","vault"]
}

resource "kubernetes_namespace" "namespaces" {
  for_each = toset(var.namespaces)
  metadata {
    name = each.key
  }
}

resource "kubernetes_manifest" "prometheus_service_monitor_crd" {
  manifest = {
    apiVersion = "apiextensions.k8s.io/v1"
    kind       = "CustomResourceDefinition"
    metadata = {
      name = "servicemonitors.monitoring.coreos.com"
      annotations = {
        "controller-gen.kubebuilder.io/version" = "v0.14.0"
        "operator.prometheus.io/version"        = "0.74.0"
      }
    }
    spec = {
      group = "monitoring.coreos.com"
      names = {
        categories = ["prometheus-operator"]
        kind       = "ServiceMonitor"
        listKind   = "ServiceMonitorList"
        plural     = "servicemonitors"
        shortNames = ["smon"]
        singular   = "servicemonitor"
      }
      scope = "Namespaced"
      versions = [
        {
          name = "v1"
          schema = {
            openAPIV3Schema = {
              description = "ServiceMonitor defines monitoring for a set of services."
              properties = {
                apiVersion = {
                  description = "APIVersion defines the versioned schema of this representation of an object."
                  type        = "string"
                }
                kind = {
                  description = "Kind is a string value representing the REST resource this object represents."
                  type        = "string"
                }
                metadata = {
                  type = "object"
                }
                spec = {
                  description = "Specification of desired Service selection for target discovery by Prometheus."
                  properties = {
                    attachMetadata = {
                      description = "`attachMetadata` defines additional metadata which is added to the discovered targets."
                      properties = {
                        node = {
                          description = "When set to true, Prometheus must have the `get` permission on the `Nodes` objects."
                          type        = "boolean"
                        }
                      }
                      type = "object"
                    }
                    bodySizeLimit = {
                      description = "When defined, bodySizeLimit specifies a job level limit on the size of uncompressed response body that will be accepted by Prometheus."
                      pattern     = "(^0|([0-9]*[.])?[0-9]+((K|M|G|T|E|P)i?)?B)$"
                      type        = "string"
                    }
                    endpoints = {
                      description = "List of endpoints part of this ServiceMonitor."
                      items = {
                        description = "Endpoint defines an endpoint serving Prometheus metrics to be scraped by Prometheus."
                        properties = {
                          authorization = {
                            description = "`authorization` configures the Authorization header credentials to use when scraping the target."
                            properties = {
                              credentials = {
                                description = "Selects a key of a Secret in the namespace that contains the credentials for authentication."
                                properties = {
                                  key = {
                                    description = "The key of the secret to select from. Must be a valid secret key."
                                    type        = "string"
                                  }
                                  name = {
                                    description = "Name of the referent."
                                    type        = "string"
                                  }
                                  optional = {
                                    description = "Specify whether the Secret or its key must be defined"
                                    type        = "boolean"
                                  }
                                }
                                required = ["key"]
                                type     = "object"
                              }
                              type = {
                                description = "Defines the authentication type. The value is case-insensitive."
                                type        = "string"
                              }
                            }
                            type = "object"
                          }
                          basicAuth = {
                            description = "`basicAuth` configures the Basic Authentication credentials to use when scraping the target."
                            properties = {
                              password = {
                                description = "`password` specifies a key of a Secret containing the password for authentication."
                                properties = {
                                  key = {
                                    description = "The key of the secret to select from. Must be a valid secret key."
                                    type        = "string"
                                  }
                                  name = {
                                    description = "Name of the referent."
                                    type        = "string"
                                  }
                                  optional = {
                                    description = "Specify whether the Secret or its key must be defined"
                                    type        = "boolean"
                                  }
                                }
                                required = ["key"]
                                type     = "object"
                              }
                              username = {
                                description = "`username` specifies a key of a Secret containing the username for authentication."
                                properties = {
                                  key = {
                                    description = "The key of the secret to select from. Must be a valid secret key."
                                    type        = "string"
                                  }
                                  name = {
                                    description = "Name of the referent."
                                    type        = "string"
                                  }
                                  optional = {
                                    description = "Specify whether the Secret or its key must be defined"
                                    type        = "boolean"
                                  }
                                }
                                required = ["key"]
                                type     = "object"
                              }
                            }
                            type = "object"
                          }
                          bearerTokenFile = {
                            description = "File to read bearer token for scraping the target."
                            type        = "string"
                          }
                          bearerTokenSecret = {
                            description = "`bearerTokenSecret` specifies a key of a Secret containing the bearer token for scraping targets."
                            properties = {
                              key = {
                                description = "The key of the secret to select from. Must be a valid secret key."
                                type        = "string"
                              }
                              name = {
                                description = "Name of the referent."
                                type        = "string"
                              }
                              optional = {
                                description = "Specify whether the Secret or its key must be defined"
                                type        = "boolean"
                              }
                            }
                            required = ["key"]
                            type     = "object"
                          }
                          enableHttp2 = {
                            description = "`enableHttp2` can be used to disable HTTP2 when scraping the target."
                            type        = "boolean"
                          }
                          filterRunning = {
                            description = "When true, the pods which are not running are dropped during the target discovery."
                            type        = "boolean"
                          }
                          followRedirects = {
                            description = "`followRedirects` defines whether the scrape requests should follow HTTP 3xx redirects."
                            type        = "boolean"
                          }
                          honorLabels = {
                            description = "When true, `honorLabels` preserves the metric's labels when they collide with the target's labels."
                            type        = "boolean"
                          }
                          honorTimestamps = {
                            description = "`honorTimestamps` controls whether Prometheus preserves the timestamps when exposed by the target."
                            type        = "boolean"
                          }
                          interval = {
                            description = "Interval at which metrics should be scraped."
                            type        = "string"
                          }
                          metricRelabelings = {
                            description = "MetricRelabelConfigs to apply to samples before ingestion."
                            items = {
                              description = "MetricRelabelConfig allows to relabel samples before ingestion."
                              properties = {
                                action = {
                                  description = "Action to perform based on regex matching."
                                  type        = "string"
                                }
                                regex = {
                                  description = "Regex to match against."
                                  type        = "string"
                                }
                                replacement = {
                                  description = "Replacement value for regex matches."
                                  type        = "string"
                                }
                              }
                              required = ["action", "regex", "replacement"]
                              type     = "object"
                            }
                            type = "array"
                          }
                          path = {
                            description = "HTTP path to scrape for metrics."
                            type        = "string"
                          }
                          port = {
                            description = "Name of the port to scrape metrics from."
                            type        = "string"
                          }
                          proxyUrl = {
                            description = "Optional URL of a proxy server to use for scraping."
                            type        = "string"
                          }
                          relabelings = {
                            description = "RelabelConfigs to apply to samples before scraping."
                            items = {
                              description = "RelabelConfig allows to relabel samples before scraping."
                              properties = {
                                action = {
                                  description = "Action to perform based on regex matching."
                                  type        = "string"
                                }
                                regex = {
                                  description = "Regex to match against."
                                  type        = "string"
                                }
                                replacement = {
                                  description = "Replacement value for regex matches."
                                  type        = "string"
                                }
                              }
                              required = ["action", "regex", "replacement"]
                              type     = "object"
                            }
                            type = "array"
                          }
                          scheme = {
                            description = "HTTP scheme to use for scraping."
                            type        = "string"
                          }
                          scrapeTimeout = {
                            description = "Timeout after which the scrape is ended."
                            type        = "string"
                          }
                          targetPort = {
                            description = "Name or number of the port to scrape metrics from."
                            type        = "string"
                          }
                          tlsConfig = {
                            description = "TLS configuration to use when scraping the target."
                            properties = {
                              caFile = {
                                description = "Path to the CA cert file in the Prometheus container for the targets."
                                type        = "string"
                              }
                              caSecret = {
                                description = "Secret containing the CA cert file for the targets."
                                properties = {
                                  key = {
                                    description = "The key of the secret to select from. Must be a valid secret key."
                                    type        = "string"
                                  }
                                  name = {
                                    description = "Name of the referent."
                                    type        = "string"
                                  }
                                  optional = {
                                    description = "Specify whether the Secret or its key must be defined"
                                    type        = "boolean"
                                  }
                                }
                                required = ["key"]
                                type     = "object"
                              }
                              certFile = {
                                description = "Path to the client cert file in the Prometheus container for the targets."
                                type        = "string"
                              }
                              insecureSkipVerify = {
                                description = "Disable target certificate validation."
                                type        = "boolean"
                              }
                              keyFile = {
                                description = "Path to the client key file in the Prometheus container for the targets."
                                type        = "string"
                              }
                              keySecret = {
                                description = "Secret containing the client key file for the targets."
                                properties = {
                                  key = {
                                    description = "The key of the secret to select from. Must be a valid secret key."
                                    type        = "string"
                                  }
                                  name = {
                                    description = "Name of the referent."
                                    type        = "string"
                                  }
                                  optional = {
                                    description = "Specify whether the Secret or its key must be defined"
                                    type        = "boolean"
                                  }
                                }
                                required = ["key"]
                                type     = "object"
                              }
                              serverName = {
                                description = "Used to verify the hostname for the targets."
                                type        = "string"
                              }
                            }
                            type = "object"
                          }
                          trackTimestampsStaleness = {
                            description = "`trackTimestampsStaleness` defines whether Prometheus tracks staleness of the metrics that have an explicit timestamp present in scraped data."
                            type        = "boolean"
                          }
                        }
                        type = "object"
                      }
                      type = "array"
                    }
                    jobLabel = {
                      description = "`jobLabel` selects the label from the associated Kubernetes `Service` object which will be used as the `job` label for all metrics."
                      type        = "string"
                    }
                    keepDroppedTargets = {
                      description = "Per-scrape limit on the number of targets dropped by relabeling that will be kept in memory. 0 means no limit."
                      format      = "int64"
                      type        = "integer"
                    }
                    labelLimit = {
                      description = "Per-scrape limit on number of labels that will be accepted for a sample."
                      format      = "int64"
                      type        = "integer"
                    }
                    labelNameLengthLimit = {
                      description = "Per-scrape limit on length of labels name that will be accepted for a sample."
                      format      = "int64"
                      type        = "integer"
                    }
                    labelValueLengthLimit = {
                      description = "Per-scrape limit on length of labels value that will be accepted for a sample."
                      format      = "int64"
                      type        = "integer"
                    }
                    namespaceSelector = {
                      description = "Selector to select which namespaces the Kubernetes `Endpoints` objects are discovered from."
                      properties = {
                        any = {
                          description = "Boolean describing whether all namespaces are selected in contrast to a list restricting them."
                          type        = "boolean"
                        }
                        matchNames = {
                          description = "List of namespace names to select from."
                          items = {
                            type = "string"
                          }
                          type = "array"
                        }
                      }
                      type = "object"
                    }
                    podTargetLabels = {
                      description = "`podTargetLabels` defines the labels which are transferred from the associated Kubernetes `Pod` object onto the ingested metrics."
                      items = {
                        type = "string"
                      }
                      type = "array"
                    }
                    sampleLimit = {
                      description = "`sampleLimit` defines a per-scrape limit on the number of scraped samples that will be accepted."
                      format      = "int64"
                      type        = "integer"
                    }
                    scrapeClass = {
                      description = "The scrape class to apply."
                      minLength   = 1
                      type        = "string"
                    }
                    scrapeProtocols = {
                      description = "`scrapeProtocols` defines the protocols to negotiate during a scrape. It tells clients the protocols supported by Prometheus in order of preference."
                      items = {
                        description = "ScrapeProtocol represents a protocol used by Prometheus for scraping metrics."
                        enum = [
                          "PrometheusProto",
                          "OpenMetricsText0.0.1",
                          "OpenMetricsText1.0.0",
                          "PrometheusText0.0.4"
                        ]
                        type = "string"
                      }
                      type = "array"
                    }
                    selector = {
                      description = "Label selector to select the Kubernetes `Endpoints` objects."
                      properties = {
                        matchExpressions = {
                          description = "matchExpressions is a list of label selector requirements. The requirements are ANDed."
                          items = {
                            description = "A label selector requirement is a selector that contains values, a key, and an operator that relates the key and values."
                            properties = {
                              key = {
                                description = "key is the label key that the selector applies to."
                                type        = "string"
                              }
                              operator = {
                                description = "operator represents a key's relationship to a set of values. Valid operators are In, NotIn, Exists and DoesNotExist."
                                type        = "string"
                              }
                              values = {
                                description = "values is an array of string values. If the operator is In or NotIn, the values array must be non-empty. If the operator is Exists or DoesNotExist, the values array must be empty."
                                items = {
                                  type = "string"
                                }
                                type = "array"
                              }
                            }
                            required = ["key", "operator"]
                            type     = "object"
                          }
                          type = "array"
                        }
                        matchLabels = {
                          additionalProperties = {
                            type = "string"
                          }
                          description = "matchLabels is a map of {key,value} pairs. A single {key,value} in the matchLabels map is equivalent to an element of matchExpressions, whose key field is `key`, the operator is `In`, and the values array contains only `value`."
                          type        = "object"
                        }
                      }
                      type = "object"
                    }
                    targetLabels = {
                      description = "`targetLabels` defines the labels which are transferred from the associated Kubernetes `Service` object onto the ingested metrics."
                      items = {
                        type = "string"
                      }
                      type = "array"
                    }
                    targetLimit = {
                      description = "`targetLimit` defines a limit on the number of scraped targets that will be accepted."
                      format      = "int64"
                      type        = "integer"
                    }
                  }
                  required = ["selector"]
                  type     = "object"
                }
              }
              required = ["spec"]
              type     = "object"
            }
          }
          served  = true
          storage = true
        }
      ]
    }
  }
}


resource "kubernetes_manifest" "opentelemetry_collector" {
  count = length(var.namespaces)
  manifest = {
    apiVersion = "opentelemetry.io/v1beta1"
    kind       = "OpenTelemetryCollector"
    metadata = {
      name      = "opentelemetry-collector-injected"
      namespace = var.namespaces[count.index]
    }
    spec = {
      mode   = "sidecar"
      config = {
        receivers = {
          otlp = {
            protocols = {
              grpc = {
                endpoint = "0.0.0.0:4317"
              }
              http = {
                endpoint = "0.0.0.0:4318"
              }
            }
          }
          jaeger = {
            protocols = {
              grpc = {
                endpoint = "0.0.0.0:14250"
              }
            }
          }
          prometheus = {
            config = {
              scrape_configs = [
                {
                  job_name       = "opentelemetry-collector"
                  scrape_interval = "10s"
                  static_configs = [
                    {
                      targets = ["$${env.MY_POD_IP}:8888"]
                    }
                  ]
                }
              ]
            }
          }
        }
        processors = {
          memory_limiter = {
            check_interval        = "1s"
            limit_percentage      = 75
            spike_limit_percentage = 15
          }
          batch = {
            send_batch_size = 10000
            timeout         = "10s"
          }
        }
        exporters = {
          otlp = {
            endpoint = "signoz-otel-collector.${var.signoz_namespace}.svc.cluster.local:4317"
            tls = {
              insecure = true
            }
          }
        }
        service = {
          pipelines = {
            logs = {
              exporters  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              receivers  = ["otlp"]
            }
            metrics = {
              exporters  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              receivers  = ["otlp", "prometheus"]
            }
            traces = {
              exporters  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              receivers  = ["otlp","jaeger"]
            }
          }
          telemetry = {
            metrics = {
              address = "$${env.MY_POD_IP}:8888"
            }
          }
        }
      }
    }
  }
}

