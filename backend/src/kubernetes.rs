use crate::utils;
use std::path::Path;
use log::{error, info};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams, KubeObject},
    client::APIClient,
    config,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

const NAMESPACE : &str = "default";

fn error_to_string<T: std::fmt::Display>(err: T) -> String {
    format!("{}", err)
}

fn read_deployment(uuid: &str, image: &str) -> Result<Value, String> {
    utils::read(&Path::new("conf/deployment.yaml"))
      .map_err(error_to_string)
      .and_then(|s| serde_json::from_str(&s.replace("%IMAGE_NAME%", image).replace("%UUID%", uuid)).map_err(error_to_string))
}

fn read_service(uuid: &str, pod: &str) -> Result<Value, String> {
    utils::read(&Path::new("conf/service.yaml"))
      .map_err(error_to_string)
      .and_then(|s| serde_json::from_str(&s.replace("%POD_NAME%", pod).replacen("%UUID%", uuid, 2)).map_err(error_to_string))
}

fn deploy_pod(client: APIClient, image: &str) -> Result<String, String> {
    let uuid = format!("{}", Uuid::new_v4());
    let p: Value = read_deployment(&uuid, image)?;

    let params = PostParams::default();
    let pods = Api::v1Pod(client.clone()).within(NAMESPACE);
    let name = pods.create(&params, serde_json::to_vec(&p).map_err(error_to_string)?)
      .map(|o| o.metadata.name).map_err(error_to_string)?;

    let p2: Value = read_service(&uuid, &name)?;

    let services = Api::v1Service(client).within(NAMESPACE);
    services.create(&params, serde_json::to_vec(&p2).map_err(error_to_string)?)
      .map(|_| uuid).map_err(error_to_string)
}

fn list_by_selector<K: Clone + DeserializeOwned + KubeObject>(api: &Api<K>, selector: String) -> Result<Vec<K>, String> {
    let params = ListParams{ label_selector: Some(selector), ..ListParams::default()};
    api.list(&params).map(|l| l.items).map_err(|s| format!("Error {}", s))
}

fn undeploy_pod(client: APIClient, uuid: &str) -> Result<(), String> {
    let service_api = Api::v1Service(client.clone()).within(NAMESPACE);
    let selector = format!("app-uuid={}", uuid);
    let services = list_by_selector(&service_api, selector.clone())?;
    let service = services.first().ok_or(format!("No matching pod for {}", uuid))?;
    let params = DeleteParams::default();
    service_api.delete(&service.metadata.name, &params).map_err(|s| format!("Error {}", s))?;

    let pod_api = Api::v1Pod(client).within(NAMESPACE);
    let pods = list_by_selector(&pod_api, selector)?;
    let pod = pods.first().ok_or(format!("No matching pod for {}", uuid))?;
    pod_api.delete(&pod.metadata.name, &params).map_err(|s| format!("Error {}", s))?;

    Ok(())
}

fn get_service(client: APIClient, uuid: &str) -> Result<String, String> {
    let service = Api::v1Service(client).within(NAMESPACE);
    match service.get(uuid) {
        Ok(o) => {
            // Find more details here:
            // * https://docs.rs/k8s-openapi/0.5.1/k8s_openapi/api/core/v1/struct.ServiceStatus.html
            // * https://docs.rs/k8s-openapi/0.5.1/k8s_openapi/api/core/v1/struct.ServiceSpec.html
            if let (Some(status), Some(ports)) = (o.status, o.spec.ports) {
                if let Some (ingress) = status.load_balancer.unwrap().ingress {
                  Ok(format!("http://{}:{}", ingress[0].ip.as_ref().unwrap(), 8080).to_string()) // TODO only the proper port (correct name)
                } else {
                    Ok("".to_string())
                }
            } else {
                Err("Failed to access service endpoint".to_string())
            }
        }
        Err(e) => {
            error!("Err {}", e);
            Err("Failed to access service endpoint".to_string())
        },
    }
}

fn create_client() -> kube::Result<APIClient> {
    let config = config::incluster_config()
      .unwrap_or_else(|_| {
        info!("Use local configuration");
        config::load_kube_config().unwrap()  
      });
    Ok(APIClient::new(config))
}

pub fn deploy(image: & str) -> Result<String, String> {
    create_client().map_err(error_to_string).and_then(|c| deploy_pod(c, image))
}

pub fn undeploy(uuid: & str) -> Result<(), String> {
    create_client().map_err(error_to_string).and_then(|c| undeploy_pod(c, uuid))
}

pub fn url(uuid: & str) -> Result<String, String> {
    create_client().map_err(error_to_string).and_then(|c| get_service(c, uuid))
}