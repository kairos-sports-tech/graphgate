use std::collections::HashMap;

use anyhow::{Context, Result};
use graphgate_transports::CoordinatorImpl;
use k8s_openapi::api::core::v1::Service;
use kube::api::{ListParams, ObjectMeta};
use kube::{Api, Client};

const NAMESPACE_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/namespace";
const LABEL_GRAPHQL_SERVICE: &str = "graphgate.org/service";
const LABEL_GRAPHQL_PROTOCOL: &str = "graphgate.org/protocol";

fn get_label_value<'a>(meta: &'a ObjectMeta, name: &str) -> Option<&'a str> {
    meta.labels
        .iter()
        .flatten()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, value)| value.as_str())
}

pub async fn find_graphql_services() -> Result<HashMap<String, String>> {
    tracing::trace!("Find GraphQL services.");
    let client = Client::try_default()
        .await
        .context("Failed to create kube client.")?;

    let namespace =
        std::fs::read_to_string(NAMESPACE_PATH).unwrap_or_else(|_| "default".to_string());
    tracing::trace!(namespace = %namespace, "Get current namespace.");

    let mut graphql_services = HashMap::new();
    let services_api: Api<Service> = Api::namespaced(client, &namespace);

    tracing::trace!("List all services.");
    let services = services_api
        .list(&ListParams::default().labels(LABEL_GRAPHQL_SERVICE))
        .await
        .context("Failed to call list services api")?;

    for service in &services {
        if let Some(((host, service_name), protocol)) = service
            .metadata
            .name
            .as_deref()
            .zip(get_label_value(&service.metadata, LABEL_GRAPHQL_SERVICE))
            .zip(get_label_value(&service.metadata, LABEL_GRAPHQL_PROTOCOL))
        {
            for service_port in service
                .spec
                .iter()
                .map(|spec| spec.ports.iter())
                .flatten()
                .flatten()
            {
                graphql_services.insert(
                    service_name.to_string(),
                    format!("{}://{}:{}", protocol, host, service_port.port),
                );
            }
        }
    }

    Ok(graphql_services)
}

pub fn create_coordinator(graphql_services: &HashMap<String, String>) -> Result<CoordinatorImpl> {
    tracing::trace!(services = ?graphql_services, "Create coordinator.");
    let mut coordinator = CoordinatorImpl::default();
    for (service, url) in graphql_services {
        coordinator = coordinator.add_url(service, url)?;
    }
    Ok(coordinator)
}