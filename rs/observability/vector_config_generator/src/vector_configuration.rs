use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use service_discovery::{job_types::JobType, TargetGroup};
use url::Url;

use crate::JobParameters;

const IC_NAME: &str = "ic";
const IC_NODE: &str = "ic_node";
const IC_SUBNET: &str = "ic_subnet";

// NOTE: Those structures are tightly coupled with the use we want out of them
// for metrics, meaning adding labels and creating prometheus scraper sources.
// We might want to make those more general, so that we can use a simple configuration
// to tell the generator what we want as an input and as a result.
// This needs to be refined further

#[derive(Debug, Serialize)]
pub struct VectorServiceDiscoveryConfigEnriched {
    sources: HashMap<String, VectorSource>,
    transforms: HashMap<String, VectorTransform>,
}

impl VectorServiceDiscoveryConfigEnriched {
    fn new() -> Self {
        Self {
            sources: HashMap::new(),
            transforms: HashMap::new(),
        }
    }

    pub fn from_target_groups_with_job(
        tgs: BTreeSet<TargetGroup>,
        job: &JobType,
        job_parameters: &JobParameters,
        scrape_interval: u64,
        proxy_url: Option<Url>,
    ) -> Self {
        let mut config = Self::new();
        for tg in tgs {
            config.add_target_group(tg, job, job_parameters, scrape_interval, proxy_url.clone())
        }
        config
    }

    fn add_target_group(
        &mut self,
        target_group: TargetGroup,
        job: &JobType,
        job_parameters: &JobParameters,
        scrape_interval: u64,
        proxy_url: Option<Url>,
    ) {
        let key = target_group
            .targets
            .iter()
            .map(|t| t.to_string())
            .next() // Only take the first one here. Might cause some issues
            .unwrap();
        self.sources.insert(
            key.clone() + "-source",
            VectorSource::from_target_group_with_job(
                target_group.clone(),
                job,
                job_parameters,
                scrape_interval,
                proxy_url,
            ),
        );
        self.transforms.insert(
            key + "-transform",
            VectorTransform::from_target_group_with_job(target_group, job, job_parameters),
        );
    }
}

#[derive(Debug, Serialize)]
struct VectorSource {
    #[serde(rename = "type")]
    _type: String,
    endpoints: Vec<String>,
    scrape_interval_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy: Option<VectorSourceProxy>,
    instance_tag: String,
    endpoint_tag: String,
}

impl VectorSource {
    fn from_target_group_with_job(
        tg: TargetGroup,
        _job: &JobType,
        job_parameters: &JobParameters,
        scrape_interval: u64,
        proxy_url: Option<Url>,
    ) -> Self {
        let endpoints: Vec<String> = tg
            .targets
            .into_iter()
            .map(|g| g.to_string())
            .map(|g| format!("http://{}{}", g, job_parameters.endpoint))
            .map(|g| url::Url::parse(&g).unwrap())
            .map(|g| g.to_string())
            .collect();

        // TODO Pass URL through args

        let proxy = proxy_url.map(|url| VectorSourceProxy {
            enabled: true,
            http: Some(url),
            https: None,
        });

        Self {
            _type: "prometheus_scrape".into(),
            endpoints,
            scrape_interval_secs: scrape_interval,
            proxy,
            // proxy: Some(VectorSourceProxy {
            //     enabled: true,
            //     http: Some(proxy_url),
            //     https: None,
            // }),
            instance_tag: "instance".into(),
            endpoint_tag: "endpoint".into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct VectorSourceProxy {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    http: Option<url::Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    https: Option<url::Url>,
}

#[derive(Debug, Serialize)]
struct VectorTransform {
    #[serde(rename = "type")]
    _type: String,
    inputs: Vec<String>,
    source: String,
}

impl VectorTransform {
    fn from_target_group_with_job(
        tg: TargetGroup,
        job: &JobType,
        _job_parameters: &JobParameters,
    ) -> Self {
        let mut labels: HashMap<String, String> = HashMap::new();
        labels.insert(IC_NAME.into(), tg.ic_name);
        labels.insert(IC_NODE.into(), tg.node_id.to_string());
        if let Some(subnet_id) = tg.subnet_id {
            labels.insert(IC_SUBNET.into(), subnet_id.to_string());
        }
        labels.insert("job".into(), job.to_string());
        Self {
            _type: "remap".into(),
            inputs: tg
                .targets
                .into_iter()
                .map(|g| g.to_string())
                .map(|g| g + "-source")
                .collect(),
            source: labels
                .into_iter()
                // Might be dangerous as the tag value is coming from an outside source and
                // is not escaped.
                .map(|(k, v)| format!(".tags.{} = \"{}\"", k, v))
                .collect::<Vec<String>>()
                .join("\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddrV6, str::FromStr};

    use ic_types::{NodeId, PrincipalId, SubnetId};
    use service_discovery::{job_types::JobType, TargetGroup};

    use std::collections::BTreeSet;

    use crate::get_jobs_parameters;
    use crate::vector_configuration::VectorServiceDiscoveryConfigEnriched;

    #[test]
    fn try_from_prometheus_target_group_to_vector_config_correct_inputs() {
        let original_addr = "[2a02:800:2:2003:5000:f6ff:fec4:4c86]:9091";
        let sources_key = String::from(original_addr) + "-source";
        let transforms_key = String::from(original_addr) + "-transform";
        let mut targets = BTreeSet::new();
        targets.insert(std::net::SocketAddr::V6(
            SocketAddrV6::from_str(original_addr).unwrap(),
        ));
        let ptg = TargetGroup {
            node_id: NodeId::from(
                PrincipalId::from_str(
                    "iylgr-zpxwq-kqgmf-4srtx-o4eey-d6bln-smmq6-we7px-ibdea-nondy-eae",
                )
                .unwrap(),
            ),
            ic_name: "mercury".into(),
            targets,
            subnet_id: Some(SubnetId::from(
                PrincipalId::from_str(
                    "x33ed-h457x-bsgyx-oqxqf-6pzwv-wkhzr-rm2j3-npodi-purzm-n66cg-gae",
                )
                .unwrap(),
            )),
            dc_id: None,
            operator_id: None,
        };

        let mut tg_set = BTreeSet::new();
        tg_set.insert(ptg);

        let job_params = get_jobs_parameters();
        let vector_config = VectorServiceDiscoveryConfigEnriched::from_target_groups_with_job(
            tg_set,
            &JobType::Orchestrator,
            job_params.get(&JobType::Orchestrator).unwrap(),
            30,
            None,
        );
        assert!(vector_config.sources.contains_key(&sources_key));
        assert!(vector_config.transforms.contains_key(&transforms_key));

        let sources_config_endpoint = vector_config.sources.get(&sources_key);
        if let Some(conf) = sources_config_endpoint {
            assert_eq!(
                conf.endpoints[0],
                url::Url::parse(&("http://".to_owned() + original_addr))
                    .unwrap()
                    .to_string()
            )
        }
    }
}
